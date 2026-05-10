//! This module contains the prediction algorithms used to predict drying times.
//! 
//! The  reccursive nonlinear process model is:
//!    R_{t+1} = (M_t*e^{−k(dt)}−M_c​)^{−\tau} + R_{offset}
//! 
//! State vector:
//!    x = [R, M, k, tau, M_c, R_offset]^T
//! 



    
use kalman_filters::NonlinearSystem;

/// State space model for the drying process. 
/// It follows the model R_{t+1} = (M_t*e^{−k(dt)}−M_c​)^{−\tau} + R_{offset}
pub struct MoistureSensorModel {
    pub _r: f64, // Resistance of the sensor
    pub _m: f64, // Initial moisture content
    pub _k: f64, // k parameter of the model
    pub _tau: f64, // tau parameter of the model
    pub _m_c: f64, // Critical moisture content
    pub _r_offset: f64, // Resistance offset parameter
}

impl NonlinearSystem<f64> for MoistureSensorModel {

    fn state_transition(&self, state: &[f64], _control: Option<&[f64]>, dt: f64) -> Vec<f64> {
        // current state vector (x_t)
        let _r = state[0];
        let m = state[1]; // Current moisture M(t); advances each step
        let k = state[2];
        let tau = state[3];
        let m_c = state[4];
        let r_offset = state[5];


        // applying the state transition function to compute the next state (x_{t+1})
        // x_{t+1} = [
        //     (M_t * e^{-k(dt)} - M_c)^{-\tau} + R_offset
        //     M_t * e^{-k(dt)}
        //     k
        //     \tau
        //     M_c
        //     R_offset
        // ] + W_t

        // Advance moisture by one time step: M(t+dt) = M(t) * exp(-k*dt)
        let m_next = (m * (-k * dt).exp()).clamp(1e-9, f64::INFINITY); // M_t * e^{-k(dt)}
        let base = (m_next - m_c).clamp(1e-9, f64::INFINITY); // M_t * e^{-k(dt)} - M_c

        vec![
            base.powf(-tau) + r_offset, // R(t+dt) computed from the advanced moisture M(t+dt)
            m_next, // M(t+dt): moisture content advances each step
            k,      // k is modelled as approximately constant
            tau,    // tau is constant
            m_c,    // M_c is constant
            r_offset,    // R_0 is constant
        ]
    }

    fn measurement(&self, state: &[f64]) -> Vec<f64> {
        vec![state[0]] // We only measure the resistance R
    }

    #[allow(non_snake_case)]
    fn state_jacobian(&self, state: &[f64], _control: Option<&[f64]>, dt: f64) -> Vec<f64> {
        let _r = state[0];
        let m = state[1]; // Current moisture M(t)
        let k = state[2];
        let tau = state[3];
        let m_c = state[4];
        let _r_offset = state[5];

        // Compute M(t+dt) and the base term, matching state_transition exactly
        let m_next = m * (-k * dt).exp(); // M(t+dt) = M(t) * exp(-k*dt)
        let base = (m_next - m_c).clamp(1e-9, f64::INFINITY);

        // Note that we may also need to clamp the jacobian to avoid extremely high estimated resistance values.

        // --- Row 0: dR_next / d[R, k, tau, M, M_c, R_0] ---
        let _dR_dR = 0.0;
        let dR_dM   = -tau * (-k * dt).exp() * base.powf(-tau - 1.0); // dR/dM (M is current moisture)
        let dR_dk   =  tau * dt * m_next * base.powf(-tau - 1.0); // chain rule through M_next
        let dR_dtau = -(base.ln()) * base.powf(-tau);
        let dR_dM_c =  tau * base.powf(-tau - 1.0);
        let dR_dR_offset = 1.0;

        // --- Row 1: dM_next / d[R, k, tau, M, M_c, R_0] ---
        // M_next = M * exp(-k*dt), so:
        //   dM_next/dk = -dt * M * exp(-k*dt) = -dt * m_next
        //   dM_next/dM = exp(-k*dt)
        let dM_dk = -dt * m_next;
        let dM_dM = (-k * dt).exp();

        // note that the library requires the jacobian to be flattened into a vector
        // it uses the following call F[i * n + k] where i is the row, n is the dimension and k is the column.
        vec![
            // Row 0: R_next
            0.0, dR_dM, dR_dk, dR_dtau, dR_dM_c, dR_dR_offset,
            // Row 1: M_next — this was the bug: was [0,0,0,1,0,0] treating M as constant
            0.0, dM_dM, dM_dk, 0.0, 0.0, 0.0,
            // Row 2: k (constant)
            0.0, 0.0 , 1.0, 0.0, 0.0, 0.0,
            // Row 3: tau (constant)
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
            // Row 4: M_c (constant)
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
            // Row 5: R_0 (constant)
            0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]
    }

    fn measurement_jacobian(&self, _state: &[f64]) -> Vec<f64> {
        
        vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0] // dR/dR, dR/dM, dR/dk, dR/dtau, dR/dM_c, dR/dR_offset
        
    }

    fn state_dim(&self) -> usize {
        6 // R, M, k, tau, M_c, R_offset
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
            _m: 0.0,
            _k: 0.0,
            _tau: 0.0,
            _m_c: 0.0,
            _r_offset: 0.0,
        };

        let dt = 2.0; // Time step in minutes

        // state vector x = [R, M, k, tau,  M_c, R_0]^T
        let initial_state = vec![30000.0, 0.02, 0.1, 0.81, 1e-9, 29976.33]; // Initial state guess
        let initial_covariance =  vec![
                            vec![1.0e1, 0.0, 0.0, 0.0, 0.0, 0.0], // R
                            vec![0.0, 1.0e-10, 0.0, 0.0, 0.0, 0.0], // M
                            vec![0.0,  0.0, 1.0e-6, 0.0, 0.0, 0.0], // k
                            vec![0.0, 0.0, 0.0, 1.0e-6, 0.0, 0.0], // tau
                            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0], // M_c
                            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0] // R_offset
                        ].into_iter().flatten().collect(); // Flatten the 2D vector into a 1D vector
        
        let process_noise_covariance = vec![ // This is the Q matrix
            vec![1.0e-2, 0.0, 0.0, 0.0, 0.0, 0.0], // R
            vec![0.0, 1.0e-12, 0.0, 0.0,  0.0, 0.0], // M
            vec![0.0, 0.0, 1.0e-8, 0.0, 0.0, 0.0], // k
            vec![0.0, 0.0,  0.0, 1.0e-7, 0.0, 0.0], // tau
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0], // M_c
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0] // R_offset
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
