//! Exercise the `Performance` facade to improve coverage of `performance.rs`.
//!
//! This is intentionally a broad API smoke test: it wires realistic inputs and
//! invokes most public methods with valid parameters.

use finstack_quant_analytics::Performance;
use finstack_quant_core::dates::{
    CalendarRegistry, Date, FiscalConfig, HolidayCalendar, Month, PeriodKind,
};

fn nyse() -> &'static dyn HolidayCalendar {
    CalendarRegistry::global()
        .resolve_str("nyse")
        .expect("nyse calendar")
}

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

#[test]
fn performance_facade_exercises_broad_api_surface() {
    let dates = calendar_days(
        Date::from_calendar_date(2025, Month::January, 1).expect("d0"),
        140,
    );
    let col_a = ramp(dates.len(), 100.0, 0.04);
    let col_b = ramp(dates.len(), 50.0, -0.02);
    let prices = vec![col_a, col_b.clone()];

    let mut perf = Performance::new(
        dates.clone(),
        prices,
        vec!["AAA".to_string(), "BBB".to_string()],
        Some("BBB"),
        PeriodKind::Daily,
    )
    .expect("perf");

    assert_eq!(perf.benchmark_idx(), 1);
    assert_eq!(perf.freq(), PeriodKind::Daily);

    let _ = perf.cagr().expect("valid performance CAGR");
    let _ = perf.mean_return(true);
    let _ = perf.mean_return(false);
    let _ = perf.volatility(true);
    let _ = perf.sharpe(0.02);
    let _ = perf.sortino(0.0);
    let _ = perf.calmar().expect("valid Calmar ratios");
    let _ = perf.max_drawdown();
    let _ = perf.value_at_risk(0.95);
    let _ = perf.expected_shortfall(0.95);
    let _ = perf.tail_ratio(0.95);
    let _ = perf.ulcer_index();
    let _ = perf.skewness();
    let _ = perf.kurtosis();
    let _ = perf.geometric_mean();
    let _ = perf.downside_deviation(0.0);
    let _ = perf.max_drawdown_duration();
    let _ = perf.up_capture();
    let _ = perf.down_capture();
    let _ = perf.capture_ratio();
    let _ = perf.rolling_volatility(0, 20).expect("rolling volatility");
    let _ = perf.omega_ratio(0.0);
    let _ = perf.treynor(0.02);
    let _ = perf.gain_to_pain();
    let _ = perf.martin_ratio().expect("valid Martin ratios");
    let _ = perf.parametric_var(0.95);
    let _ = perf.cornish_fisher_var(0.95);
    let _ = perf.rolling_sortino(0, 25, 0.0).expect("rolling sortino");
    let _ = perf.recovery_factor();
    let _ = perf.sterling_ratio(0.02, 3).expect("valid Sterling ratios");
    let _ = perf.burke_ratio(0.02, 3).expect("valid Burke ratios");
    let _ = perf.pain_index();
    let _ = perf.pain_ratio(0.02).expect("valid Pain ratios");

    let bench_series: Vec<f64> = col_b
        .windows(2)
        .map(|w| {
            if w[0] == 0.0 {
                0.0
            } else {
                (w[1] / w[0]) - 1.0
            }
        })
        .collect();
    let _ = perf
        .multi_factor_greeks(0, &[&bench_series])
        .expect("multi-factor regression");

    let _ = perf.cdar(0.95);
    let _ = perf.cumulative_returns();
    let _ = perf.drawdown_series();
    let _ = perf.drawdown_details(0, 3).expect("drawdown details");
    let _ = perf.tracking_error();
    let _ = perf.information_ratio();
    let _ = perf.r_squared();
    let _ = perf.beta();
    let _ = perf.greeks(0.0);
    let _ = perf.rolling_greeks(0, 30, 0.0).expect("rolling greeks");
    let _ = perf.batting_average();
    let _ = perf.m_squared(0.02);
    let _ = perf.modified_sharpe(0.02, 0.95);
    let _ = perf.rolling_sharpe(0, 30, 0.02).expect("rolling sharpe");

    let ref_date = *perf.active_dates().last().expect("last active date");
    let _ = perf
        .lookback_returns(ref_date, FiscalConfig::us_federal(), nyse())
        .expect("lookback");

    let _ = perf
        .period_stats(0, PeriodKind::Monthly, None)
        .expect("period stats");
    let _ = perf.correlation_matrix();
    let _ = perf.cumulative_returns_outperformance();
    let _ = perf.drawdown_difference();
    let _ = perf
        .drawdown_details(perf.benchmark_idx(), 2)
        .expect("benchmark drawdown details");

    let rf = vec![0.0; perf.active_dates().len()];
    let _ = perf.excess_returns(&rf, Some(252.0));

    let d = perf.dates().to_vec();
    perf.reset_date_range(d[15], d[d.len() - 1]);
    let _ = perf.ulcer_index();
    perf.reset_date_range(d[0], d[d.len() - 1]);

    perf.reset_bench_ticker("AAA").expect("bench switch");
    assert_eq!(perf.benchmark_idx(), 0);
    perf.reset_bench_ticker("BBB").expect("restore bench");

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
    let lookbacks = perf
        .lookback_returns(ref_date, FiscalConfig::us_federal(), nyse())
        .expect("lookback");
    assert_eq!(lookbacks.mtd, vec![0.0]);
    assert_eq!(lookbacks.qtd, vec![0.0]);
    assert_eq!(lookbacks.ytd, vec![0.0]);
    assert_eq!(lookbacks.fytd, Some(vec![0.0]));
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
    let lookbacks = rising_perf
        .lookback_returns(ref_date, config, nyse())
        .expect("lookback");
    assert!(lookbacks.fytd.expect("fytd present")[0] > 0.0);
}
