use super::*;
use crate::math::special_functions::standard_normal_inv_cdf;

#[test]
fn range_estimators_use_each_observed_bar_from_the_first() {
    let open: [f64; 3] = [100.0, 101.0, 99.0];
    let high: [f64; 3] = [103.0, 104.0, 102.0];
    let low: [f64; 3] = [98.0, 97.0, 96.0];
    let close: [f64; 3] = [102.0, 99.0, 100.0];
    for method in [
        RealizedVarMethod::Parkinson,
        RealizedVarMethod::GarmanKlass,
        RealizedVarMethod::RogersSatchell,
    ] {
        assert_eq!(
            realized_variance_ohlc(&[], &[], &[], &[], Some(method), Some(252.0), None).unwrap(),
            None
        );
        for n in 1..=3 {
            let sum: f64 = (0..n)
                .map(|i| {
                    let hl = (high[i] / low[i]).ln();
                    let co = (close[i] / open[i]).ln();
                    match method {
                        RealizedVarMethod::Parkinson => hl * hl / (4.0 * std::f64::consts::LN_2),
                        RealizedVarMethod::GarmanKlass => {
                            0.5 * hl * hl - (2.0 * std::f64::consts::LN_2 - 1.0) * co * co
                        }
                        RealizedVarMethod::RogersSatchell => {
                            (high[i] / close[i]).ln() * (high[i] / open[i]).ln()
                                + (low[i] / close[i]).ln() * (low[i] / open[i]).ln()
                        }
                        _ => unreachable!(),
                    }
                })
                .sum();
            let actual = realized_variance_ohlc(
                &open[..n],
                &high[..n],
                &low[..n],
                &close[..n],
                Some(method),
                Some(252.0),
                None,
            )
            .unwrap()
            .unwrap();
            assert!((actual - 252.0 * sum / n as f64).abs() < 1e-12);
            assert!(actual > 0.0);
        }
    }
    let flat = [110.0; 3];
    for method in [
        RealizedVarMethod::CloseToClose,
        RealizedVarMethod::YangZhang,
    ] {
        let estimate = realized_variance_ohlc(
            &flat,
            &flat,
            &flat,
            &flat,
            Some(method),
            Some(252.0),
            Some(100.0),
        )
        .unwrap()
        .unwrap();
        let expected = 252.0 * 1.1_f64.ln().powi(2) / 3.0;
        assert!((estimate - expected).abs() < 1e-12);
        assert!(
            realized_variance_ohlc(&flat, &flat, &flat, &flat, Some(method), Some(252.0), None)
                .is_err()
        );
    }
    assert_eq!(
        realized_variance_ohlc(
            &open[..1],
            &high[..1],
            &low[..1],
            &close[..1],
            Some(RealizedVarMethod::YangZhang),
            Some(252.0),
            Some(100.0)
        )
        .unwrap(),
        None
    );
}

#[test]
fn test_online_stats_basic() {
    let mut stats = OnlineStats::new();
    stats.update(1.0).unwrap();
    stats.update(2.0).unwrap();
    stats.update(3.0).unwrap();

    assert_eq!(stats.get_count(), 3);
    assert_eq!(stats.get_mean().unwrap(), Some(2.0));
    assert_eq!(stats.get_variance().unwrap(), Some(1.0));
    assert_eq!(stats.get_std_dev().unwrap(), Some(1.0));
}

#[test]
fn test_online_stats_merge() {
    let mut stats1 = OnlineStats::new();
    stats1.update(1.0).unwrap();
    stats1.update(2.0).unwrap();

    let mut stats2 = OnlineStats::new();
    stats2.update(3.0).unwrap();
    stats2.update(4.0).unwrap();

    stats1.merge(&stats2).unwrap();
    assert_eq!(stats1.get_count(), 4);
    assert_eq!(stats1.get_mean().unwrap(), Some(2.5));
}

#[test]
fn test_online_stats_merge_empty_accumulators() {
    let mut stats = OnlineStats::new();
    stats.update(1.0).unwrap();
    stats.update(3.0).unwrap();
    let before = stats;

    stats.merge(&OnlineStats::new()).unwrap();
    assert_eq!(stats.get_count(), before.get_count());
    assert_eq!(stats.get_mean().unwrap(), before.get_mean().unwrap());
    assert_eq!(
        stats.get_variance().unwrap(),
        before.get_variance().unwrap()
    );

    let mut empty = OnlineStats::new();
    empty.merge(&stats).unwrap();
    assert_eq!(empty.get_count(), stats.get_count());
    assert_eq!(empty.get_mean().unwrap(), stats.get_mean().unwrap());
    assert_eq!(empty.get_variance().unwrap(), stats.get_variance().unwrap());
}

#[test]
fn confidence_intervals_account_for_estimated_small_sample_variance() {
    let mut pair = OnlineStats::new();
    pair.update(1.0).unwrap();
    pair.update(3.0).unwrap();
    // With one degree of freedom Student-t is Cauchy, giving an
    // independent analytic critical value cot(pi * alpha / 2).
    let critical = 1.0 / (std::f64::consts::PI * 0.025).tan();
    assert!((pair.get_stderr().unwrap().unwrap() - 1.0).abs() < 1e-12);
    assert!((pair.get_ci_half_width(0.05).unwrap().unwrap() - critical).abs() < 1e-10);

    let mut replicates = OnlineStats::new();
    for value in 1..=16 {
        replicates.update(f64::from(value)).unwrap();
    }
    // Tabulated 97.5th percentile for 15 degrees of freedom.
    let expected_half_width = 2.131_449_545_559_323 * replicates.get_stderr().unwrap().unwrap();
    assert!(
        (replicates.get_ci_half_width(0.05).unwrap().unwrap() - expected_half_width).abs() < 1e-10
    );
}

#[test]
fn confidence_intervals_reject_insufficient_samples_and_invalid_alpha() {
    let mut stats = OnlineStats::new();
    for count in 0..=1 {
        if count == 1 {
            stats.update(42.0).unwrap();
        }
        assert_eq!(stats.get_ci_half_width(0.05).unwrap(), None);
        // Invalid alpha must validate BEFORE returning None on n < 2.
        for bad_alpha in [0.0, 1.0, -0.1, 1.1, f64::NAN, f64::INFINITY] {
            assert_eq!(
                stats
                    .get_ci_half_width(bad_alpha)
                    .expect_err("invalid alpha on n<2")
                    .kind(),
                crate::ErrorKind::Validation
            );
        }
    }
    stats.update(43.0).unwrap();
    for alpha in [0.0, 1.0, -0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert_eq!(
            stats.get_ci_half_width(alpha).unwrap_err().kind(),
            crate::ErrorKind::Validation
        );
    }
}

#[test]
fn test_standard_normal_inv_cdf() {
    let z_95 = standard_normal_inv_cdf(0.975).expect("valid probability");
    assert!((z_95 - 1.96).abs() < 0.01);

    let z_99 = standard_normal_inv_cdf(0.995).expect("valid probability");
    assert!((z_99 - 2.576).abs() < 0.01);

    assert!((standard_normal_inv_cdf(0.5).expect("valid probability")).abs() < 0.01);
    assert!(
        (standard_normal_inv_cdf(0.25).expect("valid probability")
            + standard_normal_inv_cdf(0.75).expect("valid probability"))
        .abs()
            < 0.01
    );
}

#[test]
fn covariance_and_correlation_reject_mismatched_lengths_and_nonfinite() {
    let x = [1.0, 2.0, 3.0];
    let y = [1.0, 2.0];
    assert!(matches!(
        covariance(&x, &y),
        Err(crate::Error::Input(crate::InputError::DimensionMismatch))
    ));
    assert!(matches!(
        correlation(&x, &y),
        Err(crate::Error::Input(crate::InputError::DimensionMismatch))
    ));
    // Length 1 with non-finite must error before insufficient-history None.
    assert!(covariance(&[f64::NAN], &[1.0]).is_err());
    assert!(correlation(&[1.0], &[f64::INFINITY]).is_err());
    assert_eq!(covariance(&[1.0], &[2.0]).unwrap(), None);
    assert_eq!(correlation(&[1.0], &[2.0]).unwrap(), None);
}

#[test]
fn variance_matches_online_stats() {
    let datasets: &[&[f64]] = &[
        &[1.0, 2.0, 3.0, 4.0, 5.0],
        &[0.0, 0.0, 0.0, 1.0],
        &[-3.0, -1.0, 0.0, 1.0, 3.0],
        &[1e6, 1e6 + 1.0, 1e6 + 2.0],
        &[1e-12, 2e-12, 3e-12],
    ];
    for xs in datasets {
        let mut s = OnlineStats::new();
        for &x in *xs {
            s.update(x).unwrap();
        }
        let v_batch = variance(xs).unwrap().unwrap();
        let v_online = s.get_variance().unwrap().unwrap();
        let rel_v = (v_batch - v_online).abs() / v_batch.max(1e-30);
        assert!(
            rel_v < 1e-12,
            "variance mismatch for {:?}: {v_batch} vs {v_online}",
            xs
        );

        let pv_batch = population_variance(xs).unwrap().unwrap();
        let pv_online = s.get_population_variance().unwrap().unwrap();
        let rel_pv = (pv_batch - pv_online).abs() / pv_batch.max(1e-30);
        assert!(
            rel_pv < 1e-12,
            "population_variance mismatch for {:?}: {pv_batch} vs {pv_online}",
            xs
        );

        let (m, v) = mean_var(xs).unwrap();
        assert!((m.unwrap() - s.get_mean().unwrap().unwrap()).abs() < 1e-12);
        assert!(
            (v.unwrap() - v_online).abs() / v.unwrap().max(1e-30) < 1e-12,
            "mean_var variance mismatch for {:?}",
            xs
        );
    }
}

#[test]
fn test_online_covariance_basic() {
    let mut cov = OnlineCovariance::new();
    cov.update(1.0, 2.0).unwrap();
    cov.update(2.0, 4.0).unwrap();
    cov.update(3.0, 6.0).unwrap();

    assert_eq!(cov.get_count(), 3);
    assert_eq!(cov.get_mean_x().unwrap(), Some(2.0));
    assert_eq!(cov.get_mean_y().unwrap(), Some(4.0));
    assert!((cov.get_correlation().unwrap().unwrap() - 1.0).abs() < 1e-10);
}

#[test]
fn test_online_covariance_negative() {
    let mut cov = OnlineCovariance::new();
    cov.update(1.0, 6.0).unwrap();
    cov.update(2.0, 4.0).unwrap();
    cov.update(3.0, 2.0).unwrap();

    assert!((cov.get_correlation().unwrap().unwrap() + 1.0).abs() < 1e-10);
}

#[test]
fn test_online_covariance_uncorrelated() {
    let mut cov = OnlineCovariance::new();
    cov.update(1.0, 1.0).unwrap();
    cov.update(1.0, -1.0).unwrap();
    cov.update(-1.0, 1.0).unwrap();
    cov.update(-1.0, -1.0).unwrap();

    assert!(cov.get_correlation().unwrap().unwrap().abs() < 1e-10);
}

#[test]
fn test_online_covariance_merge() {
    let mut cov1 = OnlineCovariance::new();
    cov1.update(1.0, 2.0).unwrap();
    cov1.update(2.0, 4.0).unwrap();

    let mut cov2 = OnlineCovariance::new();
    cov2.update(3.0, 6.0).unwrap();
    cov2.update(4.0, 8.0).unwrap();

    cov1.merge(&cov2).unwrap();

    assert_eq!(cov1.get_count(), 4);
    assert_eq!(cov1.get_mean_x().unwrap(), Some(2.5));
    assert_eq!(cov1.get_mean_y().unwrap(), Some(5.0));
    assert!((cov1.get_correlation().unwrap().unwrap() - 1.0).abs() < 1e-10);
}

#[test]
fn test_online_covariance_matches_batch() {
    let data: Vec<(f64, f64)> = vec![(1.0, 2.1), (2.0, 3.9), (3.0, 6.2), (4.0, 7.8), (5.0, 10.1)];

    let mut online = OnlineCovariance::new();
    for &(x, y) in &data {
        online.update(x, y).unwrap();
    }

    let n = data.len() as f64;
    let mean_x: f64 = data.iter().map(|(x, _)| x).sum::<f64>() / n;
    let mean_y: f64 = data.iter().map(|(_, y)| y).sum::<f64>() / n;
    let cov_batch: f64 = data
        .iter()
        .map(|(x, y)| (x - mean_x) * (y - mean_y))
        .sum::<f64>()
        / (n - 1.0);

    assert!((online.get_mean_x().unwrap().unwrap() - mean_x).abs() < 1e-10);
    assert!((online.get_mean_y().unwrap().unwrap() - mean_y).abs() < 1e-10);
    assert!((online.get_covariance().unwrap().unwrap() - cov_batch).abs() < 1e-10);
}

#[test]
fn test_online_covariance_optimal_beta() {
    let mut cov = OnlineCovariance::new();
    cov.update(1.0, 2.0).unwrap();
    cov.update(2.0, 4.0).unwrap();
    cov.update(3.0, 6.0).unwrap();

    assert!((cov.get_optimal_beta().unwrap().unwrap() - 0.5).abs() < 1e-10);
}

#[test]
fn test_online_covariance_tiny_and_huge_scale_perfect_correlation() {
    for scale in [2.0_f64.powi(-270), 1.0e-12, 2.0_f64.powi(256)] {
        let mut cov = OnlineCovariance::new();
        cov.update(-scale, -scale).unwrap();
        cov.update(scale, scale).unwrap();
        let r = cov.get_correlation().unwrap().unwrap();
        assert!(
            (r - 1.0).abs() < 1e-12,
            "scale {scale}: expected 1.0, got {r}"
        );

        let xs = [-scale, scale];
        assert_eq!(correlation(&xs, &xs).unwrap(), Some(1.0));
    }
}

#[test]
fn test_constant_series_availability_contracts() {
    let mut cov = OnlineCovariance::new();
    cov.update(5.0, 1.0).unwrap();
    cov.update(5.0, 2.0).unwrap();
    cov.update(5.0, 3.0).unwrap();
    assert_eq!(cov.get_covariance().unwrap(), Some(0.0));
    assert_eq!(cov.get_correlation().unwrap(), None);
    assert_eq!(cov.get_optimal_beta().unwrap(), Some(0.0));

    assert_eq!(
        covariance(&[5.0, 5.0, 5.0], &[1.0, 2.0, 3.0]).unwrap(),
        Some(0.0)
    );
    assert_eq!(
        correlation(&[5.0, 5.0, 5.0], &[1.0, 2.0, 3.0]).unwrap(),
        None
    );
}

// ── log_returns tests ──

#[test]
fn log_returns_empty_and_single_yield_empty() {
    assert!(super::log_returns(&[]).unwrap().is_empty());
    assert!(super::log_returns(&[100.0]).unwrap().is_empty());
}

#[test]
fn log_returns_matches_ln_ratio_for_positive_prices() {
    let lr = super::log_returns(&[100.0, 110.0, 99.0]).unwrap();
    assert_eq!(lr.len(), 2);
    assert!((lr[0] - 1.1_f64.ln()).abs() < 1e-12);
    assert!((lr[1] - (99.0_f64 / 110.0).ln()).abs() < 1e-12);
}

#[test]
fn log_returns_rejects_non_positive_or_non_finite_prices() {
    assert!(super::log_returns(&[0.0]).is_err());
    let extremes = super::log_returns(&[1e-300, 1e300, 1e-300]).unwrap();
    assert!((extremes[0] - 600.0 * f64::ln(10.0)).abs() < 1e-12);
    assert_eq!(extremes[0], -extremes[1]);
    let near = super::log_returns(&[1e300, 1e300_f64.next_up()]).unwrap();
    assert!(near[0] > 0.0);
    assert!(super::log_returns(&[100.0, 0.0]).is_err());
    assert!(super::log_returns(&[0.0, 100.0]).is_err());
    assert!(super::log_returns(&[100.0, f64::NAN]).is_err());
    assert!(super::log_returns(&[f64::NAN, 100.0]).is_err());
}

// ── quantile & median tests ──

#[test]
fn quantile_empty_returns_none_and_invalid_p_on_empty_errors() {
    let mut data: Vec<f64> = vec![];
    assert_eq!(super::quantile(&mut data, 0.5).unwrap(), None);
    assert_eq!(super::quantile_sorted(&[], 0.5).unwrap(), None);
    assert_eq!(super::median(&[]).unwrap(), None);
    for p in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert_eq!(
            super::quantile(&mut data, p).unwrap_err().kind(),
            crate::ErrorKind::Validation
        );
        assert_eq!(
            super::quantile_sorted(&[], p).unwrap_err().kind(),
            crate::ErrorKind::Validation
        );
    }
}

#[test]
fn quantile_single_element() {
    let mut data = vec![42.0];
    assert_eq!(super::quantile(&mut data, 0.0).unwrap(), Some(42.0));
    assert_eq!(super::quantile(&mut data, 0.5).unwrap(), Some(42.0));
    assert_eq!(super::quantile(&mut data, 1.0).unwrap(), Some(42.0));
}

#[test]
fn quantile_p0_and_p1_and_median_variants() {
    let mut data = vec![5.0, 1.0, 3.0, 2.0, 4.0];
    assert_eq!(super::quantile(&mut data, 0.0).unwrap(), Some(1.0));
    assert_eq!(super::quantile(&mut data, 1.0).unwrap(), Some(5.0));
    assert_eq!(super::quantile(&mut data, 0.5).unwrap(), Some(3.0));
    assert_eq!(
        super::median(&[3.0, 1.0, 2.0, 5.0, 4.0]).unwrap(),
        Some(3.0)
    );

    let mut even = vec![1.0, 2.0, 3.0, 4.0];
    assert_eq!(super::quantile(&mut even, 0.25).unwrap(), Some(1.75));
    assert_eq!(super::quantile(&mut even, 0.5).unwrap(), Some(2.5));
    assert_eq!(super::median(&[1.0, 2.0, 3.0, 4.0]).unwrap(), Some(2.5));
}

#[test]
fn quantile_and_sorted_reject_non_finite_unsorted_and_underflow() {
    let mut data = vec![1.0, f64::NAN, 3.0];
    assert_eq!(
        super::quantile(&mut data, 0.5).unwrap_err().kind(),
        crate::ErrorKind::Validation
    );
    // quantile_sorted validates entire slice including unselected entries and ordering.
    assert!(super::quantile_sorted(&[2.0, 1.0], 0.5).is_err());
    assert!(super::quantile_sorted(&[0.0, 1.0, 3.0, 2.0], 0.0).is_err());
    assert!(super::quantile_sorted(&[0.0, 1.0, f64::NAN], 0.0).is_err());
    // Nonzero mathematical R7 quantile that rounds to 0 errors with Computation/Internal.
    let tiny = f64::from_bits(1);
    let mut underflow = [0.0, tiny];
    assert_eq!(
        super::quantile(&mut underflow, 0.25).unwrap_err().kind(),
        crate::ErrorKind::Computation
    );
    assert_eq!(
        super::quantile_sorted(&[0.0, tiny], 0.25)
            .unwrap_err()
            .kind(),
        crate::ErrorKind::Computation
    );
}

#[test]
fn finite_filters_behave_separately_from_ordinary_estimators() {
    let values = [f64::NAN, f64::INFINITY, 3.0, -2.0];
    assert_eq!(super::finite_min(&values), Some(-2.0));
    assert_eq!(super::finite_max(&values), Some(3.0));
    assert_eq!(super::finite_count(&values), 2);

    assert_eq!(super::finite_min(&[f64::NAN, f64::NEG_INFINITY]), None);
    assert_eq!(super::finite_max(&[f64::NAN, f64::INFINITY]), None);
    assert_eq!(super::finite_count(&[f64::NAN]), 0);

    assert!(mean(&values).is_err());
    assert!(variance(&values).is_err());
}

#[test]
fn quantile_preserves_a_rank_residual_rounded_away_by_multiplication() {
    let sorted = [-2.0_f64.powi(53), 0.0, 1.0, 2.0];
    assert_eq!(
        super::quantile_sorted(&sorted, 1.0 / 3.0).unwrap(),
        Some(-0.5)
    );
    let mut data = sorted;
    assert_eq!(super::quantile(&mut data, 1.0 / 3.0).unwrap(), Some(-0.5));
}

#[test]
fn quantile_interpolates_opposite_extremes_without_overflow() {
    let sorted = [-f64::MAX, f64::MAX];
    assert_eq!(super::quantile_sorted(&sorted, 0.5).unwrap(), Some(0.0));
    let p = 0.5 + f64::EPSILON;
    assert_eq!(
        super::quantile_sorted(&sorted, p).unwrap(),
        Some(f64::MAX * (2.0 * p - 1.0))
    );
}

#[test]
fn quantile_ratio_preserves_unrepresentable_components_and_rounding_residuals() {
    let tiny = f64::from_bits(1);
    assert_eq!(
        super::quantile_ratio(&[0.0, tiny], 0.75, 0.25).expect("joint ratio"),
        3.0
    );
    assert_eq!(
        super::quantile_ratio(&[-1.0, -1.0 / 3.0, -tiny, 0.0], 0.875, 0.125)
            .expect("ratio above underflow midpoint"),
        tiny
    );
    assert!(super::quantile_checked(&[0.0, tiny], 0.25).is_err());
    assert_eq!(
        super::quantile_checked(&[0.0, tiny], 0.75).expect("rounded nonzero"),
        tiny
    );
}

#[test]
fn empirical_tail_mean_preserves_fractional_mass_and_subnormal_constants() {
    let mut sorted = [2.0_f64.powi(53); 10];
    sorted[0] = 0.0;
    assert_eq!(super::empirical_tail_mean(&sorted, 0.1).expect("tail"), 0.5);
    let tiny = f64::from_bits(1);
    assert_eq!(
        super::empirical_tail_mean(&[tiny; 5], 0.4).expect("tiny constant"),
        tiny
    );
    assert!(super::empirical_tail_mean(&[0.0, tiny], 1.0).is_err());
}

#[test]
fn checked_quantiles_and_tail_means_reject_invalid_requests_and_samples() {
    for sorted in [&[][..], &[1.0, 0.0], &[0.0, f64::INFINITY]] {
        assert!(super::quantile_ratio(sorted, 0.75, 0.25).is_err());
        assert!(super::quantile_checked(sorted, 0.5).is_err());
        assert!(super::empirical_tail_mean(sorted, 0.5).is_err());
    }
    for p in [f64::NAN, -0.1, 1.1] {
        assert!(super::quantile_ratio(&[0.0, 1.0], p, 0.25).is_err());
        assert!(super::quantile_ratio(&[0.0, 1.0], 0.75, p).is_err());
        assert!(super::quantile_checked(&[0.0, 1.0], p).is_err());
        assert!(super::empirical_tail_mean(&[0.0, 1.0], p).is_err());
    }
    assert!(super::empirical_tail_mean(&[0.0, 1.0], 0.0).is_err());
}

// ── RealizedVarMethod FromStr / Display roundtrip ──

#[test]
fn realized_var_method_label_is_the_serde_name() {
    use super::RealizedVarMethod;

    for method in [
        RealizedVarMethod::CloseToClose,
        RealizedVarMethod::Parkinson,
        RealizedVarMethod::GarmanKlass,
        RealizedVarMethod::RogersSatchell,
        RealizedVarMethod::YangZhang,
    ] {
        let label = crate::wire::serde_label(&method).expect("label");
        assert_eq!(label, method.to_string());
        let parsed: RealizedVarMethod = crate::wire::serde_parse(&label).expect("parse");
        assert_eq!(parsed, method);
    }
    assert!(crate::wire::serde_parse::<RealizedVarMethod>("YangZhang").is_err());
    assert!(crate::wire::serde_parse::<RealizedVarMethod>("garbage").is_err());
}

#[test]
fn realized_variance_rejects_invalid_ohlc_and_annualization() {
    use super::{RealizedVarMethod, realized_variance, realized_variance_ohlc};

    let open = [100.0, 101.0];
    let high = [102.0, 103.0];
    let low = [99.0, 100.0];
    let close = [101.0, 102.0];
    assert!(
        realized_variance_ohlc(
            &open,
            &high,
            &[0.0, 100.0],
            &close,
            Some(RealizedVarMethod::Parkinson),
            Some(252.0),
            None,
        )
        .is_err()
    );
    assert!(
        realized_variance_ohlc(
            &[f64::NAN, 101.0],
            &high,
            &low,
            &close,
            Some(RealizedVarMethod::GarmanKlass),
            Some(252.0),
            None,
        )
        .is_err()
    );
    assert!(
        realized_variance_ohlc(
            &open,
            &[98.0, 103.0],
            &low,
            &close,
            Some(RealizedVarMethod::RogersSatchell),
            Some(252.0),
            None,
        )
        .is_err()
    );
    assert!(
        realized_variance_ohlc(
            &open,
            &high,
            &low,
            &close,
            Some(RealizedVarMethod::YangZhang),
            Some(f64::INFINITY),
            Some(100.0),
        )
        .is_err()
    );
    assert!(realized_variance(&close, Some(-1.0)).is_err());
}

#[test]
fn ohlc_estimators_support_wide_prices_and_positive_subnormal_variance() {
    use super::{RealizedVarMethod, realized_variance, realized_variance_ohlc};
    let smallest = f64::from_bits(1);
    let tiny = realized_variance(&[1.0, 16.0, 16.0, 16.0, 16.0], Some(smallest)).unwrap();
    assert_eq!(tiny, Some(f64::from_bits(2)));
    assert!(realized_variance(&[1.0, 1.0001], Some(smallest)).is_err());
    let log_body = 600.0 * 10.0_f64.ln();
    for (method, expected) in [
        (
            RealizedVarMethod::GarmanKlass,
            (1.5 - 2.0 * f64::ln(2.0)) * log_body * log_body,
        ),
        (RealizedVarMethod::RogersSatchell, 0.0),
        (
            RealizedVarMethod::Parkinson,
            log_body * log_body / (4.0 * f64::ln(2.0)),
        ),
    ] {
        let value = realized_variance_ohlc(
            &[1e-300],
            &[1e300],
            &[1e-300],
            &[1e300],
            Some(method),
            Some(1.0),
            None,
        )
        .unwrap()
        .unwrap();
        assert!((value - expected).abs() <= 1e-14 * expected);
    }
}

#[test]
fn realized_variance_defaults_are_rust_owned() {
    use super::{RealizedVarMethod, realized_variance, realized_variance_ohlc};

    let open = [100.0, 101.5, 100.8, 102.0];
    let high = [102.0, 103.0, 102.5, 103.5];
    let low = [99.0, 100.2, 99.9, 101.0];
    let close = [101.0, 102.0, 101.5, 103.0];
    let daily = f64::from(crate::dates::PeriodKind::Daily.annualization_factor());
    assert_eq!(daily, 252.0);

    assert_eq!(
        realized_variance(&close, None).expect("defaults"),
        realized_variance(&close, Some(daily)).expect("explicit"),
    );
    assert_eq!(
        RealizedVarMethod::OHLC_DEFAULT,
        RealizedVarMethod::YangZhang
    );
    assert_eq!(
        realized_variance_ohlc(&open, &high, &low, &close, None, None, Some(100.0))
            .expect("defaults"),
        realized_variance_ohlc(
            &open,
            &high,
            &low,
            &close,
            Some(RealizedVarMethod::YangZhang),
            Some(daily),
            Some(100.0),
        )
        .expect("explicit"),
    );
}

#[test]
fn shifted_scaled_boundary_rows_from_decision_record() {
    // Empty & singleton availability.
    assert_eq!(mean(&[]).unwrap(), None);
    assert_eq!(population_variance(&[]).unwrap(), None);
    assert_eq!(variance(&[]).unwrap(), None);
    assert_eq!(std_dev(&[]).unwrap(), None);
    assert_eq!(mean_var(&[]).unwrap(), (None, None));
    assert_eq!(mean(&[42.0]).unwrap(), Some(42.0));
    assert_eq!(population_variance(&[42.0]).unwrap(), Some(0.0));
    assert_eq!(variance(&[42.0]).unwrap(), None);
    assert_eq!(std_dev(&[42.0]).unwrap(), None);
    assert_eq!(mean_var(&[42.0]).unwrap(), (Some(42.0), None));

    // Constant n >= 2 gives 0 variance/SD/SE and point CI without inverse-t.
    let mut cst = OnlineStats::new();
    cst.update(f64::MAX).unwrap();
    cst.update(f64::MAX).unwrap();
    assert_eq!(cst.get_mean().unwrap(), Some(f64::MAX));
    assert_eq!(cst.get_variance().unwrap(), Some(0.0));
    assert_eq!(cst.get_std_dev().unwrap(), Some(0.0));
    assert_eq!(cst.get_stderr().unwrap(), Some(0.0));
    assert_eq!(cst.get_ci_half_width(0.05).unwrap(), Some(0.0));
    // Merge equal MAX constants without weighted mean overflow.
    let mut cst2 = OnlineStats::new();
    cst2.update(f64::MAX).unwrap();
    cst.merge(&cst2).unwrap();
    assert_eq!(cst.get_count(), 3);
    assert_eq!(cst.get_mean().unwrap(), Some(f64::MAX));

    // [1e16, 1e16 + 2] exact variance 2.0 (shifted coordinates avoid raw Welford's 4.0).
    let shifted = [1.0e16, 1.0e16 + 2.0];
    assert_eq!(variance(&shifted).unwrap(), Some(2.0));
    let mut s_shifted = OnlineStats::new();
    s_shifted.update(shifted[0]).unwrap();
    s_shifted.update(shifted[1]).unwrap();
    assert_eq!(s_shifted.get_variance().unwrap(), Some(2.0));

    // [nextUp(1.0), 1.0]: genuine variation, never treated as constant zero M2.
    let next_up = f64::from_bits(1.0_f64.to_bits() + 1);
    assert!(variance(&[next_up, 1.0]).unwrap().unwrap() > 0.0);
    let mut s_tiny_diff = OnlineStats::new();
    s_tiny_diff.update(next_up).unwrap();
    s_tiny_diff.update(1.0).unwrap();
    assert!(s_tiny_diff.get_variance().unwrap().unwrap() > 0.0);

    // +/- 2^511 twice each -> pop var 2^1022, sample var (4/3)*2^1022.
    let p511 = 2.0_f64.powi(511);
    let xs_511 = [p511, -p511, p511, -p511];
    assert_eq!(
        population_variance(&xs_511).unwrap(),
        Some(2.0_f64.powi(1022))
    );
    let sv_511 = variance(&xs_511).unwrap().unwrap();
    let expected_sv = (4.0 / 3.0) * 2.0_f64.powi(1022);
    assert!((sv_511 / expected_sv - 1.0).abs() < 1e-14);

    // [-MAX, +MAX] at n = 2 -> mean=0, SE=MAX, variance & SD overflow to Computation error.
    let mut extremes = OnlineStats::new();
    extremes.update(-f64::MAX).unwrap();
    extremes.update(f64::MAX).unwrap();
    assert_eq!(extremes.get_mean().unwrap(), Some(0.0));
    assert_eq!(extremes.get_stderr().unwrap(), Some(f64::MAX));
    assert_eq!(
        extremes.get_variance().unwrap_err().kind(),
        crate::ErrorKind::Computation
    );
    assert_eq!(
        extremes.get_std_dev().unwrap_err().kind(),
        crate::ErrorKind::Computation
    );

    // Tiny varying observations [0, 1e-200]: variance underflows -> error; mean & SD succeed.
    assert_eq!(
        variance(&[0.0, 1e-200]).unwrap_err().kind(),
        crate::ErrorKind::Computation
    );
    assert!((mean(&[0.0, 1e-200]).unwrap().unwrap() - 0.5e-200).abs() < 1e-215);
    assert!(std_dev(&[0.0, 1e-200]).unwrap().unwrap() > 0.0);

    // Atomic error behavior leaves accumulator unchanged.
    let mut s = OnlineStats::new();
    s.update(1.0).unwrap();
    let snap = s;
    assert!(s.update(f64::NAN).is_err());
    assert_eq!(s, snap);

    let mut c = OnlineCovariance::new();
    c.update(1.0, 2.0).unwrap();
    let c_snap = c;
    assert!(c.update(1.0, f64::INFINITY).is_err());
    assert_eq!(c, c_snap);

    // Collapsed / overflowing CI fails interval while standalone half-width succeeds.
    let mut big_base = OnlineStats::new();
    big_base.update(f64::MAX - 2.0e293).unwrap();
    big_base.update(f64::MAX).unwrap();
    assert!(
        big_base
            .get_ci_half_width(0.05)
            .unwrap()
            .unwrap()
            .is_finite()
    );

    let mut huge_shift = OnlineStats::new();
    huge_shift.update(1.0e12).unwrap();
    huge_shift.update(1.0e12 + 1.0).unwrap();
    let h_small = huge_shift.get_ci_half_width(0.9999999).unwrap().unwrap();
    assert!(h_small > 0.0 && h_small.is_finite());

    // Tiny alpha underflowing alpha/2 fails as computation error.
    let mut two = OnlineStats::new();
    two.update(1.0).unwrap();
    two.update(3.0).unwrap();
    assert_eq!(
        two.get_ci_half_width(f64::from_bits(1)).unwrap_err().kind(),
        crate::ErrorKind::Computation
    );
}

#[test]
fn online_stats_and_covariance_merge_without_weighted_mean_overflow() {
    let mut a = super::OnlineStats::new();
    let mut b = super::OnlineStats::new();
    let mut cov_a = super::OnlineCovariance::new();
    let mut cov_b = super::OnlineCovariance::new();
    for _ in 0..1000 {
        a.update(1.0e306).unwrap();
        b.update(1.0e306).unwrap();
        cov_a.update(1.0e306, -1.0e306).unwrap();
        cov_b.update(1.0e306, -1.0e306).unwrap();
    }
    a.merge(&b).unwrap();
    cov_a.merge(&cov_b).unwrap();
    let m = a.get_mean().unwrap().unwrap();
    assert!(m.is_finite());
    assert!((m - 1.0e306).abs() <= 1.0e292);
    assert!(cov_a.get_mean_x().unwrap().unwrap().is_finite());
    assert!(cov_a.get_mean_y().unwrap().unwrap().is_finite());
}

#[test]
fn mean_and_correlation_boundary_cases() {
    let mut mixed = Vec::with_capacity(257);
    mixed.push(1.0e16);
    mixed.extend(std::iter::repeat_n(1.0, 256));
    let expected_mixed = (1.0e16 + 256.0) / 257.0;
    assert!((mean(&mixed).unwrap().unwrap() - expected_mixed).abs() < 1e-12);

    let offset = [1.0e16, 1.0e16 + 2.0, 1.0e16 + 4.0];
    assert!((mean(&offset).unwrap().unwrap() - (1.0e16 + 2.0)).abs() <= 2.0);
    assert_eq!(mean(&[f64::MAX, f64::MAX]).unwrap(), Some(f64::MAX));

    // 0.1875 collinearity where sqrt(m2)^2 rounds down below c_s.
    let skew = [0.0, 0.0, 0.0, 1.0];
    assert_eq!(correlation(&skew, &skew).unwrap(), Some(1.0));
    let neg_skew = [0.0, 0.0, 0.0, -1.0];
    assert_eq!(correlation(&skew, &neg_skew).unwrap(), Some(-1.0));

    let mut oc = OnlineCovariance::new();
    for (&x, &y) in skew.iter().zip(&skew) {
        oc.update(x, y).unwrap();
    }
    assert_eq!(oc.get_correlation().unwrap(), Some(1.0));
}
