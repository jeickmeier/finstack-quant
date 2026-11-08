//! Untimed checks for the selected Monte Carlo benchmark case.

use finstack_quant_core::currency::Currency;
use finstack_quant_models::monte_carlo::results::MoneyEstimate;

pub fn check_estimate(estimate: &MoneyEstimate, estimators: usize, physical: usize) {
    assert_eq!(estimate.num_paths, estimators);
    assert_eq!(estimate.num_simulated_paths, physical);
    assert_eq!(estimate.mean.currency(), Currency::USD);
    assert_eq!(estimate.ci_95.0.currency(), Currency::USD);
    assert_eq!(estimate.ci_95.1.currency(), Currency::USD);
    assert!(estimate.mean.amount().is_finite() && estimate.mean.amount() >= 0.0);
    assert!(estimate.stderr.is_finite() && estimate.stderr >= 0.0);
    assert!(estimate.ci_95.0.amount().is_finite());
    assert!(estimate.ci_95.1.amount().is_finite());
    assert!(estimate.ci_95.0.amount() <= estimate.mean.amount());
    assert!(estimate.mean.amount() <= estimate.ci_95.1.amount());
    let std_dev = estimate.std_dev.expect("sample standard deviation");
    assert!(std_dev.is_finite() && std_dev >= 0.0);
    assert!(estimate.median.is_none());
    assert!(estimate.percentile_25.is_none());
    assert!(estimate.percentile_75.is_none());
    assert!(estimate.min.is_none());
    assert!(estimate.max.is_none());
}

pub fn check_against_reference(estimate: &MoneyEstimate, expected: f64, absolute_bias: f64) {
    assert!(expected.is_finite() && expected >= 0.0);
    assert!(absolute_bias.is_finite() && absolute_bias >= 0.0);
    // This fixed-seed smoke envelope allows sampling noise and the explicitly
    // named discretization/fit allowance. It does not replace tighter model tests.
    let tolerance = 6.0 * estimate.stderr + absolute_bias;
    assert!(
        (estimate.mean.amount() - expected).abs() <= tolerance,
        "benchmark mean {} differs from reference {expected} by more than {tolerance}",
        estimate.mean.amount(),
    );
}

pub fn check_repeat(first: &MoneyEstimate, second: &MoneyEstimate) {
    assert_eq!(
        first.mean.amount().to_bits(),
        second.mean.amount().to_bits()
    );
    assert_eq!(first.stderr.to_bits(), second.stderr.to_bits());
    assert_eq!(
        first.ci_95.0.amount().to_bits(),
        second.ci_95.0.amount().to_bits(),
    );
    assert_eq!(
        first.ci_95.1.amount().to_bits(),
        second.ci_95.1.amount().to_bits(),
    );
    assert_eq!(
        first.std_dev.map(f64::to_bits),
        second.std_dev.map(f64::to_bits)
    );
    assert_eq!(first.num_paths, second.num_paths);
    assert_eq!(first.num_simulated_paths, second.num_simulated_paths);
}

pub fn check_lsq(beta: &[f64], expected: &[f64], design: &[f64], y: &[f64]) {
    assert_eq!(beta.len(), expected.len());
    assert_eq!(design.len(), y.len() * beta.len());
    for (&actual, &target) in beta.iter().zip(expected) {
        assert!(actual.is_finite());
        assert!((actual - target).abs() < 1e-9);
    }
    for (row, &target) in design.chunks_exact(beta.len()).zip(y) {
        let fitted: f64 = row
            .iter()
            .zip(beta)
            .map(|(x, coefficient)| x * coefficient)
            .sum();
        assert!((fitted - target).abs() < 1e-9 * target.abs().max(1.0));
    }
}

/// Independent CRR value for the shared twelve-date Bermudan put fixture.
pub fn monthly_bermudan_put_reference() -> f64 {
    let steps = 1_200;
    let dt = 1.0 / steps as f64;
    let log_up = 0.20 * dt.sqrt();
    let up = log_up.exp();
    let down = (-log_up).exp();
    let probability = ((0.05 * dt - 0.02 * dt).exp() - down) / (up - down);
    assert!((0.0..=1.0).contains(&probability));
    let discount = (-0.05 * dt).exp();
    let mut values: Vec<f64> = (0..=steps)
        .map(|j| {
            let spot = 100.0 * ((2.0 * j as f64 - steps as f64) * log_up).exp();
            (100.0 - spot).max(0.0)
        })
        .collect();
    for step in (0..steps).rev() {
        for j in 0..=step {
            let continuation =
                discount * (probability * values[j + 1] + (1.0 - probability) * values[j]);
            values[j] = if step > 0 && step % (steps / 12) == 0 {
                let spot = 100.0 * ((2.0 * j as f64 - step as f64) * log_up).exp();
                continuation.max((100.0 - spot).max(0.0))
            } else {
                continuation
            };
        }
    }
    assert!(values[0].is_finite() && values[0] > 0.0);
    values[0]
}

pub fn check_rough_first_step(actual: &[f64], dt: f64, z: &[f64]) {
    let alpha = 0.6_f64;
    let initial_variance = 0.04_f64;
    let kernel_integral = dt.powf(alpha) / alpha;
    let bridge_std = dt.powf(0.1) * 0.4 / (alpha * 0.2_f64.sqrt());
    let noise = 0.3 * initial_variance.sqrt() * z[1] * dt.sqrt();
    let volterra = kernel_integral / dt * noise + 0.3 * initial_variance.sqrt() * bridge_std * z[2];
    let expected_variance = (initial_variance
        + (-finstack_quant_core::math::special_functions::ln_gamma(alpha)).exp() * volterra)
        .max(0.0);
    let spot_normal = -0.7 * z[1] + (1.0_f64 - 0.7 * 0.7).sqrt() * z[0];
    let expected_spot = 100.0
        * ((0.03 - 0.5 * initial_variance) * dt + (initial_variance * dt).sqrt() * spot_normal)
            .exp();
    assert_eq!(actual.len(), 2);
    assert!((actual[0] - expected_spot).abs() < 1e-12 * expected_spot);
    assert!((actual[1] - expected_variance).abs() < 1e-12);
}
