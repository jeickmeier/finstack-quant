//! Independent conditional references and routing controls for equal exposures.

use super::*;
use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, HazardCurve};
use finstack_quant_core::math::summation::neumaier_sum;
use std::sync::Arc;
use time::macros::date;

fn count_pmf(probabilities: &[f64]) -> Vec<f64> {
    let mut pmf = vec![f64::NAN; probabilities.len() + 1];
    default_count_pmf(&mut pmf, probabilities.iter().copied());
    pmf
}

fn assert_moments(probabilities: &[f64], pmf: &[f64]) {
    assert_eq!(pmf.len(), probabilities.len() + 1);
    assert!(pmf.iter().all(|p| p.is_finite() && *p >= 0.0));
    assert!((neumaier_sum(pmf.iter().copied()) - 1.0).abs() <= 1e-10);
    if probabilities.is_empty() {
        assert_eq!(pmf, [1.0]);
        return;
    }
    let n = probabilities.len() as f64;
    let mean = neumaier_sum(probabilities.iter().copied()) / n;
    let variance = neumaier_sum(probabilities.iter().map(|p| p * (1.0 - p))) / (n * n);
    let actual_mean = neumaier_sum(pmf.iter().enumerate().map(|(k, p)| p * k as f64 / n));
    let actual_variance = neumaier_sum(
        pmf.iter()
            .enumerate()
            .map(|(k, p)| p * (k as f64 / n - mean).powi(2)),
    );
    assert!((actual_mean - mean).abs() <= 1e-10);
    assert!((actual_variance - variance).abs() <= 1e-10);
}

/// Explicit default subsets, independent of the coefficient recurrence.
fn subset_pmf(probabilities: &[f64]) -> Vec<f64> {
    let mut pmf = vec![0.0; probabilities.len() + 1];
    for state in 0..(1_usize << probabilities.len()) {
        let mass: f64 = probabilities
            .iter()
            .enumerate()
            .map(|(i, p)| if state & (1 << i) == 0 { 1.0 - p } else { *p })
            .product();
        pmf[state.count_ones() as usize] += mass;
    }
    pmf
}

fn assert_subset(probabilities: &[f64]) {
    let pmf = count_pmf(probabilities);
    assert_moments(probabilities, &pmf);
    let reference = subset_pmf(probabilities);
    for (actual, expected) in pmf.iter().zip(reference) {
        assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
    }
}

#[test]
fn default_count_mass_moments_and_reused_endpoint_buffers() {
    for n in [0, 1, 10, 16, 17, 18, 50, 125, 2235] {
        let probabilities: Vec<_> = (0..n)
            .map(|i| match i % 5 {
                0 => 0.0,
                1 => 1e-12,
                2 => 0.01 + 0.97 * i as f64 / n as f64,
                3 => 1.0 - 1e-12,
                _ => 1.0,
            })
            .collect();
        assert_moments(&probabilities, &count_pmf(&probabilities));
    }
    let mut scratch = vec![f64::NAN; 19];
    for defaults in [18, 7, 0, 11] {
        let probabilities: Vec<_> = (0..18)
            .map(|i| if i < defaults { 1.0 } else { 0.0 })
            .collect();
        default_count_pmf(&mut scratch, probabilities.iter().copied());
        for (k, mass) in scratch.iter().enumerate() {
            assert_eq!(*mass, if k == defaults { 1.0 } else { 0.0 });
        }
    }
}

#[test]
fn default_count_matches_subsets_at_transition_sizes_and_correlation_wings() {
    let pricer = CdsTranchePricer::new();
    // N=10 spans both identity seams and the soft wings. Larger pools use
    // selected nodes, keeping the independent 2^N reference bounded.
    for (n, rho, z) in [
        (10, 0.005, -2.0),
        (10, 0.015, 0.0),
        (10, 0.985, 1.5),
        (10, 0.995, -1.0),
        (16, 0.25, -2.0),
        (17, 0.25, 0.0),
        (18, 0.25, 1.5),
    ] {
        let correlation = pricer.smooth_correlation_boundary(rho);
        let probabilities: Vec<_> = (0..n)
            .map(|i| {
                let marginal = match i {
                    0 => 0.0,
                    1 => 1.0,
                    _ => 0.01 + 0.4 * i as f64 / n as f64,
                };
                let threshold = pricer.default_threshold_for_copula(marginal);
                pricer.conditional_default_prob_copula(
                    pricer.copula(),
                    threshold,
                    &[z],
                    correlation,
                )
            })
            .collect();
        assert_subset(&probabilities);
    }
}

/// Closed-form cohort probabilities, without the production binomial helper.
fn cohort_mass(n: usize, k: usize, p: f64) -> f64 {
    let choose = (0..k).fold(1.0, |value, i| value * (n - i) as f64 / (i + 1) as f64);
    choose * p.powi(k as i32) * (1.0 - p).powi((n - k) as i32)
}

#[test]
fn default_count_matches_two_independent_binomial_cohorts() {
    let probabilities: Vec<_> = std::iter::repeat_n(0.03, 60)
        .chain(std::iter::repeat_n(0.72, 65))
        .collect();
    let pmf = count_pmf(&probabilities);
    assert_moments(&probabilities, &pmf);
    for (k, actual) in pmf.iter().enumerate() {
        let expected = neumaier_sum((0..=60).filter_map(|a| {
            let b = k.checked_sub(a)?;
            (b <= 65).then(|| cohort_mass(60, a, 0.03) * cohort_mass(65, b, 0.72))
        }));
        assert!((actual - expected).abs() < 1e-12);
    }
}

#[test]
fn count_caps_match_contractual_subsets_at_kinks_and_recovery_endpoints() {
    let probabilities: Vec<_> = (0..10).map(|i| 0.01 + 0.08 * i as f64).collect();
    let pmf = count_pmf(&probabilities);
    // The accepted explicit total is 1 + 5e-10, so normalizing would change
    // the contractual amounts. Both endpoint recoveries are also exercised.
    let weight = 0.1 + 5e-11;
    for recovery in [0.0, 0.4, 1.0] {
        for exposure in [recovery, 1.0 - recovery] {
            let kink: f64 = 3.0 * weight * exposure;
            let mut caps = vec![0.0, kink, 2.0];
            if kink > 0.0 {
                caps.extend([kink.next_down(), kink.next_up()]);
            }
            for cap in caps {
                let expected = neumaier_sum((0..(1_usize << 10)).map(|state| {
                    let mut mass = 1.0;
                    let mut amount = 0.0;
                    for (i, p) in probabilities.iter().enumerate() {
                        if state & (1 << i) == 0 {
                            mass *= 1.0 - p;
                        } else {
                            mass *= p;
                            amount += weight * exposure;
                        }
                    }
                    mass * amount.min(cap)
                }));
                let actual = count_capped_expectation(&pmf, weight, exposure, cap);
                assert!((actual - expected).abs() < 1e-13);
            }
        }
    }
    let expected = weight * 0.6 * neumaier_sum(probabilities.iter().copied());
    let actual = count_capped_expectation(&pmf, weight, 0.6, 2.0);
    assert!((actual - expected).abs() < 1e-14);
    assert!((actual - expected * (0.1 / weight)).abs() > 1e-11);
}

#[test]
fn count_route_requires_exact_positive_contractual_exposures() {
    assert!(equal_positive_exposures(&[0.5, 0.5], &[0.4, 0.4]));
    assert!(equal_positive_exposures(&[0.5 + 2e-10; 2], &[0.4; 2]));
    assert!(!equal_positive_exposures(
        &[0.5 - 5e-12, 0.5 + 5e-12],
        &[0.4, 0.4]
    ));
    assert!(!equal_positive_exposures(
        &[0.5, 0.5],
        &[0.4 - 5e-12, 0.4 + 5e-12]
    ));
    assert!(!equal_positive_exposures(&[0.0, 0.0], &[0.4, 0.4]));
    assert!(!equal_positive_exposures(&[], &[]));
}

#[test]
fn count_storage_work_and_overflow_are_preflighted() {
    assert_eq!(checked_count_points(0).expect("empty PMF"), 1);
    assert_eq!(checked_count_points(2235).expect("within work cap"), 2236);
    assert!(checked_count_points(2236).is_err());
    assert!(checked_count_points(MAX_GRID_POINTS).is_err());
    assert!(checked_count_points(usize::MAX).is_err());
}

fn index(pds: &[f64], weights: &[f64], recoveries: &[f64]) -> Result<CreditIndexData> {
    let curves: Vec<_> = pds
        .iter()
        .enumerate()
        .map(|(i, pd)| {
            let curve = HazardCurve::builder(format!("COUNT-HZ-{i}"))
                .base_date(date!(2025 - 01 - 01))
                .recovery_rate(0.4)
                .knots([(1.0, -(-pd).ln_1p())])
                .build()?;
            Ok((format!("NAME-{i:05}"), Arc::new(curve)))
        })
        .collect::<Result<_>>()?;
    CreditIndexData::builder()
        .num_constituents(pds.len() as u16)
        .recovery_rate(0.4)
        .index_credit_curve(Arc::clone(&curves[0].1))
        .base_correlation_curve(Arc::new(
            BaseCorrelationCurve::builder("COUNT-BC")
                .knots([(3.0, 0.25), (100.0, 0.25)])
                .build()?,
        ))
        .issuer_curves(curves)
        .issuer_weights(
            weights
                .iter()
                .enumerate()
                .map(|(i, weight)| (format!("NAME-{i:05}"), *weight)),
        )
        .issuer_recovery_rates(
            recoveries
                .iter()
                .enumerate()
                .map(|(i, recovery)| (format!("NAME-{i:05}"), *recovery)),
        )
        .build()
}

fn capped(index: &CreditIndexData, exposure: PoolExposure) -> f64 {
    CdsTranchePricer::new()
        .calculate_equity_tranche_capped_hetero(100.0, 0.25, index, date!(2026 - 01 - 01), exposure)
        .expect("capped expectation")
}

#[test]
fn count_pricing_preserves_explicit_common_weight_and_zero_weight_exclusion() {
    let pds = [0.05, 0.6];
    let normal = index(&pds, &[0.5; 2], &[0.4; 2]).expect("normal weights");
    let actual_weight = 0.5 + 2e-10;
    let explicit = index(&pds, &[actual_weight; 2], &[0.4; 2]).expect("accepted total");
    let mut padded_pds = vec![0.05, 0.6];
    padded_pds.extend(std::iter::repeat_n(0.9, 16));
    let mut padded_weights = vec![0.5, 0.5];
    padded_weights.extend(std::iter::repeat_n(0.0, 16));
    let mut padded_recoveries = vec![0.4, 0.4];
    padded_recoveries.extend(std::iter::repeat_n(0.8, 16));
    let padded = index(&padded_pds, &padded_weights, &padded_recoveries).expect("zero weights");
    let reversed = index(&[0.6, 0.05], &[0.5; 2], &[0.4; 2]).expect("reordered law");
    for exposure in [PoolExposure::Loss, PoolExposure::Recovery] {
        let baseline = capped(&normal, exposure);
        let actual = capped(&explicit, exposure);
        assert!((actual * 0.5 / actual_weight - baseline).abs() < 1e-13);
        assert!((actual - baseline).abs() > 1e-11);
        assert_eq!(capped(&padded, exposure), baseline);
        assert!((capped(&reversed, exposure) - baseline).abs() < 1e-13);
    }
}

#[test]
fn count_route_leaves_true_sub_tolerance_pd_dispersion_homogeneous() {
    let data = index(&[0.1, 0.1 + 5e-10], &[0.5; 2], &[0.4; 2]).expect("pool");
    let pricer = CdsTranchePricer::new();
    let probability = 1.0 - data.index_credit_curve().sp(1.0);
    for exposure in [PoolExposure::Loss, PoolExposure::Recovery] {
        let expected = pricer
            .homogeneous_capped_expectation(1.0, 2, probability, 0.4, 0.25, exposure)
            .expect("existing homogeneous policy");
        assert_eq!(capped(&data, exposure), expected);
    }
}

#[test]
fn count_candidate_preserves_invalid_input_and_old_work_rejections() {
    let valid = index(&[0.05, 0.6], &[0.5; 2], &[0.4; 2]).expect("valid pool");
    assert!(valid.to_builder().num_constituents(0).build().is_err());
    assert!(valid.to_builder().num_constituents(3).build().is_err());
    for weights in [
        vec![0.5],
        vec![0.0, 0.0],
        vec![-0.1, 1.1],
        vec![f64::NAN, 0.5],
        vec![f64::INFINITY, 0.5],
    ] {
        assert!(index(&[0.05, 0.6], &weights, &[0.4; 2]).is_err());
    }
    let pds: Vec<_> = (0..3000)
        .map(|i| if i % 2 == 0 { 0.05 } else { 0.6 })
        .collect();
    let large = index(&pds, &[1.0 / 3000.0; 3000], &[0.4; 3000]).expect("large pool");
    let error = CdsTranchePricer::new()
        .calculate_equity_tranche_capped_hetero(
            7.0,
            0.25,
            &large,
            date!(2026 - 01 - 01),
            PoolExposure::Loss,
        )
        .expect_err("old grid resource preflight remains required");
    assert!(error.to_string().contains("issuer-grid visits"));
    // Zero exposure previously used one grid point, so the count route must
    // not impose its quadratic work limit on this cheap endpoint case.
    for (recovery, exposure) in [(0.0, PoolExposure::Recovery), (1.0, PoolExposure::Loss)] {
        let zero_exposure = large
            .to_builder()
            .issuer_recovery_rates((0..3000).map(|i| (format!("NAME-{i:05}"), recovery)))
            .build()
            .expect("zero-exposure pool");
        assert_eq!(capped(&zero_exposure, exposure), 0.0);
    }
}
