//! Regression tests pinning analytics metric values.
use finstack_analytics::Performance;
use finstack_core::dates::{Date, Month, PeriodKind};

fn d(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid date")
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn performance_cagr_uses_default_act_365_25_convention_for_single_return_window() {
    let dates = vec![d(2023, Month::January, 1), d(2024, Month::January, 1)];
    let prices = vec![vec![100.0, 110.0]];
    let perf = Performance::new(
        dates,
        prices,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Annual,
    )
    .expect("performance should build");

    let cagr = perf.cagr().expect("valid performance CAGR");
    assert_eq!(cagr.len(), 1);
    let expected = 1.10_f64.powf(365.25 / 365.0) - 1.0;
    assert!(
        (cagr[0] - expected).abs() < 1e-12,
        "expected default Act/365.25 CAGR {}, got {}",
        expected,
        cagr[0]
    );
}

#[test]
fn cumulative_outperformance_uses_relative_wealth_not_return_difference() {
    let dates = vec![
        d(2024, Month::January, 1),
        d(2024, Month::January, 2),
        d(2024, Month::January, 3),
    ];
    let benchmark_prices = vec![100.0, 80.0, 96.0];
    let portfolio_prices = vec![100.0, 90.0, 108.0];
    let perf = Performance::new(
        dates,
        vec![benchmark_prices, portfolio_prices],
        vec!["BENCH".to_string(), "PORT".to_string()],
        Some("BENCH"),
        PeriodKind::Daily,
    )
    .expect("performance should build");

    let outperformance = perf.cumulative_returns_outperformance();
    let port_vs_bench = &outperformance[1];
    let expected = [0.125, 0.125];

    assert_eq!(port_vs_bench.len(), expected.len());
    for (actual, expected_value) in port_vs_bench.iter().zip(expected.iter()) {
        assert!(
            (actual - expected_value).abs() < 1e-12,
            "expected compounded relative wealth outperformance {}, got {}",
            expected_value,
            actual
        );
    }
}

#[test]
fn performance_new_rejects_interior_nan_return_data() {
    let dates = vec![
        d(2024, Month::January, 1),
        d(2024, Month::January, 2),
        d(2024, Month::January, 3),
        d(2024, Month::January, 4),
    ];
    // Middle zero price creates an interior non-finite return that should be rejected.
    let prices = vec![vec![100.0, 0.0, 110.0, 111.0]];

    let result = Performance::new(
        dates,
        prices,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    );

    assert!(
        result.is_err(),
        "interior non-finite returns should be rejected rather than coerced"
    );
}

#[test]
fn performance_new_accepts_edge_ragged_price_columns() {
    let dates: Vec<Date> = (1..=6).map(|day| d(2024, Month::January, day)).collect();
    let perf = Performance::new(
        dates.clone(),
        vec![
            vec![100.0, 101.0, 102.0, 103.0, 104.0, 105.0],
            vec![f64::NAN, f64::NAN, 50.0, 55.0, 60.5, f64::NAN],
        ],
        vec!["BENCH".to_string(), "PORT".to_string()],
        Some("BENCH"),
        PeriodKind::Daily,
    )
    .expect("edge-ragged price panel should build");

    assert_eq!(
        perf.active_dates_for_ticker(1)
            .expect("active ticker dates"),
        &dates[3..5]
    );
    assert_eq!(perf.cumulative_returns()[1].len(), 2);
}

#[test]
fn performance_from_returns_accepts_edge_ragged_return_columns() {
    let dates: Vec<Date> = (1..=5).map(|day| d(2024, Month::January, day)).collect();
    let perf = Performance::from_returns(
        dates.clone(),
        vec![
            vec![0.01, 0.02, 0.03, 0.04, 0.05],
            vec![f64::NAN, 0.02, 0.03, 0.04, f64::NAN],
        ],
        vec!["BENCH".to_string(), "PORT".to_string()],
        Some("BENCH"),
        PeriodKind::Daily,
    )
    .expect("edge-ragged return panel should build");

    assert_eq!(
        perf.active_dates_for_ticker(1)
            .expect("active ticker dates"),
        &dates[1..4]
    );
    assert_eq!(perf.drawdown_series()[1].len(), 3);
}

#[test]
fn performance_rejects_interior_missing_values_in_ragged_panels() {
    let dates: Vec<Date> = (1..=4).map(|day| d(2024, Month::January, day)).collect();

    assert!(
        Performance::new(
            dates.clone(),
            vec![vec![100.0, f64::NAN, 102.0, 103.0]],
            vec!["PORT".to_string()],
            None,
            PeriodKind::Daily,
        )
        .is_err(),
        "interior missing prices inside a finite price span should be rejected"
    );

    assert!(
        Performance::from_returns(
            dates,
            vec![vec![0.01, f64::NAN, 0.02, 0.03]],
            vec!["PORT".to_string()],
            None,
            PeriodKind::Daily,
        )
        .is_err(),
        "interior missing returns inside a finite return span should be rejected"
    );
}

#[test]
fn performance_rejects_invalid_price_spans() {
    let dates = vec![d(2024, Month::January, 1), d(2024, Month::January, 2)];

    assert!(
        Performance::new(
            dates.clone(),
            vec![vec![f64::NAN, f64::NAN]],
            vec!["PORT".to_string()],
            None,
            PeriodKind::Daily,
        )
        .is_err(),
        "price column with no finite positive observations should be rejected"
    );

    assert!(
        Performance::new(
            dates,
            vec![vec![f64::NAN, 100.0]],
            vec!["PORT".to_string()],
            None,
            PeriodKind::Daily,
        )
        .is_err(),
        "price column with only one finite positive observation should be rejected"
    );
}

#[test]
fn benchmark_relative_metrics_use_overlapping_dates_only() {
    let dates: Vec<Date> = (1..=5).map(|day| d(2024, Month::January, day)).collect();
    let perf = Performance::from_returns(
        dates,
        vec![
            vec![0.50, 0.01, 0.02, 0.03, -0.40],
            vec![f64::NAN, 0.02, 0.04, 0.06, f64::NAN],
        ],
        vec!["BENCH".to_string(), "PORT".to_string()],
        Some("BENCH"),
        PeriodKind::Monthly,
    )
    .expect("edge-ragged returns should build");

    assert_close(perf.beta()[1].beta, 2.0);
    assert_close(perf.correlation_matrix()[0][1], 1.0);

    let expected_up_capture = ((1.02_f64 * 1.04_f64 * 1.06_f64).powf(12.0 / 3.0) - 1.0)
        / ((1.01_f64 * 1.02_f64 * 1.03_f64).powf(12.0 / 3.0) - 1.0);
    assert_close(perf.up_capture()[1], expected_up_capture);
}

#[test]
fn drawdown_series_rebases_after_date_window_reset() {
    let dates = vec![
        d(2024, Month::January, 1),
        d(2024, Month::January, 2),
        d(2024, Month::January, 3),
        d(2024, Month::January, 4),
    ];
    let prices = vec![vec![100.0, 150.0, 120.0, 130.0]];
    let mut perf = Performance::new(
        dates,
        prices,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    )
    .expect("performance should build");

    perf.reset_date_range(d(2024, Month::January, 4), d(2024, Month::January, 4));

    let drawdowns = perf.drawdown_series();
    assert_eq!(drawdowns.len(), 1);
    assert_eq!(
        drawdowns[0],
        vec![0.0],
        "windowed drawdowns should be rebased to the active window's own wealth path"
    );
    assert_eq!(
        perf.max_drawdown(),
        vec![0.0],
        "max drawdown over an all-positive active window should be zero"
    );
}

#[test]
fn drawdown_cache_refreshes_across_repeated_date_window_resets() {
    let dates = vec![
        d(2024, Month::January, 1),
        d(2024, Month::January, 2),
        d(2024, Month::January, 3),
        d(2024, Month::January, 4),
        d(2024, Month::January, 5),
    ];
    let prices = vec![vec![100.0, 150.0, 120.0, 130.0, 110.0]];
    let mut perf = Performance::new(
        dates,
        prices,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    )
    .expect("performance should build");

    perf.reset_date_range(d(2024, Month::January, 4), d(2024, Month::January, 4));
    assert_eq!(
        perf.drawdown_series()[0],
        vec![0.0],
        "positive one-period window should rebase to zero drawdown"
    );

    perf.reset_date_range(d(2024, Month::January, 5), d(2024, Month::January, 5));
    let refreshed = perf.drawdown_series();
    let refreshed_port = &refreshed[0];
    assert_eq!(refreshed_port.len(), 1);
    assert!(
        refreshed_port[0] < 0.0,
        "cache should refresh when the active date window changes"
    );
    assert_eq!(
        perf.max_drawdown(),
        vec![refreshed_port[0]],
        "max drawdown should reflect the refreshed one-period window"
    );
}

#[test]
fn benchmark_drawdown_views_follow_benchmark_switch_with_active_window() {
    let dates = vec![
        d(2024, Month::January, 1),
        d(2024, Month::January, 2),
        d(2024, Month::January, 3),
        d(2024, Month::January, 4),
    ];
    let mut perf = Performance::new(
        dates,
        vec![
            vec![100.0, 100.0, 110.0, 121.0],
            vec![100.0, 100.0, 100.0, 80.0],
            vec![100.0, 100.0, 100.0, 90.0],
        ],
        vec!["BENCH".to_string(), "ALT".to_string(), "PORT".to_string()],
        Some("BENCH"),
        PeriodKind::Daily,
    )
    .expect("performance should build");

    perf.reset_date_range(d(2024, Month::January, 4), d(2024, Month::January, 4));
    let initial_port_outperformance = perf.drawdown_difference()[2][0];
    assert!(
        (initial_port_outperformance + 0.1).abs() < 1e-12,
        "PORT should lag a flat benchmark by its own rebased drawdown"
    );
    assert!(
        perf.drawdown_details(perf.benchmark_idx(), 1)
            .expect("benchmark drawdown details")
            .is_empty(),
        "positive benchmark window should have no drawdown episodes"
    );

    perf.reset_bench_ticker("ALT")
        .expect("alternate benchmark should exist");
    let switched_port_outperformance = perf.drawdown_difference()[2][0];
    assert!(
        (switched_port_outperformance - 0.1).abs() < 1e-12,
        "drawdown outperformance should use the switched benchmark's windowed drawdown"
    );

    let bench_episodes = perf
        .drawdown_details(perf.benchmark_idx(), 1)
        .expect("benchmark drawdown details after switch");
    assert_eq!(bench_episodes.len(), 1);
    assert!(
        (bench_episodes[0].max_drawdown + 0.2).abs() < 1e-12,
        "benchmark drawdown stats should come from the switched benchmark"
    );
}

#[test]
fn performance_max_drawdown_and_calmar_are_consistent() {
    let returns = vec![0.10, -0.20, 0.05, -0.10, 0.08];
    let dates: Vec<Date> = (0..returns.len() as u8 + 1)
        .map(|i| d(2024, Month::January, i + 1))
        .collect();
    let perf = Performance::from_returns(
        dates[1..].to_vec(),
        vec![returns],
        vec!["P".to_string()],
        None,
        PeriodKind::Monthly,
    )
    .expect("performance should build");

    let max_dd = perf.max_drawdown()[0];
    assert!(max_dd <= 0.0);
    let calmar = perf.calmar().expect("valid Calmar")[0];
    assert!(calmar.is_finite() || calmar.is_nan());
}

#[test]
fn performance_new_rejects_returns_below_negative_one() {
    let dates = vec![d(2024, Month::January, 1), d(2024, Month::January, 2)];
    let prices = vec![vec![100.0, -10.0]];

    let result = Performance::new(
        dates,
        prices,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    );

    assert!(
        result.is_err(),
        "price paths implying returns below -100% should be rejected"
    );
}

#[test]
fn performance_new_rejects_negative_price_domain_in_simple_return_mode() {
    let dates = vec![d(2024, Month::January, 1), d(2024, Month::January, 2)];
    let prices = vec![vec![-100.0, -90.0]];

    let result = Performance::new(
        dates,
        prices,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    );

    assert!(
        result.is_err(),
        "negative price domains should be rejected rather than converted into synthetic returns"
    );
}

#[test]
fn performance_new_rejects_duplicate_dates() {
    let dates = vec![
        d(2024, Month::January, 1),
        d(2024, Month::January, 2),
        d(2024, Month::January, 2),
        d(2024, Month::January, 3),
    ];
    let prices = vec![vec![100.0, 101.0, 102.0, 103.0]];

    let result = Performance::new(
        dates,
        prices,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    );

    assert!(
        result.is_err(),
        "duplicate dates should be rejected at construction so downstream lookback / aggregation logic does not silently produce nonsense"
    );
}

#[test]
fn performance_new_rejects_non_monotonic_dates() {
    let dates = vec![
        d(2024, Month::January, 1),
        d(2024, Month::January, 3),
        d(2024, Month::January, 2),
        d(2024, Month::January, 4),
    ];
    let prices = vec![vec![100.0, 101.0, 102.0, 103.0]];

    let result = Performance::new(
        dates,
        prices,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    );

    assert!(
        result.is_err(),
        "out-of-order dates should be rejected; partition_point lookups assume strictly ascending order"
    );
}

#[test]
fn performance_from_returns_rejects_duplicate_dates() {
    let dates = vec![
        d(2024, Month::January, 1),
        d(2024, Month::January, 2),
        d(2024, Month::January, 2),
    ];
    let returns = vec![vec![0.01, 0.02, 0.03]];

    let result = Performance::from_returns(
        dates,
        returns,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    );

    assert!(
        result.is_err(),
        "duplicate return-aligned dates should be rejected at construction"
    );
}

#[test]
fn performance_from_returns_rejects_non_monotonic_dates() {
    let dates = vec![
        d(2024, Month::January, 1),
        d(2024, Month::January, 3),
        d(2024, Month::January, 2),
    ];
    let returns = vec![vec![0.01, 0.02, 0.03]];

    let result = Performance::from_returns(
        dates,
        returns,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    );

    assert!(
        result.is_err(),
        "out-of-order return-aligned dates should be rejected"
    );
}

#[test]
fn performance_rolling_returns_errors_on_invalid_ticker_index() {
    let dates = vec![
        d(2024, Month::January, 1),
        d(2024, Month::January, 2),
        d(2024, Month::January, 3),
    ];
    let prices = vec![vec![100.0, 101.0, 102.0]];
    let perf = Performance::new(
        dates,
        prices,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    )
    .expect("performance should build");

    assert!(
        perf.rolling_returns(7, 2).is_err(),
        "out-of-range ticker index should surface as an error rather than an empty series"
    );
    assert!(
        perf.drawdown_details(7, 1).is_err(),
        "out-of-range ticker index should error from drawdown_details rather than silently return no episodes"
    );
}

#[test]
fn performance_new_rejects_non_finite_price_domain_in_simple_return_mode() {
    let dates = vec![d(2024, Month::January, 1), d(2024, Month::January, 2)];
    let prices = vec![vec![f64::INFINITY, 101.0]];

    let result = Performance::new(
        dates,
        prices,
        vec!["PORT".to_string()],
        None,
        PeriodKind::Daily,
    );

    assert!(
        result.is_err(),
        "non-finite prices should be rejected even when the derived return would look finite"
    );
}
