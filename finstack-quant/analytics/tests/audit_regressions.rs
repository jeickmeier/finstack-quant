//! Numerical and boundary regressions for analytics kernels.
use finstack_quant_analytics::correlation::{nearest_correlation_matrix, NearestCorrelationOpts};
use finstack_quant_analytics::{
    max_drawdown, sharpe, sortino, volatility, CagrDayCount, Performance, ReturnKind,
};
use finstack_quant_core::dates::{Date, Duration, Month, PeriodKind};
use serde_json::json;

fn dates(n: usize) -> Vec<Date> {
    let start = Date::from_calendar_date(2024, Month::January, 1).unwrap();
    (0..n).map(|i| start + Duration::days(i as i64)).collect()
}

fn panel(returns: Vec<f64>, frequency: PeriodKind) -> Performance {
    Performance::from_returns(
        dates(returns.len()),
        vec![returns],
        vec!["A".into()],
        None,
        frequency,
    )
    .unwrap()
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-12, "{actual} != {expected}");
}

fn assert_rolling_risk_matches(actual: f64, expected: f64) {
    if expected == 0.0 || !expected.is_finite() {
        assert_eq!(actual, expected);
    } else {
        assert!(
            (actual / expected - 1.0).abs() < 1e-8,
            "rolling={actual}, fresh={expected}"
        );
    }
}

fn assert_rolling_ratio_matches(actual: f64, expected: f64) {
    if expected.abs() < 1e-8 {
        close(actual, expected);
    } else {
        assert_rolling_risk_matches(actual, expected);
    }
}

#[test]
fn rolling_risk_recovers_after_outliers_leave_constant_windows() {
    for window in [2, 3, 21, 63, 252] {
        for level in [0.0, 0.001, 0.01, -0.01] {
            let mut returns = vec![level; window + 1100];
            returns[0] = -0.1;
            returns[1] = -0.2;
            // Cross a scheduled rebuild and then resume varying returns.
            returns.extend([0.02, -0.03, 0.01, -0.01]);
            let perf = panel(returns.clone(), PeriodKind::Daily);
            let vol = perf.rolling_volatility(0, window).unwrap();
            let ratio = perf.rolling_sharpe(0, window, 0.0).unwrap();
            assert_eq!(vol.dates, dates(returns.len())[window - 1..]);
            assert_eq!(ratio.dates, vol.dates);
            for (start, slice) in returns.windows(window).enumerate() {
                assert_rolling_risk_matches(vol.values[start], volatility(slice, 252.0));
                assert_rolling_ratio_matches(ratio.values[start], sharpe(slice, 0.0, 252.0));
            }
        }
    }
}

#[test]
fn rolling_risk_preserves_small_nonzero_variance_after_outliers_leave() {
    for movement in [1e-10, 1e-18] {
        let returns = vec![0.1, -0.1, movement, 2.0 * movement, 3.0 * movement];
        let perf = panel(returns.clone(), PeriodKind::Daily);
        assert_rolling_risk_matches(
            perf.rolling_volatility(0, 3).unwrap().values[2],
            volatility(&returns[2..], 252.0),
        );
        assert_rolling_risk_matches(
            perf.rolling_sharpe(0, 3, 0.0).unwrap().values[2],
            sharpe(&returns[2..], 0.0, 252.0),
        );
    }
}

#[test]
fn rolling_risk_sortino_recovers_after_large_losses_leave() {
    for window in [2, 3, 21, 63, 252] {
        for mar in [0.0, 0.005] {
            for tail in [0.001, 0.0, -1e-10, -1e-18] {
                let mut returns = vec![mar + tail; window + 1100];
                returns[0] = mar - 0.03;
                returns[1] = mar - 0.01;
                returns.extend([mar - 0.02, mar + 0.03, mar - 0.01]);
                let perf = panel(returns.clone(), PeriodKind::Daily);
                let rolling = perf.rolling_sortino(0, window, mar).unwrap();
                assert_eq!(rolling.dates, dates(returns.len())[window - 1..]);
                for (start, slice) in returns.windows(window).enumerate() {
                    assert_rolling_ratio_matches(rolling.values[start], sortino(slice, mar, 252.0));
                }
            }
        }
    }
}

#[test]
fn rolling_greeks_matches_fresh_windows_across_constant_benchmark_spans() {
    for window in [1, 3, 21, 63, 252] {
        for level in [0.001, 0.01, 0.03, -0.01] {
            let n = window + 150;
            let mut benchmark = vec![level; n];
            benchmark[0] = 0.02;
            benchmark[1] = -0.03;
            // Stay constant across multiple scheduled rebuilds, then resume
            // moving so the regression must recover immediately.
            for (i, value) in benchmark.iter_mut().enumerate().skip(window + 130) {
                *value += (i % 7) as f64 * 0.001;
            }
            let returns: Vec<f64> = (0..n).map(|i| (i % 7) as f64 * 0.001 - 0.003).collect();
            let grid = dates(n);
            let mut perf = Performance::from_returns(
                grid.clone(),
                vec![benchmark, returns],
                vec!["B".into(), "P".into()],
                None,
                PeriodKind::Daily,
            )
            .unwrap();
            let rolling = perf.rolling_greeks(1, window, 0.02).unwrap();
            assert_eq!(rolling.dates, grid[window - 1..]);
            for start in 0..rolling.betas.len() {
                perf.reset_date_range(grid[start], grid[start + window - 1]);
                let fresh = &perf.greeks(0.02)[1];
                if fresh.beta.is_nan() {
                    assert!(
                        rolling.betas[start].is_nan(),
                        "window={window}, level={level}, start={start}"
                    );
                    assert!(rolling.alphas[start].is_nan());
                } else {
                    assert!((rolling.betas[start] - fresh.beta).abs() < 1e-10);
                    assert!((rolling.alphas[start] - fresh.alpha).abs() < 1e-10);
                }
            }
        }
    }
}

#[test]
fn rolling_greeks_reproduced_three_observation_window_is_undefined() {
    let perf = Performance::from_returns(
        dates(5),
        vec![
            vec![0.02, -0.03, 0.001, 0.001, 0.001],
            vec![0.04, -0.06, 0.01, 0.02, 0.03],
        ],
        vec!["B".into(), "P".into()],
        None,
        PeriodKind::Daily,
    )
    .unwrap();
    let rolling = perf.rolling_greeks(1, 3, 0.0).unwrap();
    assert!(rolling.betas[2].is_nan());
    assert!(rolling.alphas[2].is_nan());
}

#[test]
fn rolling_greeks_preserves_small_nonzero_benchmark_variance() {
    for (level, movement) in [(0.0, 1e-18), (0.001, 1e-10)] {
        let mut benchmark: Vec<f64> = (0..160)
            .map(|i| level + (i % 7) as f64 * movement)
            .collect();
        benchmark[0] = 0.02;
        benchmark[1] = -0.03;
        let returns = benchmark.iter().map(|b| 2.0 * b).collect();
        let perf = Performance::from_returns(
            dates(benchmark.len()),
            vec![benchmark, returns],
            vec!["B".into(), "P".into()],
            None,
            PeriodKind::Daily,
        )
        .unwrap();
        let rolling = perf.rolling_greeks(1, 3, 0.0).unwrap();
        for (&beta, &alpha) in rolling.betas.iter().zip(&rolling.alphas) {
            close(beta, 2.0);
            close(alpha, 0.0);
        }
    }
}

#[test]
fn tail_mass_is_exact_with_ties_and_fractional_observations() {
    let mut returns = vec![0.0; 100];
    returns[0] = -0.2;
    let perf = panel(returns.clone(), PeriodKind::Daily);
    close(perf.expected_shortfall(0.95).unwrap()[0], -0.04);
    returns[1] = 0.25;
    close(
        panel(returns, PeriodKind::Daily).cdar(0.95).unwrap()[0],
        -0.04,
    );
    let fractional = panel(vec![-0.2, -0.1, 0.1, 0.2], PeriodKind::Daily);
    close(
        fractional.expected_shortfall(0.625).unwrap()[0],
        -0.25 / 1.5,
    );
    close(fractional.expected_shortfall(0.99).unwrap()[0], -0.2);
}

#[test]
fn invalid_scalar_inputs_do_not_report_zero_risk() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(max_drawdown(&[invalid, -0.2]).is_nan());
        assert!(max_drawdown(&[-0.2, invalid]).is_nan());
        assert!(sortino(&[invalid, 0.01], 0.0, 252.0).is_nan());
        assert!(sortino(&[0.0, 0.0], invalid, 252.0).is_nan());
        assert!(sharpe(&[0.0, 0.0], invalid, 252.0).is_nan());
        assert!(panel(vec![0.0, 0.0], PeriodKind::Daily).sharpe(invalid)[0].is_nan());
    }
}

#[test]
fn singleton_ratios_agree_across_entry_points() {
    let perf = panel(vec![0.01, 0.02], PeriodKind::Daily);
    assert!(sharpe(&[0.01], 0.0, 252.0).is_nan());
    assert!(sortino(&[0.01], 0.0, 252.0).is_nan());
    assert!(panel(vec![0.01], PeriodKind::Daily).sharpe(0.0)[0].is_nan());
    let rolling = perf.rolling_sharpe(0, 1, 0.0).unwrap();
    assert_eq!(rolling.dates, dates(2));
    assert!(rolling.values.iter().all(|v| v.is_nan()));
    assert!(perf
        .rolling_sortino(0, 1, 0.0)
        .unwrap()
        .values
        .iter()
        .all(|v| v.is_nan()));
}

#[test]
fn constructors_reject_duplicate_identity_and_unrepresentable_returns() {
    for prices in [vec![1e-300, 1e300], vec![1e300, 1e-300]] {
        assert!(Performance::new(
            dates(2),
            vec![prices],
            vec!["A".into()],
            None,
            PeriodKind::Daily
        )
        .is_err());
    }
    let names = vec!["A".into(), "A".into()];
    assert!(Performance::from_returns(
        dates(2),
        vec![vec![0.01; 2]; 2],
        names.clone(),
        None,
        PeriodKind::Daily
    )
    .is_err());
    assert!(Performance::new(
        dates(2),
        vec![vec![100.0, 101.0]; 2],
        names,
        None,
        PeriodKind::Daily
    )
    .is_err());
}

#[test]
fn restored_state_is_validated_and_caches_are_recomputed() {
    let perf = panel(vec![-0.2, 0.25, 0.01], PeriodKind::Daily);
    let state = serde_json::to_value(&perf).unwrap();
    assert!(state.get("drawdowns").is_none());
    assert!(state.get("active_window_drawdowns").is_none());
    let mut changed = state.clone();
    changed["returns"][0][0] = json!(-0.3);
    let restored: Performance = serde_json::from_value(changed).unwrap();
    close(restored.max_drawdown()[0], -0.3);
    for (key, value) in [
        ("drawdowns", json!([[0.0, 0.0, 0.0]])),
        ("benchmark_idx", json!(99)),
        ("start_idx", json!(99)),
        ("end_idx", json!(99)),
        ("return_spans", json!([{"start":0,"end":99}])),
        ("returns", json!([[-1.0, 0.25, 0.01]])),
        ("returns", json!([[null, 0.25, 0.01]])),
    ] {
        let mut invalid = state.clone();
        invalid[key] = value;
        assert!(
            serde_json::from_value::<Performance>(invalid).is_err(),
            "accepted {key}"
        );
    }
}

#[test]
fn ragged_window_round_trip_preserves_alignment_and_initial_dates() {
    let mut perf = Performance::from_returns(
        dates(4),
        vec![
            vec![0.01, -0.1, 0.2, 0.01],
            vec![f64::NAN, -0.2, 0.25, f64::NAN],
        ],
        vec!["A".into(), "B".into()],
        Some("B"),
        PeriodKind::Daily,
    )
    .unwrap();
    perf.reset_date_range(dates(4)[1], dates(4)[2]);
    let restored: Performance =
        serde_json::from_str(&serde_json::to_string(&perf).unwrap()).unwrap();
    assert_eq!(restored.active_dates(), perf.active_dates());
    assert_eq!(restored.returns(), perf.returns());
    assert_eq!(restored.max_drawdown(), perf.max_drawdown());
    assert_eq!(
        restored.drawdown_details(1, 5).unwrap(),
        perf.drawdown_details(1, 5).unwrap()
    );
    assert_eq!(restored.benchmark_idx(), 1);
}

#[test]
fn initial_drawdown_uses_known_peak_date_in_full_and_windowed_panels() {
    let ds = dates(4);
    let mut perf = Performance::new(
        ds.clone(),
        vec![vec![100.0, 90.0, 100.0, 110.0]],
        vec!["A".into()],
        None,
        PeriodKind::Daily,
    )
    .unwrap();
    for windowed in [false, true] {
        if windowed {
            perf.reset_date_range(ds[1], ds[2]);
        }
        let episode = &perf.drawdown_details(0, 5).unwrap()[0];
        assert_eq!(episode.start, ds[0]);
        assert_eq!(episode.end, Some(ds[2]));
        assert_eq!(episode.duration_days, 2);
        assert!(!episode.truncated_at_start);
        assert_eq!(perf.max_drawdown_duration(), vec![2]);
    }
}

#[test]
fn m_squared_self_comparison_has_no_cash_basis_residual() {
    let perf = panel(vec![0.01, -0.02, 0.03, 0.01], PeriodKind::Monthly);
    for cash in [0.0, 0.02, 0.12] {
        close(perf.m_squared(cash)[0], 0.09);
    }
}

#[test]
fn neutral_periods_do_not_change_binary_kelly_fraction() {
    let decisive = panel(vec![0.02, 0.02, -0.01], PeriodKind::Daily)
        .period_stats(0, PeriodKind::Daily, None)
        .unwrap();
    let neutral = panel(vec![0.02, 0.02, -0.01, 0.0], PeriodKind::Daily)
        .period_stats(0, PeriodKind::Daily, None)
        .unwrap();
    close(decisive.kelly_criterion, 0.5);
    close(neutral.kelly_criterion, decisive.kelly_criterion);
    close(neutral.win_rate, 0.5);
}

#[test]
fn nearest_correlation_preserves_already_feasible_off_diagonal() {
    let output = nearest_correlation_matrix(
        &[1.0005, 0.5, 0.5, 1.0005],
        2,
        NearestCorrelationOpts::default(),
    )
    .unwrap();
    assert_eq!(output, vec![1.0, 0.5, 0.5, 1.0]);
}

#[test]
fn constant_response_fit_statistics_remain_undefined() {
    for n in [3, 5, 29, 57, 58] {
        let perf = panel(vec![0.01; n], PeriodKind::Daily);
        let factors: Vec<f64> = (0..n).map(|i| i as f64 * 0.001).collect();
        let fit = perf
            .multi_factor_greeks(0, &[&factors], ReturnKind::Excess)
            .unwrap();
        assert!(
            fit.r_squared.is_nan(),
            "finite R-squared for {n} constant observations"
        );
        assert!(fit.adjusted_r_squared.is_nan());
        let restored: finstack_quant_analytics::MultiFactorResult =
            serde_json::from_str(&serde_json::to_string(&fit).unwrap()).unwrap();
        assert!(restored.r_squared.is_nan());
    }
}

#[test]
fn cagr_annualizes_log_growth_before_reconstructing_terminal_wealth() {
    let grid: Vec<Date> = (2001..=2020)
        .map(|year| Date::from_calendar_date(year, Month::January, 1).unwrap())
        .collect();
    let perf = Performance::from_returns(
        grid,
        vec![vec![-0.9; 20]],
        vec!["A".into()],
        None,
        PeriodKind::Annual,
    )
    .unwrap();
    // The 20-year wealth is 1e-20, but the annual growth factor remains 0.1.
    let years = 7305.0 / 365.25;
    let expected = 0.1_f64.powf(20.0 / years) - 1.0;
    close(
        perf.cagr(CagrDayCount::default(), None).unwrap()[0],
        expected,
    );

    let overflow = Performance::from_returns(
        vec![
            Date::from_calendar_date(2021, Month::January, 1).unwrap(),
            Date::from_calendar_date(2022, Month::January, 1).unwrap(),
        ],
        vec![vec![1e200; 2]],
        vec!["A".into()],
        None,
        PeriodKind::Annual,
    )
    .unwrap();
    let result = overflow.cagr(CagrDayCount::default(), None).unwrap()[0];
    let expected = 1e200_f64.powf(2.0 / (731.0 / 365.25));
    assert!(result.is_finite());
    assert!((result / expected - 1.0).abs() < 1e-12);
}

#[test]
fn identical_paths_have_zero_relative_wealth_at_extreme_absolute_wealth() {
    for returns in [vec![-0.9; 20], vec![1e300; 4]] {
        let perf = Performance::from_returns(
            dates(returns.len()),
            vec![returns.clone(), returns],
            vec!["BENCH".into(), "PORT".into()],
            None,
            PeriodKind::Daily,
        )
        .unwrap();
        assert!(perf
            .cumulative_returns_outperformance()
            .iter()
            .flatten()
            .all(|&relative| relative == 0.0));
    }
}

#[test]
fn drawdowns_remain_finite_when_absolute_wealth_overflows() {
    let perf = panel(vec![1e300, 1e300, -0.5, 0.0], PeriodKind::Daily);
    let drawdown = &perf.drawdown_series()[0];
    close(drawdown[0], 0.0);
    close(drawdown[1], 0.0);
    close(drawdown[2], -0.5);
    close(drawdown[3], -0.5);
    close(perf.max_drawdown()[0], -0.5);
    let episodes = perf.drawdown_details(0, 5).unwrap();
    assert_eq!(episodes.len(), 1);
    close(episodes[0].max_drawdown, -0.5);
    assert_eq!(episodes[0].start, dates(4)[1]);
    assert_eq!(episodes[0].end, None);
    assert_eq!(perf.max_drawdown_duration(), vec![2]);
}

#[test]
fn positive_near_wipeout_growth_is_not_exact_wipeout() {
    let near_wipeout = -0.9999999999999999_f64;
    let perf = panel(
        vec![near_wipeout, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        PeriodKind::Daily,
    );
    let expected = (1.0 + near_wipeout).powf(1.0 / 8.0) - 1.0;
    close(perf.geometric_mean()[0], expected);
    assert!(perf.geometric_mean()[0] > -1.0);
}

#[test]
fn compounded_metrics_preserve_returns_smaller_than_machine_epsilon() {
    let movement = 1e-18;
    let perf = panel(vec![movement, movement], PeriodKind::Daily);
    assert!((perf.cumulative_returns()[0][1] / (2.0 * movement) - 1.0).abs() < 1e-12);
    assert!((perf.geometric_mean()[0] / movement - 1.0).abs() < 1e-12);
    let excess = perf.excess_returns(&[movement; 2], Some(252.0)).unwrap();
    let expected = movement * (1.0 - 1.0 / 252.0);
    assert!((excess[0][0] / expected - 1.0).abs() < 1e-12);
}

#[test]
fn rolling_returns_preserve_tiny_windows_after_positive_outlier_leaves() {
    let movement = 1e-18;
    let mut returns = vec![0.1];
    returns.extend((0..1110).map(|i| if i % 2 == 0 { movement } else { -movement }));
    let perf = panel(returns.clone(), PeriodKind::Daily);
    let singleton = perf.rolling_returns(0, 1).unwrap();
    assert_eq!(singleton.dates, dates(returns.len()));
    for (&actual, &expected) in singleton.values.iter().zip(&returns) {
        assert!((actual / expected - 1.0).abs() < 1e-12);
    }

    let rolling = perf.rolling_returns(0, 3).unwrap();
    assert_eq!(rolling.dates, dates(returns.len())[2..]);
    close(rolling.values[0], 0.1);
    // At this scale the product correction is below f64 resolution: each
    // alternating three-return window compounds to its first return.
    // The series also crosses the scheduled accumulator rebuild.
    for (start, &actual) in rolling.values.iter().enumerate().skip(1) {
        assert!((actual / returns[start] - 1.0).abs() < 1e-12);
    }
}

#[test]
fn drawdown_ratios_propagate_invalid_cash_before_zero_risk_sentinels() {
    for returns in [vec![0.1, 0.2], vec![0.0, 0.0], vec![-0.1, 0.2]] {
        let perf = panel(returns, PeriodKind::Daily);
        for cash in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(perf.sterling_ratio(cash, 5).unwrap()[0].is_nan());
            assert!(perf.burke_ratio(cash, 5).unwrap()[0].is_nan());
            assert!(perf.pain_ratio(cash).unwrap()[0].is_nan());
        }
    }
    let positive = panel(vec![0.1, 0.2], PeriodKind::Daily);
    assert_eq!(positive.calmar().unwrap()[0], f64::INFINITY);
    assert_eq!(positive.martin_ratio().unwrap()[0], f64::INFINITY);
    let flat = panel(vec![0.0, 0.0], PeriodKind::Daily);
    assert_eq!(flat.calmar().unwrap()[0], 0.0);
    assert_eq!(flat.martin_ratio().unwrap()[0], 0.0);
}
