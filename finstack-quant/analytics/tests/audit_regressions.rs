//! Numerical and boundary regressions for analytics kernels.
use finstack_quant_analytics::correlation::{nearest_correlation_matrix, NearestCorrelationOpts};
use finstack_quant_analytics::{max_drawdown, sharpe, sortino, Performance, ReturnKind};
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
