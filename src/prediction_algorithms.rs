//! This module contains the prediction algorithms used to predict drying times.
//! 
//! The nonlinear process model is:
//!    R(t) = (M_0 * exp(-k*t) - M_C)^(-TAU) + R_0
//! 
//! State vector:
//!    x = [R, k, tau, M_0, M_C, R_0]^T
//! 

use kalman_filters::NonlinearSystem;

/// State space model for the drying process. 
/// It follows the model R(t) = (M_0 * exp(-k*t) - M_C)^(-TAU) + R_0
struct MoistureSensorModel {
    _r: f64, // Resistance of the sensor
    _k: f64, // k parameter of the model
    _tau: f64, // tau parameter of the model
    _m_0: f64, // Initial moisture content
    _m_c: f64, // Critical moisture content
    _r_0: f64, // Resistance offset parameter
}

impl NonlinearSystem<f64> for MoistureSensorModel {

    fn state_transition(&self, state: &[f64], _control: Option<&[f64]>, dt: f64) -> Vec<f64> {
        let _r = state[0];
        let k = state[1];
        let tau = state[2];
        let m_0 = state[3];
        let m_c = state[4];
        let r_0 = state[5];

        vec![
            (m_0 * (-k * dt).exp() - m_c).clamp(1e-9, f64::INFINITY).powf(-tau) + r_0, // R(t)
            k, // constant k however will likely change sligthly due to changes in the drying conditions
            tau, // tau should remain constant
            m_0, // M_0 should remain constant
            m_c, // M_c should remain constant
            r_0, // R_0 should remain constant
        ]
    }

    fn measurement(&self, state: &[f64]) -> Vec<f64> {
        vec![state[0]] // We only measure the resistance R
    }

    #[allow(non_snake_case)]
    fn state_jacobian(&self, state: &[f64], _control: Option<&[f64]>, dt: f64) -> Vec<f64> {
        let _r = state[0];
        let k = state[1];
        let tau = state[2];
        let m_0 = state[3];
        let m_c = state[4];
        let _r_0 = state[5];

        // consider clamping to epsilon rather than 1e-9
        let base = (m_0 * (-k * dt).exp() - m_c).clamp(1e-9, f64::INFINITY); // Avoid negative or zero base for the power function

        // Note that we make also need to clamp the jacobian to getting extrememly high estimated resistance values.

        let _dR_dR = 0.0;
        let dR_dk = tau * dt * m_0 * (-k*dt).exp() * base.powf(-tau-1.0);
        let dR_dtau = -(base.ln()) * base.powf(-tau);
        let dR_dM_0 = -tau * (-k*dt).exp() * base.powf(-tau - 1.0);
        let dR_dM_c = tau * base.powf(-tau - 1.0);
        let dR_dR_0 = 1.0;

        // note that the library requires the jacobian to be flatterned into a vector
        // it uses the follow call F[i * n + k] where i is the row, n is the dimension and k is the column. 
        // the whole thing uses a nasty for loop, we should be able to vectorize this in the future if we want to speed up the EKF.
            vec![
                0.0, // dR/dR
                dR_dk, // dR/dk
                dR_dtau, // dR/dtau
                dR_dM_0, // dR/dM_0
                dR_dM_c, // dR/dM_c
                dR_dR_0, // dR/dR_0
            
            0.0, 1.0, 0.0, 0.0, 0.0, 0.0, // dk/dR, dk/dk, dk/dtau, dk/dM_0, dk/dM_c, dk/dR_0
            0.0, 0.0, 1.0, 0.0, 0.0, 0.0, // dtau/dR, dtau/dk, dtau/dtau, dtau/dM_0, dtau/dM_c, dtau/dR_0
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, // dM_0/dR, dM_0/dk, dM_0/dtau, dM_0/dM_0, dM_0/dM_c, dM_0/dR_0
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0, // dM_c/dR, dM_c/dk, dM_c/dtau, dM_c/dM_0, dM_c/dM_c, dM_c/dR_0
            0.0, 0.0, 0.0, 0.0, 0.0, 1.0] // dR_0/dR, dR_0/dk, dR_0/dtau, dR_0/dM_0, dR_0/dM_c, dR_0/dR_0
        
    }

    fn measurement_jacobian(&self, _state: &[f64]) -> Vec<f64> {
        
        vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0] // dR/dR, dR/dk, dR/dtau, dR/dM_0, dR/dM_c, dR/dR_0
        
    }

    fn state_dim(&self) -> usize {
        6 // R, k, tau, M_0, M_c, R_0
    }

    fn measurement_dim(&self) -> usize {
        1 // We only measure R
    }
}





#[cfg(test)]
mod tests {
    use kalman_filters::ExtendedKalmanFilterBuilder;

    use super::*;
    
    #[test]
    fn test_setup() {
        let system = MoistureSensorModel {
            _r: 0.0,
            _k: 0.0,
            _tau: 0.0,
            _m_0: 0.0,
            _m_c: 0.0,
            _r_0: 0.0,
        };

        let dt = 2.0; // Time step in minutes

        // state vector x = [R, k, tau, M_0, M_c, R_0]^T
        let initial_state = vec![30000.0, 0.1, 0.81, 0.02, 1e-9, 29976.33]; // Initial state guess
        let initial_covariance =  vec![
                            vec![1.0e1, 0.0, 0.0, 0.0, 0.0, 0.0], // R
                            vec![0.0, 1.0e-6, 0.0, 0.0, 0.0, 0.0], // k
                            vec![0.0, 0.0, 1.0e-6, 0.0, 0.0, 0.0], // tau
                            vec![0.0, 0.0, 0.0, 1.0e-10, 0.0, 0.0], // M_0
                            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0], // M_c
                            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0] // R_0
                        ].into_iter().flatten().collect(); // Flatten the 2D vector into a 1D vector
        
        let process_noise_covariance = vec![ // This is the Q matrix
            vec![1.0e-2, 0.0, 0.0, 0.0, 0.0, 0.0], // R
            vec![0.0, 1.0e-8, 0.0, 0.0, 0.0, 0.0], // k
            vec![0.0, 0.0, 1.0e-7, 0.0, 0.0, 0.0], // tau
            vec![0.0, 0.0, 0.0, 1.0e-12, 0.0, 0.0], // M_0
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0], // M_c
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0] // R_0
        ].into_iter().flatten().collect(); // Flatten the 2D vector into a 1D vector

        let measurement_noise_covariance = vec![1.0e6]; // This is the R matrix

        let mut ekf = ExtendedKalmanFilterBuilder::new(system)
            .initial_state(initial_state)
            .initial_covariance(initial_covariance)
            .process_noise(process_noise_covariance)
            .measurement_noise(measurement_noise_covariance)
            .dt(dt)
            .build()
            .unwrap();

        
        let initial_state = ekf.state().to_vec();
        ekf.predict();// Propagate the state forward by dt
        ekf.update(&vec![35000.0]).unwrap(); 
        let updated_state = ekf.state().to_vec();
        
        println!("Initial state: {:?}", initial_state);
        println!("Updated state: {:?}", updated_state);

        assert!(updated_state[0] > initial_state[0], "The predicted resistance should increase after the update");

    
     }
}
