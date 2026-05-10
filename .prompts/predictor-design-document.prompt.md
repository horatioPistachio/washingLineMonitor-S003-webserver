# Predictor design document

This document describes the washing predictor as it is currently implemented in `src/prediction_algorithms.rs` and `src/washing_predictor.rs`.

The aim is to keep the maths, the Rust types, and the runtime behavior aligned so the design note reflects the real code rather than an earlier draft.

## Drying model

The predictor models sensor resistance as a nonlinear function of moisture content:

$$
R(t) = (M_0 e^{-kt} - M_c)^{-\tau} + R_{offset}
$$

Where:

- $R(t)$ is the measured sensor resistance.
- $M(t)$ is the moisture content of the fabric.
- $M_0$ is the initial moisture estimate at the start of a drying cycle.
- $k$ is the drying coefficient.
- $M_c$ is the critical moisture threshold.
- $\tau$ controls how sharply resistance rises near the critical point.
- $R_{offset}$ is the sensor's baseline resistance offset.

The implementation uses the exponential drying law:

$$
M(t) = M_0 e^{-kt}
$$

This makes the resistance curve start relatively flat, then rise sharply as moisture approaches the critical threshold.

## Why an Extended Kalman Filter

The state transition is nonlinear, so a standard linear Kalman filter is not sufficient.

The recursive model implemented in code is:

$$
R_{t+1} = (M_t e^{-k \cdot dt} - M_c)^{-\tau} + R_{offset}
$$

Because this cannot be written as a simple linear matrix multiply, the predictor uses an Extended Kalman Filter (EKF), which linearizes the system around the current state using Jacobians.

## Implemented state vector

The current code uses this six-element state vector:

$$
x_t = \begin{bmatrix}
R_t \\
M_t \\
k \\
\\tau \\
M_c \\
R_{offset}
\end{bmatrix}
$$

State ordering matters because the implementation stores the EKF state as `Vec<f64>`, so each index has a fixed meaning:

| Index | Symbol | Meaning |
|-------|--------|---------|
| 0 | $R$ | Current resistance state |
| 1 | $M$ | Current moisture state |
| 2 | $k$ | Drying coefficient |
| 3 | $\tau$ | Phase-transition exponent |
| 4 | $M_c$ | Critical moisture threshold |
| 5 | $R_{offset}$ | Resistance offset |

Important clarification: the implementation does **not** keep $M_0$ as a separate state after initialization. The initial moisture guess is loaded into index `1` when the EKF is created, and after that the same slot represents the evolving moisture state $M_t$.

This is an important Rust design lesson: when a state vector is stored as positional numeric data, the documentation must stay tightly synchronized with the code, otherwise the compiler cannot protect you from semantic index mistakes.

## State transition and measurement model

The implemented state transition is:

$$
x_{t+1} =
\begin{bmatrix}
(M_t e^{-k \cdot dt} - M_c)^{-\tau} + R_{offset} \\
M_t e^{-k \cdot dt} \\
k \\
\\tau \\
M_c \\
R_{offset}
\end{bmatrix}
+ W_t
$$

Where $W_t$ is the process noise.

Only resistance is measured directly, so the measurement model is:

$$
h(x_t) = [R_t] + V_t
$$

Where $V_t$ is the measurement noise.

In the Rust implementation:

- `measurement(state)` returns `vec![state[0]]`
- `measurement_jacobian(state)` returns `[1, 0, 0, 0, 0, 0]`

The Jacobian in `prediction_algorithms.rs` also models the moisture state as dynamic and the remaining parameters as approximately constant states.

## Initial conditions

The EKF is initialized from database-provided parameters:

$$
x_0 = \begin{bmatrix}
R_0 \\
M_0 \\
k_0 \\
\\tau_0 \\
M_{c,0} \\
R_{offset,0}
\end{bmatrix}
$$

The production code also loads:

- the flattened initial covariance matrix
- the flattened process noise covariance matrix
- the measurement noise covariance
- a fixed timestep `dt`

These are wrapped in the `EKFParameters` struct.

## Rust API and type design

### Telemetry input

```rust
pub struct TelemetryData {
    pub timestamp: DateTime<Utc>,
    pub resistance: f64,
}
```

This struct derives `Deserialize`, so it can be populated directly from incoming request data.

### Repository abstraction

The predictor does not call SQLx directly from its core logic. Instead it depends on a trait:

```rust
pub trait DeviceRepository: Send + Sync {
    async fn get_ekf_parameters(&self, device_id: &str) -> Result<EKFParameters, PredictorError>;
}
```

This is a good Rust design choice for two reasons:

1. It separates prediction logic from storage logic.
2. It makes unit testing easy by allowing a mock repository.

`WashingPredictor<R>` is generic over the repository type, so production can use `PostgresDeviceRepository` while tests use `MockDeviceRepository`. In Rust this is zero-cost static dispatch because the compiler monomorphizes the generic type.

### Predictor struct

```rust
pub struct WashingPredictor<R: DeviceRepository> {
    repo: R,
    predictor_cache: DashMap<String, EKFEntry>,
}
```

The predictor stores one EKF instance per device in a `DashMap`.

This is idiomatic Rust for a shared, concurrent cache:

- the API accepts `&str` for `device_id` so callers do not have to allocate
- the cache stores an owned `String` because the key must outlive the request
- `DashMap` provides interior mutability and sharded locking for concurrent access

### Per-device cache entry

```rust
struct EKFEntry {
    ekf: ExtendedKalmanFilter<f64, MoistureSensorModel>,
    start_time: DateTime<Utc>,
    last_received_time: DateTime<Utc>,
}
```

`start_time` and `last_received_time` are stored alongside the EKF state. In the current implementation:

- both fields are initialized from the first telemetry timestamp for a device
- `last_received_time` is used by the cache eviction helper
- completion time is currently calculated from the incoming telemetry timestamp rather than `start_time`

## Database/configuration contract

The production repository implementation performs this SQL query:

```sql
SELECT configuration FROM devices WHERE device_id = $1
```

It reads the `configuration` column as JSON and deserializes it into:

```rust
struct Wrapper {
    configuration: EKFParameters,
}
```

So the current code expects the JSON column to deserialize into a wrapper object whose `configuration` field contains the EKF parameters.

`EKFParameters` currently contains:

```rust
pub(crate) struct EKFParameters {
    pub(crate) initial_state: Vec<f64>,
    pub(crate) initial_covariance: Vec<f64>,
    pub(crate) process_noise_covariance: Vec<f64>,
    pub(crate) measurement_noise_covariance: Vec<f64>,
    pub(crate) dt: f64,
}
```

Using `Vec<f64>` is pragmatic because it matches the current EKF builder API, even though fixed-size arrays would carry stronger compile-time guarantees.

## Prediction flow in the current implementation

`predict_drying_time(&self, device_id: &str, telemetry_data: TelemetryData)` behaves as follows:

1. Look up the device's EKF in `predictor_cache`.
2. If there is no entry, fetch `EKFParameters` through the repository, build a new `ExtendedKalmanFilter`, and insert a new `EKFEntry` into the cache.
3. Compare the current predicted resistance `entry.ekf.state()[0]` with the incoming telemetry resistance.
4. If the absolute difference is greater than `1e5`, treat it as a new drying cycle, remove the cached EKF entry, and retry once with a fresh filter.
5. Call `predict()` on the EKF.
6. Call `update(&vec![telemetry_data.resistance])` with the latest resistance measurement.
7. Read the updated state vector.
8. Compute a completion `DateTime<Utc>` from the updated state and the telemetry timestamp.

The retry loop exists to handle a sudden large resistance jump without leaving the old EKF state in place.

## Completion-time calculation used by the code

The current implementation computes **remaining time** from the current moisture estimate, not absolute drying time from the original start state.

Starting from the recursive moisture model:

$$
M(t + \Delta t) = M(t)e^{-k \Delta t}
$$

Dryness is treated as the point where the moisture reaches the critical threshold:

$$
M_c = M(t)e^{-k \Delta t}
$$

Solving for the remaining time gives:

$$
\Delta t = \frac{1}{k} \ln\!\left(\frac{M(t)}{M_c}\right)
$$

The code implements the algebraically equivalent form:

$$
\Delta t = -\frac{\ln(M_c / M(t))}{k}
$$

Then it returns:

$$
t_{completion} = t_{telemetry} + \Delta t
$$

This means the returned `DateTime<Utc>` is anchored to the timestamp on the newest telemetry packet, not to the cached `start_time`.

The current guard in code is also implementation-specific: if the computed remaining time is `NaN` or negative, the predictor returns `PredictorError::InvalidPrediction`.

## Error model

The current predictor error enum is:

```rust
#[derive(Debug, thiserror::Error)]
pub enum PredictorError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("EKF error for device {device_id}: {message}")]
    EkfError { device_id: String, message: String },

    #[error("device {0} not found in database")]
    DeviceNotFound(String),

    #[error("Unable to create device in DashMap cache for device {0}")]
    CacheInsertError(String),

    #[error("drying time calculation produced invalid result (NaN/negative)")]
    InvalidPrediction,
}
```

`thiserror` is a good fit here because it keeps the error type readable and lets `?` convert `sqlx::Error` into `PredictorError::Database` automatically.

## Maintenance helpers in the current code

The predictor also has three helper methods used by tests and cache management:

- `reset_predictor(device_id)` removes one cached EKF entry.
- `reset_old_predictors(max_age)` evicts entries whose `last_received_time` is older than `max_age`.
- `get_cache_size()` returns the number of cached predictors.

These are currently internal methods on `WashingPredictor` rather than part of a public external API.

## Rust best-practice takeaways from this design

- Use trait-based dependency inversion for database access so the algorithm can be tested without a real database.
- Borrow at the API boundary (`&str`) and own data only where long-lived storage requires it (`String` in the cache).
- Keep algorithm state and metadata together in a small struct such as `EKFEntry` rather than scattering related fields across multiple maps.
- When representing scientific state as a raw vector, document the index layout clearly and keep the doc in sync with the implementation.
- Prefer a dedicated error enum over `Box<dyn Error>` so callers can react to meaningful failure modes.

## Future work

- Consider replacing raw positional state indexing with named constants or helper accessors to reduce the risk of index-order bugs.
- Consider explicit validation for `k > 0`, `M > 0`, and `M_c > 0` before calculating the remaining time.
- Consider updating the EKF timestep dynamically if telemetry becomes irregular and the EKF crate supports that cleanly.



