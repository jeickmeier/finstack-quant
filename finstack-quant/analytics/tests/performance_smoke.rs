//! Exercise the `Performance` facade to improve coverage of `performance.rs`.
//!
//! This is intentionally a broad API smoke test: it wires realistic inputs and
//! invokes most public methods with valid parameters.

use finstack_quant_analytics::Performance;
use finstack_quant_core::dates::{Date, FiscalConfig, Month, PeriodKind};

fn calendar_days(start: Date, n: usize) -> Vec<Date> {
    let mut out = Vec::with_capacity(n);
    let mut d = start;
    for _ in 0..n {
        out.push(d);
        d = d.next_day().expect("finite calendar");
    }
    out
}

fn ramp(len: usize, start: f64, step: f64) -> Vec<f64> {
    (0..len).map(|i| start + step * i as f64).collect()
}

fn oscillating_prices(len: usize, initial: f64, phase: f64) -> Vec<f64> {
    let mut prices = Vec::with_capacity(len);
    let mut price = initial;
    prices.push(price);
    for i in 1..len {
        let x = i as f64;
        let period_return =
            0.0004 + 0.012 * (0.41 * x + phase).sin() + 0.006 * (0.17 * x - phase).cos();
        price *= 1.0 + period_return;
        prices.push(price);
    }
    prices
}

fn assert_finite_metric(label: &str, values: &[f64], expected_len: usize) {
    assert_eq!(values.len(), expected_len, "{label} column count");
    assert!(
        values.iter().all(|value| value.is_finite()),
        "{label} should be finite for a non-degenerate panel: {values:?}"
    );
}

fn assert_finite_panel(label: &str, panel: &[Vec<f64>], expected_lens: &[usize]) {
    assert_eq!(panel.len(), expected_lens.len(), "{label} column count");
    for (column, &expected_len) in panel.iter().zip(expected_lens) {
        assert_eq!(column.len(), expected_len, "{label} row count");
        assert!(
            column.iter().all(|value| value.is_finite()),
            "{label} should contain finite values"
        );
    }
}

#[test]
fn performance_facade_exercises_broad_api_surface() {
    let dates = calendar_days(
        Date::from_calendar_date(2025, Month::January, 1).expect("d0"),
        140,
    );
    let col_a = oscillating_prices(dates.len(), 100.0, 0.0);
    let col_b = oscillating_prices(dates.len(), 80.0, 1.3);
    let prices = vec![col_a, col_b];

    let mut perf = Performance::new(
        dates.clone(),
        prices,
        vec!["AAA".to_string(), "BBB".to_string()],
        Some("BBB"),
        PeriodKind::Daily,
    )
    .expect("perf");

    assert_eq!(perf.benchmark_idx(), 1);
    assert_eq!(perf.frequency(), PeriodKind::Daily);
    let ticker_count = perf.ticker_names().len();
    let active_lens: Vec<usize> = (0..ticker_count)
        .map(|idx| {
            perf.active_dates_for_ticker(idx)
                .expect("valid ticker index")
                .len()
        })
        .collect();

    for (label, values) in [
        (
            "cagr",
            perf.cagr(finstack_quant_analytics::CagrDayCount::default(), None)
                .expect("valid performance CAGR"),
        ),
        ("annual mean", perf.mean_return(true)),
        ("period mean", perf.mean_return(false)),
        ("volatility", perf.volatility(true)),
        ("sharpe", perf.sharpe(0.02)),
        ("sortino", perf.sortino(0.0)),
        ("calmar", perf.calmar().expect("valid Calmar ratios")),
        ("max drawdown", perf.max_drawdown()),
        (
            "value at risk",
            perf.value_at_risk(0.95).expect("valid confidence"),
        ),
        (
            "expected shortfall",
            perf.expected_shortfall(0.95).expect("valid confidence"),
        ),
        (
            "tail ratio",
            perf.tail_ratio(0.95).expect("valid confidence"),
        ),
        ("ulcer index", perf.ulcer_index()),
        ("skewness", perf.skewness()),
        ("kurtosis", perf.kurtosis()),
        ("geometric mean", perf.geometric_mean()),
        ("downside deviation", perf.downside_deviation(0.0)),
        ("up capture", perf.up_capture()),
        ("down capture", perf.down_capture()),
        ("capture ratio", perf.capture_ratio()),
        ("omega ratio", perf.omega_ratio(0.0)),
        ("treynor", perf.treynor(0.02)),
        ("gain to pain", perf.gain_to_pain()),
        (
            "martin ratio",
            perf.martin_ratio().expect("valid Martin ratios"),
        ),
        (
            "parametric var",
            perf.parametric_var(0.95, None).expect("valid confidence"),
        ),
        (
            "cornish-fisher var",
            perf.cornish_fisher_var(0.95, None)
                .expect("valid confidence"),
        ),
        ("recovery factor", perf.recovery_factor()),
        (
            "sterling ratio",
            perf.sterling_ratio(0.02, 3).expect("valid Sterling ratios"),
        ),
        (
            "burke ratio",
            perf.burke_ratio(0.02, 3).expect("valid Burke ratios"),
        ),
        ("pain index", perf.pain_index()),
        (
            "pain ratio",
            perf.pain_ratio(0.02).expect("valid Pain ratios"),
        ),
        (
            "conditional drawdown at risk",
            perf.cdar(0.95).expect("valid confidence"),
        ),
        ("tracking error", perf.tracking_error()),
        ("information ratio", perf.information_ratio()),
        ("r squared", perf.r_squared()),
        ("batting average", perf.batting_average()),
        ("m squared", perf.m_squared(0.02)),
        (
            "modified sharpe",
            perf.modified_sharpe(0.02, 0.95).expect("valid confidence"),
        ),
    ] {
        assert_finite_metric(label, &values, ticker_count);
    }

    let standalone_var = perf.value_at_risk(0.95).expect("valid confidence");
    let standalone_es = perf.expected_shortfall(0.95).expect("valid confidence");
    let (batch_var, batch_es) = perf.value_at_risk_and_es(0.95);
    assert_eq!(batch_var, standalone_var);
    assert_eq!(batch_es, standalone_es);

    let durations = perf.max_drawdown_duration();
    assert_eq!(durations.len(), ticker_count);
    assert!(durations.iter().all(|duration| *duration >= 0));

    let rolling_volatility = perf.rolling_volatility(0, 20).expect("rolling volatility");
    assert_eq!(rolling_volatility.values.len(), active_lens[0] - 20 + 1);
    assert_eq!(
        rolling_volatility.dates.len(),
        rolling_volatility.values.len()
    );
    assert!(rolling_volatility
        .values
        .iter()
        .all(|value| value.is_finite()));

    let rolling_sortino = perf.rolling_sortino(0, 25, 0.0).expect("rolling sortino");
    assert_eq!(rolling_sortino.values.len(), active_lens[0] - 25 + 1);
    assert_eq!(rolling_sortino.dates.len(), rolling_sortino.values.len());
    assert!(rolling_sortino.values.iter().all(|value| value.is_finite()));

    let bench_series = perf
        .returns_for_ticker(perf.benchmark_idx())
        .expect("benchmark returns");
    let multi_factor = perf
        .multi_factor_greeks(
            0,
            &[&bench_series],
            finstack_quant_analytics::ReturnKind::Excess,
        )
        .expect("multi-factor regression");
    assert_eq!(multi_factor.betas.len(), 1);
    assert!(multi_factor.alpha.is_finite());
    assert!(multi_factor.betas[0].is_finite());
    assert!(multi_factor.r_squared.is_finite());
    assert!(multi_factor.adjusted_r_squared.is_finite());
    assert!(multi_factor.residual_vol.is_finite());

    let cumulative = perf.cumulative_returns();
    assert_finite_panel("cumulative returns", &cumulative, &active_lens);
    let drawdowns = perf.drawdown_series();
    assert_finite_panel("drawdown series", &drawdowns, &active_lens);
    assert!(drawdowns
        .iter()
        .flatten()
        .all(|value| (-1.0..=0.0).contains(value)));

    let drawdown_episodes = perf.drawdown_details(0, 3).expect("drawdown details");
    assert!(drawdown_episodes.len() <= 3);
    assert!(drawdown_episodes
        .iter()
        .all(|episode| episode.max_drawdown.is_finite() && episode.max_drawdown <= 0.0));

    let betas = perf.beta();
    assert_eq!(betas.len(), ticker_count);
    assert!(betas.iter().all(|result| {
        result.beta.is_finite()
            && result.std_err.is_finite()
            && result.ci_lower.is_finite()
            && result.ci_upper.is_finite()
    }));

    let greeks = perf.greeks(0.0);
    assert_eq!(greeks.len(), ticker_count);
    assert!(greeks.iter().all(|result| {
        result.alpha.is_finite()
            && result.beta.is_finite()
            && result.r_squared.is_finite()
            && result.adjusted_r_squared.is_finite()
    }));

    let rolling_greeks = perf.rolling_greeks(0, 30, 0.0).expect("rolling greeks");
    let rolling_count = active_lens[0] - 30 + 1;
    assert_eq!(rolling_greeks.dates.len(), rolling_count);
    assert_eq!(rolling_greeks.alphas.len(), rolling_count);
    assert_eq!(rolling_greeks.betas.len(), rolling_count);
    assert!(rolling_greeks.alphas.iter().all(|value| value.is_finite()));
    assert!(rolling_greeks.betas.iter().all(|value| value.is_finite()));

    let rolling_sharpe = perf.rolling_sharpe(0, 30, 0.02).expect("rolling sharpe");
    assert_eq!(rolling_sharpe.values.len(), rolling_count);
    assert_eq!(rolling_sharpe.dates.len(), rolling_count);
    assert!(rolling_sharpe.values.iter().all(|value| value.is_finite()));

    let ref_date = *perf.active_dates().last().expect("last active date");
    let lookbacks = perf.lookback_returns(ref_date, FiscalConfig::us_federal());
    assert_finite_metric("month-to-date lookback", &lookbacks.mtd, ticker_count);
    assert_finite_metric("quarter-to-date lookback", &lookbacks.qtd, ticker_count);
    assert_finite_metric("year-to-date lookback", &lookbacks.ytd, ticker_count);
    assert_finite_metric(
        "fiscal-year-to-date lookback",
        &lookbacks.fytd,
        ticker_count,
    );

    let period_stats = perf
        .period_stats(0, PeriodKind::Monthly, None)
        .expect("period stats");
    assert!(period_stats.best.is_finite());
    assert!(period_stats.worst.is_finite());
    assert!(period_stats.avg_return.is_finite());
    assert!((0.0..=1.0).contains(&period_stats.win_rate));

    let correlation = perf.correlation_matrix().expect("psd correlation");
    assert_eq!(correlation.len(), ticker_count);
    for (i, row) in correlation.iter().enumerate() {
        assert_eq!(row.len(), ticker_count);
        assert!((row[i] - 1.0).abs() < 1e-12);
        for (j, value) in row.iter().enumerate() {
            assert!(value.is_finite());
            assert!((value - correlation[j][i]).abs() < 1e-12);
        }
    }

    let outperformance = perf.cumulative_returns_outperformance();
    assert_finite_panel("cumulative outperformance", &outperformance, &active_lens);
    let drawdown_difference = perf.drawdown_difference();
    assert_finite_panel("drawdown difference", &drawdown_difference, &active_lens);

    let benchmark_drawdowns = perf
        .drawdown_details(perf.benchmark_idx(), 2)
        .expect("benchmark drawdown details");
    assert!(benchmark_drawdowns.len() <= 2);

    let rf = vec![0.0; perf.active_dates().len()];
    let excess = perf
        .excess_returns(&rf, Some(252.0))
        .expect("aligned zero rf");
    assert_finite_panel("excess returns", &excess, &active_lens);

    let d = perf.dates().to_vec();
    let baseline_cumulative = perf.cumulative_returns();
    perf.reset_date_range(d[15], d[d.len() - 1]);
    let windowed_cumulative = perf.cumulative_returns();
    assert_ne!(windowed_cumulative, baseline_cumulative);
    assert!(windowed_cumulative
        .iter()
        .all(|column| column.len() < active_lens[0]));
    perf.reset_date_range(d[0], d[d.len() - 1]);
    assert_eq!(perf.cumulative_returns(), baseline_cumulative);

    let baseline_tracking_error = perf.tracking_error();
    perf.reset_bench_ticker("AAA").expect("bench switch");
    assert_eq!(perf.benchmark_idx(), 0);
    assert_ne!(perf.tracking_error(), baseline_tracking_error);
    perf.reset_bench_ticker("BBB").expect("restore bench");
    assert_eq!(perf.benchmark_idx(), 1);
    assert_eq!(perf.tracking_error(), baseline_tracking_error);

    assert!(Performance::new(
        dates.clone(),
        vec![ramp(dates.len(), 1.0, 0.01)],
        vec!["X".to_string()],
        Some("missing"),
        PeriodKind::Daily,
    )
    .is_err());
}

#[test]
fn performance_lookback_returns_clamps_pre_start_reference_date() {
    let dates = calendar_days(
        Date::from_calendar_date(2025, Month::June, 1).expect("d0"),
        40,
    );
    let prices = vec![ramp(dates.len(), 50.0, 0.1)];
    let perf = Performance::new(
        dates,
        prices,
        vec!["P".to_string()],
        None,
        PeriodKind::Daily,
    )
    .expect("performance");

    let ref_date = Date::from_calendar_date(2025, Month::May, 15).expect("ref date");
    let lookbacks = perf.lookback_returns(ref_date, FiscalConfig::us_federal());
    assert_eq!(lookbacks.mtd, vec![0.0]);
    assert_eq!(lookbacks.qtd, vec![0.0]);
    assert_eq!(lookbacks.ytd, vec![0.0]);
    assert_eq!(lookbacks.fytd, vec![0.0]);
}

#[test]
fn performance_smoke_asserts_fiscal_lookback_and_zero_variance_invariants() {
    let dates = calendar_days(
        Date::from_calendar_date(2025, Month::January, 1).expect("d0"),
        40,
    );
    let flat_perf = Performance::new(
        dates.clone(),
        vec![vec![100.0; dates.len()]],
        vec!["FLAT".to_string()],
        None,
        PeriodKind::Daily,
    )
    .expect("flat performance");

    assert_eq!(flat_perf.sharpe(0.0), vec![0.0]);
    assert_eq!(flat_perf.max_drawdown(), vec![0.0]);
    assert!(flat_perf
        .drawdown_details(0, 3)
        .expect("drawdown details on flat series")
        .is_empty());

    let rising_perf = Performance::new(
        dates,
        vec![ramp(40, 100.0, 1.0)],
        vec!["RISING".to_string()],
        None,
        PeriodKind::Daily,
    )
    .expect("rising performance");
    let config = FiscalConfig::new(1, 15).expect("valid fiscal config");
    let ref_date = *rising_perf.active_dates().last().expect("last active date");
    let lookbacks = rising_perf.lookback_returns(ref_date, config);
    assert!(lookbacks.fytd[0] > 0.0);
}

#[test]
fn returns_accessors_reproduce_cumulative_returns_and_zero_rf_excess_returns() {
    let dates = calendar_days(
        Date::from_calendar_date(2025, Month::January, 1).expect("d0"),
        60,
    );
    let perf = Performance::new(
        dates.clone(),
        vec![
            ramp(dates.len(), 100.0, 0.35),
            ramp(dates.len(), 80.0, -0.1),
        ],
        vec!["AAA".to_string(), "BBB".to_string()],
        Some("BBB"),
        PeriodKind::Daily,
    )
    .expect("perf");

    let panel = perf.returns();
    assert_eq!(panel.len(), perf.ticker_names().len());

    // The panel accessor and the checked per-ticker accessor agree.
    for (idx, series) in panel.iter().enumerate() {
        let single = perf.returns_for_ticker(idx).expect("valid ticker index");
        assert_eq!(&single, series);
        // Span-aware: one return per active date for this ticker.
        assert_eq!(
            series.len(),
            perf.active_dates_for_ticker(idx)
                .expect("valid ticker index")
                .len()
        );
    }

    // Compounding the raw returns reproduces the cumulative return series.
    let cumulative = perf.cumulative_returns();
    for (series, cum) in panel.iter().zip(cumulative.iter()) {
        assert_eq!(series.len(), cum.len());
        let mut growth = 1.0_f64;
        for (&r, &c) in series.iter().zip(cum.iter()) {
            growth *= 1.0 + r;
            assert!(
                (growth - 1.0 - c).abs() < 1e-12,
                "compounded returns must match cumulative_returns"
            );
        }
    }

    // The `excess_returns(zeros)` workaround is now unnecessary: it is exactly
    // what `returns()` yields.
    let rf = vec![0.0; perf.active_dates().len()];
    assert_eq!(
        perf.excess_returns(&rf, None).expect("aligned zero rf"),
        panel
    );

    // Out-of-range indices are rejected rather than silently returning empty.
    assert!(perf.returns_for_ticker(perf.ticker_names().len()).is_err());
}

#[test]
fn aligned_panel_rejects_malformed_shapes_and_only_pads_inactive_dates() {
    let dates = calendar_days(
        Date::from_calendar_date(2025, Month::January, 2).expect("first date"),
        4,
    );
    let perf = Performance::from_returns(
        dates.clone(),
        vec![
            vec![0.01, 0.02, 0.03, 0.04],
            vec![f64::NAN, 0.02, 0.03, f64::NAN],
        ],
        vec!["A".into(), "B".into()],
        None,
        PeriodKind::Daily,
    )
    .expect("ragged panel");

    for malformed in [
        vec![vec![1.0; 4]],
        vec![vec![1.0; 4], vec![2.0; 2], vec![]],
        vec![vec![1.0; 3], vec![2.0; 2]],
        vec![vec![1.0; 5], vec![2.0; 2]],
        vec![vec![1.0; 4], vec![2.0]],
        vec![vec![1.0; 4], vec![2.0; 3]],
    ] {
        assert!(perf.aligned_panel(malformed).is_err());
    }

    let (aligned_dates, columns) = perf.aligned_panel(perf.returns()).expect("valid series");
    assert_eq!(aligned_dates, dates);
    assert_eq!(columns[0], vec![0.01, 0.02, 0.03, 0.04]);
    assert!(columns[1][0].is_nan());
    assert_eq!(columns[1][1], 0.02);
    assert_eq!(columns[1][2], 0.03);
    assert!(columns[1][3].is_nan());
}

#[test]
fn lookback_returns_includes_maximum_reference_date_without_overflow() {
    let perf = Performance::new(
        vec![Date::MAX.previous_day().expect("prior date"), Date::MAX],
        vec![vec![100.0, 110.0]],
        vec!["A".into()],
        None,
        PeriodKind::Daily,
    )
    .expect("maximum-date panel");
    for fiscal_config in [FiscalConfig::calendar_year(), FiscalConfig::us_federal()] {
        let lookbacks = perf.lookback_returns(Date::MAX, fiscal_config);
        for values in [lookbacks.mtd, lookbacks.qtd, lookbacks.ytd, lookbacks.fytd] {
            assert_eq!(values.len(), 1);
            assert!((values[0] - 0.1).abs() < 1.0e-14);
        }
    }
}
