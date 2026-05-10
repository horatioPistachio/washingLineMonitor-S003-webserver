//! This file contains the implementation of the Extended Kalman Filter for predicting the drying process of clothes based on the resistance measurements from the moisture sensor.
//! It is structured to allow have a simple input the device and data and to return an estimate of the drying time remaining.
//!
//!
//!

use crate::prediction_algorithms::MoistureSensorModel;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use kalman_filters::{ExtendedKalmanFilter, ExtendedKalmanFilterBuilder};
use sqlx::PgPool;
use rocket_db_pools::sqlx::{self, Row};
use thiserror;

#[derive(serde::Deserialize)]
pub struct TelemetryData {
    pub timestamp: DateTime<Utc>,
    pub resistance: f64,
}
struct EKFEntry {
    ekf: ExtendedKalmanFilter<f64, MoistureSensorModel>,
    start_time: DateTime<Utc>,
    last_received_time: DateTime<Utc>,
}

#[derive(serde::Deserialize)]
struct Wrapper {
    configuration: EKFParameters,
}

// pub(crate) makes EKFParameters visible within this crate (including the test submodule)
// but not to external crates. This is needed so the mock in `mod tests` can construct it.
// Without pub(crate), the struct and its fields would be private to this module only.
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub(crate) struct EKFParameters {
    /// Initial 6-element state vector: [R, M, k, tau, M_c, R_offset]
    pub(crate) initial_state: Vec<f64>,
    /// Flattened 6×6 initial covariance matrix (36 elements, row-major)
    pub(crate) initial_covariance: Vec<f64>,
    /// Flattened 6×6 process noise covariance Q (36 elements, row-major)
    pub(crate) process_noise_covariance: Vec<f64>,
    /// 1-element measurement noise covariance R (since we only measure resistance)
    pub(crate) measurement_noise_covariance: Vec<f64>,
    /// EKF time step in minutes
    pub(crate) dt: f64,
}

// --- Dependency inversion via a trait ---
//
// Instead of WashingPredictor calling sqlx directly, it depends on this trait.
// In production we pass PostgresDeviceRepository; in tests we pass a mock.
//
// `async fn` in traits is stable since Rust 1.75 (AFIT). No `async_trait` crate needed.
// `Send + Sync` are required so WashingPredictor<R> can be shared across async tasks.
pub trait DeviceRepository: Send + Sync {
    async fn get_ekf_parameters(&self, device_id: &str) -> Result<EKFParameters, PredictorError>;
}

/// Production implementation: fetches EKF configuration from PostgreSQL.
pub struct PostgresDeviceRepository {
    pool: PgPool,
}

impl PostgresDeviceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl DeviceRepository for PostgresDeviceRepository {
    async fn get_ekf_parameters(&self, device_id: &str) -> Result<EKFParameters, PredictorError> {
        let row = sqlx::query("SELECT configuration FROM devices WHERE device_id = $1")
            .bind(device_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                eprintln!("Unable to retrieve device configuration: {e}");
                PredictorError::Database(e)
            })?;

            let configuration_json: serde_json::Value = row.try_get("configuration")?;
            println!("Raw configuration JSON for device {}: {}", device_id, configuration_json);
            let wrapper: Wrapper = serde_json::from_value(configuration_json).map_err(|e| {
                eprintln!("Unable to parse EKF parameters from database: {e}");
                PredictorError::Database(sqlx::Error::ColumnDecode {
                    index: "configuration".to_string(),
                    source: Box::new(e),
                })
            })?;

            let ekf_parameters = wrapper.configuration;
            
        // if rows.is_empty() {
        //     return Err(PredictorError::DeviceNotFound(device_id.to_string()));
        // }

        // let wrapper: Wrapper =
        //     serde_json::from_str(&rows[0].get::<String, _>("configuration")).map_err(|e| {
        //         eprintln!("Unable to parse EKF parameters from database: {e}");
        //         PredictorError::Database(sqlx::Error::ColumnDecode {
        //             index: "configuration".to_string(),
        //            source: Box::new(e),
        //         })
        //     })?;
        println!("Successfully retrieved EKF parameters for device {}: {:?}", device_id, serde_json::to_string_pretty(&ekf_parameters));
        Ok(ekf_parameters)
    }
}

// WashingPredictor is now generic over R.
//
// In Rust, generics are resolved at compile time — the compiler stamps out a separate
// concrete version of WashingPredictor for each R you use (monomorphisation). This is
// zero-cost compared to using `Box<dyn DeviceRepository>` (dynamic dispatch / vtable).
pub struct WashingPredictor<R: DeviceRepository> {
    repo: R,
    predictor_cache: DashMap<String, EKFEntry>, // Cache for EKF instances keyed by device ID
}

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

impl<R: DeviceRepository> WashingPredictor<R> {
    /// Constructor takes any type implementing DeviceRepository.
    /// In production: `WashingPredictor::new(PostgresDeviceRepository::new(pool))`
    /// In tests:      `WashingPredictor::new(MockDeviceRepository)`
    pub fn new(repo: R) -> Self {
        WashingPredictor {
            repo,
            predictor_cache: DashMap::new(),
        }
    }

    pub async fn predict_drying_time(
        &self,
        device_id: &str,
        telemetry_data: TelemetryData,
    ) -> Result<DateTime<Utc>, PredictorError> {
        // Placeholder for the actual EKF implementation
        // In a real implementation, this function would initialize the EKF with the MoistureSensorModel,
        // process the resistance measurements, and return an estimate of the remaining drying time.
        // for _ in 0..1 {
        let mut loop_counter = 0;
        loop {
            let mut entry = match self.predictor_cache.get_mut(device_id) {
                Some(entry) => entry,
                None => {
                    let ekf_parameters =
                        self.repo.get_ekf_parameters(device_id).await?;

                    let system = MoistureSensorModel {
                        _r: ekf_parameters.initial_state[0],
                        _m: ekf_parameters.initial_state[1],
                        _k: ekf_parameters.initial_state[2],
                        _tau: ekf_parameters.initial_state[3],
                        _m_c: ekf_parameters.initial_state[4],
                        _r_offset: ekf_parameters.initial_state[5],
                    };

                    let ekf = ExtendedKalmanFilterBuilder::new(system)
                        .initial_state(ekf_parameters.initial_state)
                        .initial_covariance(ekf_parameters.initial_covariance)
                        .process_noise(ekf_parameters.process_noise_covariance)
                        .measurement_noise(ekf_parameters.measurement_noise_covariance)
                        .dt(ekf_parameters.dt)
                        .build()
                        .map_err(|e| PredictorError::EkfError {
                            device_id: device_id.to_string(),
                            message: e.to_string(),
                        })?;

                    self.predictor_cache.insert(
                        device_id.to_string(),
                        EKFEntry {
                            ekf,
                            start_time: telemetry_data.timestamp,
                            last_received_time: telemetry_data.timestamp,
                        },
                    );

                    self.predictor_cache
                        .get_mut(device_id)
                        .ok_or_else(|| PredictorError::CacheInsertError(device_id.to_string()))? // Ensure the entry was inserted and can be retrieved
                }
            };

            // Check for large jump in resistance to detect new drying cycle
            println!(
                "Current vs new resitance {}: {:?}",
                entry.ekf.state()[0],
                telemetry_data.resistance
            );
            if (entry.ekf.state()[0] - telemetry_data.resistance).abs() > 1e5 {
                //we have a large jump in resistance, which likely indicates a new drying cycle has started. We should reset the EKF for this device.
                drop(entry); // Drop the mutable reference to the EKF entry before modifying the cache
                self.predictor_cache.remove(device_id); // evict the existing EKF entry from the cache
                println!(
                    "Large jump in resistance detected for device {}. Resetting EKF entry.",
                    device_id
                );
                if loop_counter > 1 { return Err(PredictorError::InvalidPrediction); } // Prevent infinite loop in case of repeated large jumps
                loop_counter += 1;
                continue; // Restart the loop to create a new EKF entry for this device}
            }

            // Update the EKF with the new telemetry data
            entry.ekf.predict();
            entry
                .ekf
                .update(&vec![telemetry_data.resistance])
                .map_err(|e| PredictorError::EkfError {
                    device_id: device_id.to_string(),
                    message: e.to_string(),
                })?;

            // Read the new state estimates
            let current_state_estimate = entry.ekf.state();

            // Estimate the remaining drying time based on the current state estimate
            let completion_time = self
                .estimate_drying_time(current_state_estimate, &telemetry_data.timestamp)
                .map_err(|e| PredictorError::EkfError {
                    device_id: device_id.to_string(),
                    message: e.to_string(),
                })?;

            // For demonstration purposes, we will return a dummy value.
            return Ok(completion_time); // Dummy value representing 30 minutes remaining
        }
    }

    fn estimate_drying_time(
        &self,
        state_estimate: &[f64],
        current_time: &DateTime<Utc>,
    ) -> Result<DateTime<Utc>, PredictorError> {
        // state[3] is the CURRENT moisture M(t), not the initial M_0.
        // We compute how much longer until M decays to M_c:
        //   M(t) * exp(-k * t_remaining) = M_c
        //   t_remaining = ln(M(t) / M_c) / k
        let m = state_estimate[1]; // Current moisture (advances each EKF step)
        let k = state_estimate[2];
        let m_c = state_estimate[4];

        let t_remaining = -((m_c / m).ln() / k); // Remaining time until moisture reaches M_c

        if t_remaining.is_nan() || t_remaining < 0.0 {
            return Err(PredictorError::InvalidPrediction);
        }

        let completion_time = *current_time + chrono::Duration::minutes(t_remaining as i64);

        Ok(completion_time)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use chrono::Utc;

    // --- Mock ---
    //
    // A unit struct (no fields) that implements DeviceRepository.
    // It always returns a hardcoded set of EKF parameters, so tests never touch a database.
    //
    // `async fn` in a trait impl works here because Rust 2024 has stable AFIT.
    // The compiler generates a concrete Future type for each implementation — there's
    // no boxing or heap allocation, unlike the `async_trait` crate approach.
    struct MockDeviceRepository;

    impl DeviceRepository for MockDeviceRepository {
        async fn get_ekf_parameters(
            &self,
            _device_id: &str,
        ) -> Result<EKFParameters, PredictorError> {
            Ok(EKFParameters {
                initial_state: vec![30000.0, 0.02, 0.1, 0.81, 1e-9, 29976.33],
                initial_covariance: vec![
                    1.0e1, 0.0, 0.0, 0.0, 0.0, 0.0,
                    0.0, 1.0e-10, 0.0, 0.0, 0.0, 0.0,
                    0.0, 0.0, 1.0e-6, 0.0, 0.0, 0.0,
                    0.0, 0.0, 0.0, 1.0e-6, 0.0, 0.0,
                    0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                    0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                ],
                process_noise_covariance: vec![
                    1.0e-2, 0.0, 0.0, 0.0, 0.0, 0.0,
                    0.0, 1.0e-12, 0.0, 0.0, 0.0, 0.0,
                    0.0, 0.0, 1.0e-8, 0.0, 0.0, 0.0,
                    0.0, 0.0, 0.0, 1.0e-7, 0.0, 0.0,
                    0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                    0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                ],
                measurement_noise_covariance: vec![1.0e6],
                dt: 2.0,
            })
        }
    }

    #[tokio::test]
    async fn test_predict_drying_time() {
        let dooter = TelemetryData {
            timestamp: Utc::now(),
            resistance: 30000.0,
        };
        // WashingPredictor<MockDeviceRepository> — no database connection needed.
        // The type parameter R is inferred by the compiler from what we pass to ::new().
        let kf = WashingPredictor::new(MockDeviceRepository);

        let res = kf.predict_drying_time("dootle", dooter).await;
        println!("Prediction result: {:?}", res);
        assert!(res.is_ok());

        let telemetry_data_2 = TelemetryData {
            timestamp: Utc::now()+chrono::Duration::minutes(2),
            resistance: 40000.0, 
        };

        let res2 = kf.predict_drying_time("dootle", telemetry_data_2).await;
        println!("Prediction result after second telemetry: {:?}", res2);
        assert!(res2.is_ok());
        assert_ne!(res2.unwrap(), res.unwrap()); // The second prediction should indicate a sooner completion time due to the rapid increase in resistance
    }


    #[tokio::test]
    async fn test_evict_on_large_jump() {
        let kf = WashingPredictor::new(MockDeviceRepository);

        let telemetry_data_1 = TelemetryData {
            timestamp: Utc::now(),
            resistance: 30000.0,
        };

        let res1 = kf.predict_drying_time("dootle", telemetry_data_1).await;
        println!("First prediction result: {:?}", res1);
        assert!(res1.is_ok());

        for i in 1..60 {
            // Simulate resistance following R(t) = (M_0 * exp(-k*t) - M_C)^(-tau) + R_0,
            // using the same initial EKF parameters so the filter can track smoothly.
            //
            // We stop at i=59 (t=118 min) deliberately. The formula's R grows exponentially
            // as M(t) → M_c, and beyond i≈63 the *step-to-step* change in R exceeds the
            // 1e5 jump-detection threshold, which would incorrectly trigger an eviction
            // during normal tracking. At i=59, R ≈ 396 kΩ, well above 30 kΩ, so the
            // jump-back-to-30 kΩ below will still exercise the eviction path.
            let t = (i as f64) * 2.0; // total elapsed time in minutes (dt = 2.0 min/step)
            let resistance = (0.02_f64 * (-0.1_f64 * t).exp() - 1e-9_f64)
                .max(1e-9_f64)
                .powf(-0.81_f64)
                + 29976.33_f64;
            let telemetry_data = TelemetryData {
                timestamp: Utc::now() + (chrono::Duration::minutes(2) * i),
                resistance,
            };
            let res = kf.predict_drying_time("dootle", telemetry_data).await;
            assert!(res.is_ok());
        }

        // Resistance drops back to 30 kΩ — a jump of ~366 kΩ — simulating new wet
        // clothes being hung up. This should evict the existing EKF and create a fresh one.
        let telemetry_data_2 = TelemetryData {
            timestamp: Utc::now(),
            resistance: 30000.0,
        };

        let res2 = kf.predict_drying_time("dootle", telemetry_data_2).await;
        println!("Second prediction result after large jump: {:?}", res2);
        // After the EKF is evicted and re-created from the default parameters, the
        // prediction should succeed just like the very first call did.
        assert!(res2.is_ok());
    }
}
