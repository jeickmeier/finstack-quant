use super::capture::*;
use super::regression::*;
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::math::special_functions::student_t_inv_cdf;
use finstack_quant_core::math::summation::neumaier_sum;
#[test]
fn non_finite_field_lists_match_serde_sentinels() {
    use crate::test_support::assert_non_finite_fields;
    let beta = BetaResult {
        beta: f64::NAN,
        std_err: f64::NAN,
        ci_lower: f64::NAN,
        ci_upper: f64::NAN,
        n_obs: 0,
        confidence_level: BETA_CONFIDENCE_LEVEL,
    };
    assert_non_finite_fields(&beta);
    let greeks = GreeksResult {
        alpha: f64::NAN,
        beta: f64::NAN,
        r_squared: f64::NAN,
        adjusted_r_squared: f64::NAN,
        n_obs: 0,
    };
    assert_non_finite_fields(&greeks);
}

#[test]
fn regression_results_report_the_aligned_sample_size() {
    let port = [0.020, 0.042, 0.058, 0.081, 0.099];
    let bench = [0.010, 0.020, 0.030, 0.040, 0.050];
    let fit = beta(&port, &bench).expect("fit");
    assert_eq!(fit.n_obs, 5);
    assert_eq!(fit.confidence_level, 0.95);
    let critical = student_t_inv_cdf(0.975, 3.0).expect("t quantile");
    assert!((fit.ci_lower - (fit.beta - critical * fit.std_err)).abs() < 1e-12);
    assert!((fit.ci_upper - (fit.beta + critical * fit.std_err)).abs() < 1e-12);
    let short = beta(&port[..2], &bench[..2]).expect("two pairs");
    assert_eq!(short.n_obs, 2);
    assert!(short.beta.is_finite());
    assert!(short.std_err.is_nan());
    assert_eq!(
        greeks(&port[..1], &bench[..1], 252.0, 0.0)
            .expect("one pair")
            .n_obs,
        1
    );
}

fn treynor(annual_mean: f64, cash: f64, slope: f64, annualization: f64) -> Result<f64> {
    let mean = annual_mean / annualization;
    let b = [-0.01, 0.0, 0.01];
    let p = [mean - 0.01 * slope, mean, mean + 0.01 * slope];
    treynor_from_returns(&p, &b, cash, annualization)
}

use time::{Duration, Month};

fn jan(day: u8) -> Date {
    Date::from_calendar_date(2025, Month::January, day).expect("valid date")
}

#[test]
fn tracking_error_zero_when_identical() {
    let r = [0.01, 0.02, -0.01, 0.03];
    let te = tracking_error(&r, &r, 252.0).expect("valid benchmark request");
    assert!(te.abs() < 1e-12);
}

#[test]
fn tracking_error_rejects_invalid_frequency() {
    for factor in [0.0, -1.0, f64::NAN] {
        assert!(tracking_error(&[0.01, 0.02], &[0.01, 0.01], factor).is_err());
    }
}

#[test]
fn information_ratio_basic() {
    let r = [0.02, 0.03, 0.01, 0.04];
    let b = [0.01, 0.01, 0.01, 0.01];
    let ir = information_ratio(&r, &b, 252.0).expect("valid benchmark request");
    assert!(ir > 0.0);
}

#[test]
fn information_ratio_rejects_invalid_frequency() {
    for factor in [0.0, f64::INFINITY] {
        assert!(information_ratio(&[0.01, 0.02], &[0.01, 0.01], factor).is_err());
    }
}

#[test]
fn r_squared_perfect_correlation() {
    let r = [1.0, 2.0, 3.0, 4.0];
    let b = [2.0, 4.0, 6.0, 8.0];
    let r2 = r_squared(&r, &b).expect("valid benchmark request");
    assert!((r2 - 1.0).abs() < 1e-10);
}

#[test]
fn regression_statistics_are_nan_without_two_overlapping_observations() {
    for (returns, benchmark) in [(&[][..], &[][..]), (&[0.02][..], &[0.01][..])] {
        let beta_result = beta(returns, benchmark).expect("valid benchmark request");
        assert!(beta_result.beta.is_nan());
        assert!(beta_result.std_err.is_nan());
        assert!(beta_result.ci_lower.is_nan());
        assert!(beta_result.ci_upper.is_nan());

        assert!(
            beta(returns, benchmark)
                .map(|fit| fit.beta)
                .expect("valid benchmark request")
                .is_nan()
        );
        assert!(
            r_squared(returns, benchmark)
                .expect("valid benchmark request")
                .is_nan()
        );

        let greeks_result =
            greeks(returns, benchmark, 252.0, 0.0).expect("valid benchmark request");
        assert!(greeks_result.alpha.is_nan());
        assert!(greeks_result.beta.is_nan());
        assert!(greeks_result.r_squared.is_nan());
        assert!(greeks_result.adjusted_r_squared.is_nan());
    }
}

#[test]
fn degenerate_beta_and_greeks_json_round_trip_nan_fields() {
    let beta_result = beta(&[], &[]).expect("valid benchmark request");
    let beta_json = serde_json::to_string(&beta_result).expect("beta result should serialize");
    let beta_round_trip: BetaResult =
        serde_json::from_str(&beta_json).expect("beta result should deserialize");
    assert!(beta_round_trip.beta.is_nan());
    assert!(beta_round_trip.std_err.is_nan());
    assert!(beta_round_trip.ci_lower.is_nan());
    assert!(beta_round_trip.ci_upper.is_nan());

    let greeks_result = greeks(&[], &[], 252.0, 0.0).expect("valid benchmark request");
    let greeks_json =
        serde_json::to_string(&greeks_result).expect("greeks result should serialize");
    let greeks_round_trip: GreeksResult =
        serde_json::from_str(&greeks_json).expect("greeks result should deserialize");
    assert!(greeks_round_trip.alpha.is_nan());
    assert!(greeks_round_trip.beta.is_nan());
    assert!(greeks_round_trip.r_squared.is_nan());
    assert!(greeks_round_trip.adjusted_r_squared.is_nan());
}

#[test]
fn beta_basic() {
    let y = [0.02, 0.04, 0.06, 0.08, 0.10];
    let x = [0.01, 0.02, 0.03, 0.04, 0.05];
    let result = beta(&y, &x).expect("valid benchmark request");
    assert!((result.beta - 2.0).abs() < 1e-10);
}

#[test]
fn beta_point_estimate_supports_two_pairs_without_inference() {
    let returns = [0.03, 0.05];
    let benchmark = [0.01, 0.02];

    assert!(
        (beta(&returns, &benchmark)
            .map(|fit| fit.beta)
            .expect("valid benchmark request")
            - 2.0)
            .abs()
            < 1e-12
    );

    let result = beta(&returns, &benchmark).expect("valid benchmark request");
    assert!((result.beta - 2.0).abs() < 1e-12);
    assert!(result.std_err.is_nan());
    assert!(result.ci_lower.is_nan());
    assert!(result.ci_upper.is_nan());

    let greeks_result = greeks(&returns, &benchmark, 12.0, 0.0).expect("valid benchmark request");
    assert!((greeks_result.alpha - 0.12).abs() < 1e-12);
    assert!((greeks_result.beta - 2.0).abs() < 1e-12);
    assert!((greeks_result.r_squared - 1.0).abs() < 1e-12);
    assert!(greeks_result.adjusted_r_squared.is_nan());
}

#[test]
fn batch_regression_rejects_mismatched_or_nonfinite_observations() {
    let returns = [0.01, 0.03, 0.05, 99.0];
    let benchmark = [0.0, 0.01, 0.02];
    assert!(beta(&returns, &benchmark).is_err());
    assert!(beta(&returns, &benchmark).map(|fit| fit.beta).is_err());
    assert!(r_squared(&returns, &benchmark).is_err());
    assert!(greeks(&returns, &benchmark, 12.0, 0.0).is_err());
    assert!(beta(&[0.0, f64::NAN], &[0.0, 1.0]).is_err());
}

#[test]
fn beta_uses_t_critical_value_for_small_samples() {
    let y = [0.020, 0.041, 0.059, 0.082, 0.099];
    let x = [0.010, 0.020, 0.030, 0.040, 0.050];
    let result = beta(&y, &x).expect("valid benchmark request");
    let expected_t_critical_df3 = 3.182_446_305_284_263_f64;
    let expected_half_width = expected_t_critical_df3 * result.std_err;
    let actual_half_width = result.ci_upper - result.beta;

    assert!(
        (actual_half_width - expected_half_width).abs() < 1e-12,
        "small-sample beta CI should use Student-t critical value: expected {}, got {}",
        expected_half_width,
        actual_half_width
    );
}

#[test]
fn beta_uses_t_critical_value_beyond_df_37() {
    let x: Vec<f64> = (0..42).map(|i| -0.02 + i as f64 * 0.001).collect();
    let y: Vec<f64> = x
        .iter()
        .enumerate()
        .map(|(i, &b)| 1.4 * b + if i % 2 == 0 { 0.002 } else { -0.0015 })
        .collect();
    let result = beta(&y, &x).expect("valid benchmark request");
    // n = 42 → df = 40: use the actual degrees of freedom, not a bucket.
    let expected_t_critical_df40 = 2.021_075_390_306_273_3_f64;
    let actual_half_width = result.ci_upper - result.beta;

    assert!(
        (actual_half_width - expected_t_critical_df40 * result.std_err).abs() < 1e-12,
        "beta CI should continue using Student-t beyond df=37"
    );
}

#[test]
fn beta_intervals_cover_the_student_t_probability_at_bucket_boundaries() {
    use finstack_quant_core::math::special_functions::student_t_cdf;

    for df in [38, 59, 60, 119, 120, 239, 240, 1_000] {
        let x: Vec<f64> = (0..df + 2).map(|i| i as f64 * 0.001).collect();
        let y: Vec<f64> = x
            .iter()
            .enumerate()
            .map(|(i, &b)| 1.4 * b + if i % 2 == 0 { 0.002 } else { -0.0015 })
            .collect();
        let result = beta(&y, &x).expect("valid benchmark request");
        let critical = (result.ci_upper - result.beta) / result.std_err;
        let probability = student_t_cdf(critical, df as f64).expect("valid degrees of freedom");
        assert!(
            (probability - 0.975).abs() < 1e-9,
            "df={df}: upper confidence bound has CDF {probability}"
        );
    }
}

#[test]
fn greeks_basic() {
    let r = [0.01, 0.02, 0.03, 0.04, 0.05];
    let b = [0.005, 0.01, 0.015, 0.02, 0.025];
    let g = greeks(&r, &b, 252.0, 0.0).expect("valid benchmark request");
    assert!((g.beta - 2.0).abs() < 1e-10);
}

#[test]
fn greeks_reports_adjusted_r_squared() {
    let r = [0.011, 0.018, 0.031, 0.039, 0.052, 0.061];
    let b = [0.005, 0.010, 0.015, 0.020, 0.025, 0.030];
    let g = greeks(&r, &b, 252.0, 0.0).expect("valid benchmark request");
    let n = r.len() as f64;
    let expected = 1.0 - (1.0 - g.r_squared) * (n - 1.0) / (n - 2.0);

    assert!((g.adjusted_r_squared - expected).abs() < 1e-12);
    assert!(g.adjusted_r_squared <= g.r_squared);
}

#[test]
fn greeks_alpha_is_annualized_jensen_alpha() {
    let ann_factor = 12.0;
    let risk_free_rate = 0.12;
    let beta_value = 1.6;
    let intercept = 0.001;
    let b = [-0.02, -0.01, 0.00, 0.01, 0.02, 0.03];
    let r: Vec<f64> = b
        .iter()
        .map(|&bench| intercept + beta_value * bench)
        .collect();

    let zero_rf = greeks(&r, &b, ann_factor, 0.0).expect("valid benchmark request");
    let nonzero_rf = greeks(&r, &b, ann_factor, risk_free_rate).expect("valid benchmark request");
    let rf_period = (1.0_f64 + risk_free_rate).powf(1.0 / ann_factor) - 1.0;
    let expected_change = rf_period * (zero_rf.beta - 1.0) * ann_factor;

    assert!((zero_rf.beta - beta_value).abs() < 1e-12);
    assert!((zero_rf.alpha - intercept * ann_factor).abs() < 1e-12);
    assert!((nonzero_rf.alpha - zero_rf.alpha - expected_change).abs() < 1e-12);
}

#[test]
fn rolling_greeks_basic() {
    let r: Vec<f64> = (0..20).map(|i| (i as f64 + 1.0) * 0.001).collect();
    let b: Vec<f64> = (0..20).map(|i| i as f64 * 0.0005).collect();
    let dates: Vec<Date> = (1..=20).map(jan).collect();
    let rg = rolling_greeks(&r, &b, &dates, 5, 252.0, 0.0).expect("valid benchmark request");
    assert_eq!(rg.betas.len(), 16);
}

#[test]
fn rolling_greeks_alpha_is_annualized_jensen_alpha() {
    let ann_factor = 12.0;
    let risk_free_rate = 0.12;
    let beta_value = 1.4;
    let intercept = 0.002;
    let b: Vec<f64> = (0..12).map(|i| -0.03 + i as f64 * 0.006).collect();
    let r: Vec<f64> = b
        .iter()
        .map(|&bench| intercept + beta_value * bench)
        .collect();
    let dates: Vec<Date> = (1..=12).map(jan).collect();

    let zero_rf =
        rolling_greeks(&r, &b, &dates, 6, ann_factor, 0.0).expect("valid benchmark request");
    let nonzero_rf = rolling_greeks(&r, &b, &dates, 6, ann_factor, risk_free_rate)
        .expect("valid benchmark request");
    let rf_period = (1.0_f64 + risk_free_rate).powf(1.0 / ann_factor) - 1.0;
    let expected_change = rf_period * (beta_value - 1.0) * ann_factor;

    for ((zero_alpha, nonzero_alpha), beta) in zero_rf
        .alphas
        .iter()
        .zip(nonzero_rf.alphas.iter())
        .zip(nonzero_rf.betas.iter())
    {
        assert!((*beta - beta_value).abs() < 1e-12);
        assert!((*zero_alpha - intercept * ann_factor).abs() < 1e-12);
        assert!((*nonzero_alpha - *zero_alpha - expected_change).abs() < 1e-12);
    }
}

#[test]
fn rolling_greeks_emits_nan_when_benchmark_window_is_constant() {
    // Use exactly-representable benchmark values (0.5) so the OLS denominator
    // collapses to a true zero rather than floating-point noise.
    let window = 5;
    let r: Vec<f64> = (0..20).map(|i| (i as f64 + 1.0) * 0.001).collect();
    let b: Vec<f64> = vec![0.5_f64; 20];
    let dates: Vec<Date> = (1..=20).map(jan).collect();
    let rg = rolling_greeks(&r, &b, &dates, window, 252.0, 0.0).expect("valid benchmark request");
    assert_eq!(rg.alphas.len(), rg.dates.len());
    assert_eq!(rg.betas.len(), rg.dates.len());
    assert!(
        rg.alphas.iter().all(|a| a.is_nan()),
        "constant benchmark window must surface as NaN alpha rather than a plausible 0"
    );
    assert!(
        rg.betas.iter().all(|b| b.is_nan()),
        "constant benchmark window must surface as NaN beta rather than a plausible 0"
    );
}

#[test]
fn rolling_greeks_rejects_nonfinite_history_before_window_selection() {
    let mut portfolio = vec![0.01; 30];
    portfolio[7] = f64::NAN;
    let benchmark: Vec<f64> = (0..30).map(|i| i as f64 * 0.001).collect();
    let dates: Vec<Date> = (1..=30).map(jan).collect();
    assert!(rolling_greeks(&portfolio, &benchmark, &dates, 5, 252.0, 0.0).is_err());
}

#[test]
fn batch_beta_functions_report_nan_for_zero_variance_benchmark() {
    let r: Vec<f64> = (0..10).map(|i| (i as f64 + 1.0) * 0.001).collect();
    let b = vec![0.5_f64; 10];

    assert!(
        beta(&r, &b)
            .map(|fit| fit.beta)
            .expect("valid benchmark request")
            .is_nan()
    );
    assert!(r_squared(&r, &b).expect("valid benchmark request").is_nan());

    let g = greeks(&r, &b, 252.0, 0.0).expect("valid benchmark request");
    assert!(g.beta.is_nan());
    assert!(g.alpha.is_nan());
    assert!(g.r_squared.is_nan());
    assert!(g.adjusted_r_squared.is_nan());

    let result = beta(&r, &b).expect("valid benchmark request");
    assert!(result.beta.is_nan());
    assert!(result.std_err.is_nan());
    assert!(result.ci_lower.is_nan());
    assert!(result.ci_upper.is_nan());

    assert!(treynor_from_returns(&r, &b, 0.02, 1.0).unwrap().is_nan());
}

#[test]
fn rolling_greeks_stays_close_to_exact_recomputation_on_long_series() {
    /// Reference implementation, computed per window about that window's
    /// own means.
    ///
    /// Centring is what makes this a *reference*: the naive power-sum form
    /// is itself accurate to only ~8e-5 on this data (verified against
    /// exact rational arithmetic), so comparing the kernel against an
    /// uncentred reference measures agreement between two ill-conditioned
    /// computations rather than accuracy.
    fn exact_rolling_greeks(
        returns: &[f64],
        benchmark: &[f64],
        window: usize,
        ann_factor: f64,
    ) -> (Vec<f64>, Vec<f64>) {
        let n = returns.len().min(benchmark.len());
        let w = window as f64;
        let mut alphas = Vec::with_capacity(n - window + 1);
        let mut betas = Vec::with_capacity(n - window + 1);
        for end in window..=n {
            let rs = &returns[end - window..end];
            let bs = &benchmark[end - window..end];
            let mean_r = neumaier_sum(rs.iter().copied()).unwrap() / w;
            let mean_b = neumaier_sum(bs.iter().copied()).unwrap() / w;
            let sr: f64 = neumaier_sum(rs.iter().map(|&r| r - mean_r)).unwrap();
            let sb: f64 = neumaier_sum(bs.iter().map(|&b| b - mean_b)).unwrap();
            let srb: f64 = neumaier_sum(
                rs.iter()
                    .zip(bs.iter())
                    .map(|(&r, &b)| (r - mean_r) * (b - mean_b)),
            )
            .unwrap();
            let sb2: f64 = neumaier_sum(bs.iter().map(|&b| (b - mean_b) * (b - mean_b))).unwrap();
            let denom = w * sb2 - sb * sb;
            let (alpha, beta) = if denom.abs() < 1e-30 {
                (f64::NAN, f64::NAN)
            } else {
                let beta = (w * srb - sb * sr) / denom;
                let alpha = (mean_r - beta * mean_b) * ann_factor;
                (alpha, beta)
            };
            alphas.push(alpha);
            betas.push(beta);
        }
        (alphas, betas)
    }

    let window = 64;
    let ann_factor = 252.0;
    let n = ROLLING_GREEKS_RECOMPUTE_INTERVAL * 4 + window + 33;
    let r: Vec<f64> = (0..n)
        .map(|i| {
            let x = i as f64;
            1_000_000.0 + x * 0.125 + (x / 9.0).sin() * 0.01
        })
        .collect();
    let b: Vec<f64> = (0..n)
        .map(|i| {
            let x = i as f64;
            500_000.0 + x * 0.0625 + (x / 7.0).cos() * 0.01
        })
        .collect();
    let dates: Vec<Date> = (0..n).map(|i| jan(1) + Duration::days(i as i64)).collect();

    let rolling =
        rolling_greeks(&r, &b, &dates, window, ann_factor, 0.0).expect("valid benchmark request");
    let (_, expected_betas) = exact_rolling_greeks(&r, &b, window, ann_factor);

    let max_beta_diff = rolling
        .betas
        .iter()
        .zip(expected_betas.iter())
        .map(|(&actual, &expected)| (actual - expected).abs())
        .fold(0.0_f64, f64::max);
    assert!(
        // Measured 1.13e-13 with shifted-origin sums. The bound here used
        // to be 4.5e-4, which reflected the *uncentred* reference this
        // test compared against rather than any real error: kernel and
        // reference were losing the same ~8e-5 to cancellation, so they
        // agreed while both being wrong. Against exact rational
        // arithmetic the uncentred form errs by 7.8e-5 on this series and
        // the shifted form by 1.1e-13.
        max_beta_diff < 1e-12,
        "max beta diff too large: {max_beta_diff}"
    );
    assert!(rolling.alphas.iter().all(|alpha| alpha.is_finite()));

    // A departing portfolio-side outlier (while benchmark stays smooth)
    // must trigger an immediate rebuild rather than polluting beta/alpha
    // over the remaining recompute interval.
    let mut r_spike: Vec<f64> = b.iter().map(|&bi| 2.0 * (bi - 500_000.0) + 0.001).collect();
    let b_smooth: Vec<f64> = b.iter().map(|&bi| bi - 500_000.0).collect();
    r_spike[0] = 1.0e9;
    r_spike[3] = -1.0e9;
    let spike_rolling = rolling_greeks(&r_spike, &b_smooth, &dates, window, ann_factor, 0.0)
        .expect("valid benchmark request");
    let (expected_alphas, expected_spike_betas) =
        exact_rolling_greeks(&r_spike, &b_smooth, window, ann_factor);
    for (idx, ((&a, &ea), (&bt, &ebt))) in spike_rolling
        .alphas
        .iter()
        .zip(expected_alphas.iter())
        .zip(spike_rolling.betas.iter().zip(expected_spike_betas.iter()))
        .enumerate()
        .skip(4)
    {
        assert!(
            (bt - ebt).abs() < 1e-10,
            "beta mismatch at {idx}: {bt} vs {ebt}"
        );
        assert!(
            (a - ea).abs() < 1e-10,
            "alpha mismatch at {idx}: {a} vs {ea}"
        );
    }
}

#[test]
fn up_capture_hand_calc() {
    // r = [0.10, −0.05], b = [0.05, −0.10]
    // Up periods (b≥0): index 0
    // port_prod = 1.10, bench_prod = 1.05
    // up_capture = (1.10−1) / (1.05−1) = 0.10/0.05 = 2.0
    let r = [0.10, -0.05];
    let b = [0.05, -0.10];
    let uc = up_capture(&r, &b, 1.0).expect("valid benchmark request");
    assert!((uc - 2.0).abs() < 1e-12);
}

#[test]
fn down_capture_hand_calc() {
    // Same data: down periods (b<0): index 1
    // port_prod = 0.95, bench_prod = 0.90
    // down_capture = (0.95−1) / (0.90−1) = −0.05/−0.10 = 0.5
    let r = [0.10, -0.05];
    let b = [0.05, -0.10];
    let down = down_capture(&r, &b, 1.0).expect("valid benchmark request");
    assert!((down - 0.5).abs() < 1e-12);
}

#[test]
fn capture_ratio_hand_calc() {
    // up/down = 2.0/0.5 = 4.0
    let r = [0.10, -0.05];
    let b = [0.05, -0.10];
    let cr = capture_ratio(&r, &b, 1.0).expect("valid benchmark request");
    assert!((cr - 4.0).abs() < 1e-12);
}

#[test]
fn up_capture_multiple_periods() {
    // r = [0.04, −0.01, 0.06], b = [0.02, −0.03, 0.03]
    // Up periods: indices 0, 2 (b[0]=0.02≥0, b[2]=0.03≥0)
    // port geometric return = sqrt((1.04)(1.06)) − 1
    // bench geometric return = sqrt((1.02)(1.03)) − 1
    let r = [0.04, -0.01, 0.06];
    let b = [0.02, -0.03, 0.03];
    let uc = up_capture(&r, &b, 1.0).expect("valid benchmark request");
    let expected = ((1.04_f64 * 1.06_f64).sqrt() - 1.0) / ((1.02_f64 * 1.03_f64).sqrt() - 1.0);
    assert!((uc - expected).abs() < 1e-12);
}

#[test]
fn up_capture_uses_empyrical_style_annualized_subset_return() {
    let r = vec![0.02; 36];
    let b = vec![0.015; 36];
    let uc = up_capture(&r, &b, 12.0).expect("valid benchmark request");
    let expected = ((1.02_f64).powf(12.0) - 1.0) / ((1.015_f64).powf(12.0) - 1.0);

    assert!((uc - expected).abs() < 1e-12);
    assert!((uc - 1.371_251_926_947_35).abs() < 1e-12);
}

#[test]
fn up_capture_uses_geometric_subset_returns() {
    let r = [1.0, 0.0, -0.4];
    let b = [0.5, 0.5, -0.1];
    let uc = up_capture(&r, &b, 1.0).expect("valid benchmark request");
    let expected_port = (2.0_f64 * 1.0_f64).sqrt() - 1.0;
    let expected_bench = (1.5_f64 * 1.5_f64).sqrt() - 1.0;
    let expected = expected_port / expected_bench;
    assert!((uc - expected).abs() < 1e-12);
}

#[test]
fn down_capture_defensive_portfolio() {
    // Portfolio loses less than benchmark → down < 1.0 (desirable)
    let r = [0.04, -0.01, 0.06];
    let b = [0.02, -0.03, 0.03];
    let down = down_capture(&r, &b, 1.0).expect("valid benchmark request");
    // Down periods: index 1. port_prod=0.99, bench_prod=0.97
    let expected = (0.99 - 1.0) / (0.97 - 1.0);
    assert!((down - expected).abs() < 1e-12);
    assert!(down < 1.0);
}

#[test]
fn down_capture_uses_geometric_subset_returns() {
    let r = [-0.25, 0.0, 0.1];
    let b = [-0.5, -0.5, 0.1];
    let down = down_capture(&r, &b, 1.0).expect("valid benchmark request");
    let expected_port = (0.75_f64 * 1.0_f64).sqrt() - 1.0;
    let expected_bench = (0.5_f64 * 0.5_f64).sqrt() - 1.0;
    let expected = expected_port / expected_bench;
    assert!((down - expected).abs() < 1e-12);
}

#[test]
fn up_capture_no_up_periods() {
    assert!(
        up_capture(&[0.01, 0.02], &[-0.01, -0.02], 1.0)
            .unwrap()
            .is_nan()
    );
}

#[test]
fn down_capture_no_down_periods() {
    assert!(
        down_capture(&[0.01, 0.02], &[0.01, 0.02], 1.0)
            .unwrap()
            .is_nan()
    );
}

#[test]
fn capture_ratio_perfect_tracking() {
    // Portfolio = benchmark → up_capture=1, down_capture=1, ratio=1
    let r = [0.02, -0.03, 0.01, -0.01];
    let cr = capture_ratio(&r, &r, 1.0).expect("valid benchmark request");
    assert!((cr - 1.0).abs() < 1e-12);
}

#[test]
fn batting_average_hand_calc() {
    // r = [0.02, 0.01, 0.03, -0.01], b = [0.01, 0.02, 0.01, 0.00]
    // Wins: r[0]>b[0] (0.02>0.01), r[2]>b[2] (0.03>0.01) → 2/4 = 0.5
    let r = [0.02, 0.01, 0.03, -0.01];
    let b = [0.01, 0.02, 0.01, 0.00];
    assert!((batting_average(&r, &b).expect("valid benchmark request") - 0.5).abs() < 1e-12);
}

#[test]
fn batting_average_all_wins() {
    let r = [0.05, 0.03, 0.04];
    let b = [0.01, 0.01, 0.01];
    assert!((batting_average(&r, &b).expect("valid benchmark request") - 1.0).abs() < 1e-12);
}

#[test]
fn batting_average_empty() {
    assert!(batting_average(&[], &[]).unwrap().is_nan());
}

#[test]
fn treynor_hand_calc() {
    let t = treynor(0.10, 0.02, 1.2, 1.0).expect("valid benchmark request");
    assert!((t - 0.08 / 1.2).abs() < 1e-14);
}

#[test]
fn treynor_zero_beta() {
    assert_eq!(treynor(0.1, 0.02, 0.0, 1.0).unwrap(), f64::INFINITY);
    assert_eq!(treynor(0.01, 0.02, 0.0, 1.0).unwrap(), f64::NEG_INFINITY);
    assert!(treynor(0.02, 0.02, 0.0, 1.0).unwrap().is_nan());
}

#[test]
fn treynor_negative_beta() {
    let t = treynor(0.10, 0.02, -0.5, 1.0).expect("valid benchmark request");
    assert!((t - (0.08 / -0.5)).abs() < 1e-14);
}

#[test]
fn m_squared_hand_calc() {
    let m2 = m_squared_from_returns(&[-0.08, 0.12, 0.32], &[-0.15, 0.0, 0.15], 0.02, 1.0)
        .expect("valid benchmark request");
    assert!((m2 - 0.095).abs() < 1e-12);
}

#[test]
fn m_squared_zero_vol() {
    assert!(
        m_squared_from_returns(&[0.1, 0.1, 0.1], &[-0.15, 0.0, 0.15], 0.02, 1.0)
            .unwrap()
            .is_nan()
    );
    assert!(
        (m_squared_from_returns(&[-0.08, 0.12, 0.32], &[0.0, 0.0, 0.0], 0.02, 1.0).unwrap() - 0.02)
            .abs()
            < 1e-14
    );
}

#[test]
fn jensen_and_sharpe_agree_on_rf_when_beta_is_zero() {
    let mu = 0.0004_f64;
    let ann_factor = 252.0;
    let rf_annual = 0.02;
    let rf_period = crate::returns::periodic_risk_free_rate(rf_annual, ann_factor).expect("cash");
    let expected = (mu - rf_period) * ann_factor;
    let alpha = jensen_alpha(mu, 0.001, 0.0, ann_factor, rf_period);
    let sample_sd = 0.0002 * 2.0_f64.sqrt();
    let sharpe_excess =
        crate::risk_metrics::sharpe(&[mu - 0.0002, mu + 0.0002], rf_annual, ann_factor).unwrap()
            * sample_sd
            * ann_factor.sqrt();
    assert!((alpha - expected).abs() < 1e-12);
    assert!((sharpe_excess - expected).abs() < 1e-12);
    assert!((alpha - sharpe_excess).abs() < 1e-12);
}
#[test]
fn scaled_ols_and_rolling_keep_tiny_identifiable_variation() {
    let b = [0.0, 1e-200, 2e-200];
    let p = [0.0, 2e-200, 4e-200];
    let estimate = beta(&p, &b).expect("tiny fit");
    assert!((estimate.beta - 2.0).abs() < 1e-14);
    let rolling = rolling_greeks(&p, &b, &[jan(1), jan(2), jan(3)], 2, 12.0, 0.0).expect("rolling");
    assert!(rolling.betas.iter().all(|x| (*x - 2.0).abs() < 1e-14));
    assert!(rolling.alphas.iter().all(|x| x.abs() < 1e-212));
}

#[test]
fn strict_capture_ignores_neutral_and_cancels_explosive_growth() {
    let p = [0.02, 0.75];
    let b = [0.01, 0.0];
    let capture = up_capture(&p, &b, 12.0).expect("strict capture");
    let expected = 1.02_f64.powi(12).mul_add(1.0, -1.0) / (1.01_f64.powi(12) - 1.0);
    assert!((capture - expected).abs() < 1e-12);
    let explosive = [100.0, 200.0];
    assert_eq!(
        up_capture(&explosive, &explosive, 1e300).expect("cancel equal growth"),
        1.0
    );
    assert!(
        capture_ratio(&[0.02, 0.0], &[0.01, -0.01], 12.0)
            .expect("zero down")
            .is_nan()
    );
}

#[test]
fn sample_risk_and_zero_risk_are_distinct() {
    assert!(
        tracking_error(&[0.1], &[0.1], 12.0)
            .expect("singleton")
            .is_nan()
    );
    assert_eq!(
        tracking_error(&[0.1, 0.1], &[0.0, 0.0], 12.0).expect("constant"),
        0.0
    );
    assert!(
        information_ratio(&[0.0, 0.0], &[0.0, 0.0], 12.0)
            .expect("zero reward risk")
            .is_nan()
    );
    assert!(
        treynor_from_returns(&[1.0, 2.0, 3.0], &[1e-310, 2e-310, 3e-310], 0.0, 1.0)
            .expect("requested tiny quotient")
            > 0.0
    );
}

#[test]
fn rolling_greeks_roundtrip_preserves_undefined_and_rejects_invalid_state() {
    let result = rolling_greeks(&[0.1, 0.2], &[0.1, 0.2], &[jan(1), jan(2)], 1, 12.0, 0.0)
        .expect("one point windows");
    let json = serde_json::to_value(&result).expect("wire");
    assert_eq!(json["alphas"][0], "nan");
    let restored: RollingGreeks = serde_json::from_value(json.clone()).expect("restore");
    assert!(restored.alphas.iter().all(|x| x.is_nan()));
    for key in ["alphas", "betas"] {
        let mut invalid = json.clone();
        invalid[key][0] = serde_json::json!(0.0);
        assert!(serde_json::from_value::<RollingGreeks>(invalid).is_err());
    }
    let mut invalid = json.clone();
    invalid["alphas"] = serde_json::json!(["nan"]);
    assert!(serde_json::from_value::<RollingGreeks>(invalid).is_err());
    let mut invalid = json;
    invalid["dates"] = serde_json::json!(["2025-01-02", "2025-01-01"]);
    assert!(serde_json::from_value::<RollingGreeks>(invalid).is_err());
}
#[test]
fn noisy_ols_matches_exact_rational_reference() {
    // Exact decimal rational arithmetic: beta=199/100, intercept=1/2000,
    // SSE=67/10^7, SST=9917/2500000 and SE(beta)^2=67/30000.
    let p = [0.020, 0.041, 0.059, 0.082, 0.099];
    let b = [0.010, 0.020, 0.030, 0.040, 0.050];
    let estimate = beta(&p, &b).expect("fit");
    assert!((estimate.beta - 1.99).abs() < 1e-14);
    assert!((estimate.std_err - (67.0_f64 / 30000.0).sqrt()).abs() < 1e-14);
    let point = greeks(&p, &b, 12.0, 0.0).expect("point fit");
    assert!((point.alpha - 0.006).abs() < 1e-14);
    assert!((point.r_squared - 39601.0 / 39668.0).abs() < 1e-14);
}

#[test]
fn requested_r_squared_and_ratios_avoid_unrequested_overflows() {
    assert!(
        (r_squared(&[0.0, 1e300, 2e300], &[0.0, 1e-300, 2e-300]).expect("r squared") - 1.0).abs()
            < 1e-14
    );
    let p = [0.0, 1e300, 2e300];
    let b = [0.0, 1.0, 2.0];
    assert!(
        (treynor_from_returns(&p, &b, 0.0, 1e100).expect("Treynor") / 1e100 - 1.0).abs() < 1e-12
    );
    assert_eq!(
        m_squared_from_returns(&p, &[0.0, 0.0, 0.0], 0.0, 1e100).expect("cash target"),
        0.0
    );
}

#[test]
fn rolling_reconstructs_covariance_product_underflow() {
    let b = [0.0, 1e-100, 2e-100];
    let p = [0.0, 1e-250, 2e-250];
    let result =
        rolling_greeks(&p, &b, &[jan(1), jan(2), jan(3)], 3, 1.0, 0.0).expect("scaled rolling");
    assert!((result.betas[0] / 1e-150 - 1.0).abs() < 1e-12);
}
#[test]
fn tiny_capture_ratios_do_not_require_representable_component_growth() {
    let tiny = f64::from_bits(1);
    assert_eq!(up_capture(&[tiny, tiny], &[tiny, tiny], 1.0).unwrap(), 1.0);
    assert_eq!(up_capture(&[1e-200], &[1e-200], 1e-200).unwrap(), 1.0);
    assert_eq!(
        up_capture(&[tiny, tiny, tiny], &[tiny, tiny, tiny], tiny).unwrap(),
        1.0
    );
}

#[test]
fn exact_active_rebuild_retains_variation_beneath_large_differences() {
    let risk = tracking_error(&[1e308, 1e308], &[1.0, 2.0], 1.0).unwrap();
    assert!((risk - 0.5_f64.sqrt()).abs() < 1e-14);
}

#[test]
fn exact_treynor_avoids_unrequested_unrepresentable_beta() {
    let ratio =
        treynor_from_returns(&[1.0, 2.0, 3.0], &[1e-310, 2e-310, 3e-310], 0.0, 1.0).unwrap();
    assert!((ratio / 2e-310 - 1.0).abs() < 1e-12);
}

#[test]
fn exact_slope_and_alpha_recover_cancelled_cross_scale_terms() {
    let p = [1e308, 0.0, 1.0];
    let b = [0.0, 1.0, -1.0];
    let result = fit(&p, &b, false, false).unwrap();
    assert_eq!(result.beta, -0.5);
    let point = greeks(&p, &b, 1.0, 0.0).unwrap_err();
    // The nonzero R squared is below binary64 range, an explicit failure.
    assert!(point.to_string().contains("range"));
}
#[test]
fn exact_uncertainty_preserves_tiny_nonzero_residuals() {
    let p = [0.0, 1.0, 2.0, 1e-300];
    let b = [0.0, 1.0, 2.0, 0.0];
    let result = beta(&p, &b).unwrap();
    assert!((result.beta - 1.0).abs() < 1e-14);
    assert!(result.std_err > 0.0);
    assert!(result.std_err < 1e-299);
}
