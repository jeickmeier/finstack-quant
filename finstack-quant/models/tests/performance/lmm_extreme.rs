//! Regression cases for finite pairwise drift under extreme factor cancellation.

use crate::lmm_reference as reference;
use finstack_quant_models::monte_carlo::discretization::lmm_predictor_corrector::LmmPredictorCorrector;
use finstack_quant_models::monte_carlo::process::lmm::{LmmParams, LmmProcess};
use finstack_quant_models::monte_carlo::traits::{Discretization, StochasticProcess};

fn parameters(displacements: [f64; 2], loadings: [[f64; 3]; 2]) -> LmmParams {
    LmmParams {
        num_forwards: 2,
        num_factors: 2,
        tenors: vec![1.0, 2.0, 4.0],
        accrual_factors: vec![1.0, 2.0],
        displacements: displacements.to_vec(),
        vol_times: vec![],
        vol_values: vec![loadings.to_vec()],
        initial_forwards: vec![0.0, 0.0],
    }
}

#[test]
fn lmm_extreme_cancellation_drift_stays_finite() {
    let large = f64::from_bits(0x7fe0_0000_0000_0000);
    let small = f64::MIN_POSITIVE;
    let params = parameters([1.0, 1.0], [[small, small, 0.0], [large, -large, 0.0]]);
    params.validate().expect("finite accepted parameters");
    // Each covariance product is exactly +/-2 before cancellation. The
    // weighted absolute contribution scale is 2 * (2 + 2) = 8, finite.
    assert_eq!(small * large, 2.0);
    let mut expected = [0.0; 2];
    reference::drift(&params, 0.0, &params.initial_forwards, &mut expected);
    assert_eq!(expected.map(f64::to_bits), [0; 2]);
    let process = LmmProcess::new(params.clone());
    let mut actual = [0.0; 2];
    process.drift(0.0, &params.initial_forwards, &mut actual);
    assert!(actual.iter().all(|value| value.is_finite()), "{actual:?}");
    assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits));
}

#[test]
fn lmm_extreme_zero_shock_step_stays_finite() {
    let params = parameters([1.0, 1e160], [[1e-160, 1e-160, 0.0], [1e150, -1e150, 0.0]]);
    params.validate().expect("finite accepted parameters");
    assert!(reference::step_is_aligned(&params, 0.0, 0.5));
    let mut expected = params.initial_forwards.clone();
    reference::step(&params, 0.0, 0.5, &mut expected, &[0.0; 2], &mut [0.0; 6]);
    assert!(expected.iter().all(|value| value.is_finite()));
    assert_eq!(expected[0].to_bits(), 0.0_f64.to_bits());
    assert_eq!(expected[1], -params.displacements[1]);
    let mut actual = params.initial_forwards.clone();
    let process = LmmProcess::new(params);
    let scheme = LmmPredictorCorrector::new();
    let mut work = vec![0.0; scheme.work_size(&process)];
    scheme.step(&process, 0.0, 0.5, &mut actual, &[0.0; 2], &mut work);
    assert!(actual.iter().all(|value| value.is_finite()), "{actual:?}");
    assert_eq!(actual, expected);
}
