# Predictor design document
This document outlines the design of the predictor component for the washing line monitor system. The predictor is responsible for estimating the drying time remaining based on the resistance measurements from the moisture sensor.

It is designed to be closely linked to the webserver component, allowing it to receive telemetry data and return an estimated completion time.

## Drying model

The nonlinear process model relating sensor resistance to time is:

$$R(t) = (M_0 \cdot e^{-kt} - M_C)^{-\tau} + R_0$$

This model is implemented in `prediction_algorithms.rs` as `MoistureSensorModel` and tracked by a 6-state Extended Kalman Filter:

| Index | Symbol | Description |
|-------|--------|-------------|
| 0 | R | Sensor resistance (the measured quantity) |
| 1 | k | Drying rate constant |
| 2 | τ (tau) | Power-law exponent |
| 3 | M_0 | Initial moisture content |
| 4 | M_C | Critical moisture content |
| 5 | R_0 | Resistance offset (wet baseline) |

Only R is directly measured; the remaining five parameters are estimated by the EKF as constant-dynamics states (identity transitions).


## Public API

```rust
struct TelemetryData {
    timestamp: DateTime<Utc>,
    resistance: f64,
}
```

---

**`fn predict_drying_time(device_id: &str, telemetry_data: TelemetryData) -> Result<DateTime<Utc>, PredictorError>`**

Takes in the device_id and the latest telemetry data (timestamp + resistance measurement). Returns an estimated completion time for when the clothes will be dry.

Internally it runs the EKF instance for the given device_id, updating the state with the new telemetry data and returning the predicted drying time.

The function uses the `dashmap` crate to manage EKF instances for each device, allowing concurrent access and updates from multiple devices without blocking. Each device has its own EKF instance that maintains the state of the drying process. If the device_id does not have an existing EKF instance, a new one is created and stored in the DashMap. A database call retrieves the device-specific gains and initial values to initialise the new EKF instance.

The function also checks for large drops between the predicted resistance and the incoming telemetry value. If a large drop is detected, this indicates the device was reset (e.g., new load of washing placed) and the EKF instance is reinitialised.

> **Ownership note:** The function signature accepts `&str` for `device_id` to avoid forcing callers to allocate. Internally, the DashMap key must be an owned `String` (since the map outlives any single request), so the function calls `.to_owned()` when inserting a new entry. This is the idiomatic Rust pattern — borrow at the API boundary, own inside long-lived containers.

---

**`fn reset_predictor(device_id: &str) -> Result<(), PredictorError>`**

Manually resets the predictor for a specific device. Removes the EKF instance associated with the given device_id from the DashMap, effectively clearing all learned state for that device. Useful for testing or if the user wants to force a restart.

In future, an API endpoint could be added to allow users to trigger this from the frontend.

---

**`fn reset_old_predictors(max_age: Duration) -> Result<(), PredictorError>`**

Iterates through the DashMap and removes any EKF instance that hasn't received new telemetry data within `max_age`. This prevents the DashMap from growing indefinitely with stale entries for inactive devices. Intended to be called periodically (e.g., every hour) via a Rocket fairing or background `tokio::spawn` task.

> **Rust concept — DashMap iteration:** `DashMap::retain()` is the idiomatic way to filter entries in-place. It takes a closure and keeps only entries for which the closure returns `true`. Under the hood, DashMap shards the data across multiple locks, so `retain()` only locks one shard at a time — much better than locking the entire map.


## Internal functions

**`async fn get_ekf_parameters_from_db(pool: &PgPool, device_id: &str) -> Result<EKFParameters, PredictorError>`**

Retrieves the initial parameters and gains for the EKF instance from the database for a given device_id. Called when a new EKF instance is created for a device.

Note this function needs a reference to the database connection pool (`&PgPool`), not a Rocket `Connection<Db>`, since it will be called from within the predictor module rather than directly from a route handler.

The following SQL query is used to fetch the parameters, which are stored in a JSONB column in the `devices` table:

```sql
SELECT
    configuration -> 'EKFparameters' AS ekf_parameters
FROM devices
WHERE device_id = $1;
```


> **Async note:** This must be `async` because SQLx database operations are asynchronous. The caller (`predict_drying_time`) will need to be `async` as well, and the `await` will propagate up to the Rocket route handler. Rust's `async`/`await` is zero-cost — it compiles down to a state machine with no heap allocation for the future itself.


## Error type

```rust
#[derive(Debug, thiserror::Error)]
enum PredictorError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("EKF error for device {device_id}: {message}")]
    EkfError { device_id: String, message: String },
    
    #[error("device {0} not found in database")]
    DeviceNotFound(String),

    #[error("drying time calculation produced invalid result (NaN/negative)")]
    InvalidPrediction,
}
```

> **Rust concept — `thiserror`:** The `thiserror` crate generates `Display` and `Error` trait implementations from the `#[error(...)]` attributes. The `#[from]` attribute auto-generates a `From<sqlx::Error>` impl, which means the `?` operator can automatically convert a SQLx error into a `PredictorError`. This is much cleaner than manually writing `match` arms for error conversion.


## Data structures

### DashMap for EKF instances

The predictor uses a `DashMap<String, EKFEntry>` to store the EKF instances for each device. The key is the owned device_id `String` and the value is the EKF entry struct below.

This DashMap is shared with Rocket via managed state:
```rust
// In main.rs / rocket() launch function:
let predictor_map: DashMap<String, EKFEntry> = DashMap::new();
rocket::build()
    .manage(predictor_map)
    // ...routes, fairings, etc.
```

Route handlers access it via `&State<DashMap<String, EKFEntry>>`, and the predictor functions receive `&DashMap<String, EKFEntry>` directly.

> **Rust concept — `State<T>` in Rocket:** `rocket::State<T>` is Rocket's mechanism for dependency injection. You `.manage(value)` during setup, then request `&State<T>` in route parameters. Under the hood it's just an `Arc`-like wrapper — retrieving state is O(1), not a lookup.


### DashMap value struct
```rust
struct EKFEntry {
    ekf: ExtendedKalmanFilter,
    start_time: DateTime<Utc>,
    last_received_time: DateTime<Utc>,
}
```

### Initial parameters and gains struct
```rust
struct EKFParameters {
    /// Initial 6-element state vector: [R, k, tau, M_0, M_C, R_0]
    initial_state: Vec<f64>,
    /// Flattened 6×6 initial covariance matrix (36 elements, row-major)
    initial_covariance: Vec<f64>,
    /// Flattened 6×6 process noise covariance Q (36 elements, row-major)
    process_noise_covariance: Vec<f64>,
    /// 1-element measurement noise covariance R (since we only measure resistance)
    measurement_noise_covariance: Vec<f64>,
    /// EKF time step in minutes
    dt: f64,
}
```

> **Alternative — fixed-size arrays:** Since the state dimension is always 6 and the measurement dimension is always 1, you could use `[f64; 6]` and `[f64; 36]` instead of `Vec<f64>`. This avoids heap allocation and lets the compiler catch size mismatches at compile time. However, the `kalman_filters` crate's builder currently expects `Vec<f64>`, so `Vec` is pragmatic for now.


## Data flow

1. The webserver receives telemetry data from a washing line monitor device via the REST API. *(external)*
2. The webserver calls `predict_drying_time`, passing the device_id and the telemetry data.
3. `predict_drying_time` looks up the device_id in the DashMap:
   - **If no entry exists:** calls `get_ekf_parameters_from_db` to fetch device-specific parameters, builds a new `ExtendedKalmanFilter` via the builder, wraps it in an `EKFEntry` with `start_time = TelemetryData.timestamp`, and inserts it into the DashMap.
4. The function calls the EKF **state** step, which gets the current state. It then compares the current resistance to the incoming telemetry value — if there's a large drop, this indicates a device reset and the EKF instance is reinitialised (back to step 3).
5. The function calls the EKF **predict** and then EKF **update** steps, incorporating the new resistance measurement to correct the state estimate. The `last_received_time` on the entry is updated to `now`.
6. The function extracts the estimated parameters `k`, `M_0`, and `M_C` from the updated state vector and calculates the estimated total drying time using the formula below.
7. The estimated drying time is added to `start_time` of the EKF entry to produce the estimated completion `DateTime<Utc>`, which is returned.
8. The webserver uses this completion time to decide when to send a notification and to display in the web app. *(external)*



## Estimated drying time calculation

$$t^* = \frac{1}{k} \ln\!\left(\frac{M_0}{M_C}\right)$$

This formula finds the time at which the model has a **singularity** — where the base term $(M_0 \cdot e^{-kt} - M_C)$ reaches zero and resistance $R \to \infty$. Physically, this represents the point where the moisture content drops to the critical threshold $M_C$, meaning the fabric is effectively dry as the critical threshold is extremely low.

The formula is derived by setting the base to zero and solving for $t$:

$$M_0 \cdot e^{-kt^*} = M_C \implies t^* = \frac{1}{k} \ln\!\left(\frac{M_0}{M_C}\right)$$

Note that $\tau$ and $R_0$ do not appear in this formula — they affect the *shape* of the resistance curve but not *when* the singularity occurs. The EKF estimates all six state parameters, but only `k`, `M_0`, and `M_C` are needed for the completion time calculation.

**Guard conditions:** The function must validate before computing:
- `k > 0` (positive drying rate)
- `M_0 > M_C > 0` (initial moisture exceeds critical moisture)
- Result is finite and positive

If any condition fails, return `PredictorError::InvalidPrediction`.


## future work

- **Completion threshold:** Rather than using the exact singularity time $t^*$, should we declare drying complete at some percentage before it (e.g., 95% of $t^*$) to account for model uncertainty?
- **Variable dt:** Currently `dt` is fixed. If telemetry arrives at irregular intervals, should `dt` be computed from successive timestamps? 
    - This is a very good idea to make it time independent, however it adds complexity to the EKF implementation as the used crated doesnt appear to support a dynamic dt



