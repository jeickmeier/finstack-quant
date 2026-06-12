//! Serde round-trip tests for analytics result types.
use finstack_analytics::{
    AnnualizationConvention, BetaResult, DatedSeries, DrawdownEpisode, GreeksResult,
    LookbackReturns, MultiFactorResult, Performance, PeriodStats, RollingGreeks,
};
use finstack_core::dates::{Date, Month, PeriodKind};

fn roundtrip_json<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let json = serde_json::to_string(value).expect("serialization should succeed");
    serde_json::from_str(&json).expect("deserialization should succeed")
}

fn assert_roundtrip_value<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let restored = roundtrip_json(value);
    assert_eq!(
        serde_json::to_value(value).expect("value serialization should succeed"),
        serde_json::to_value(&restored).expect("value reserialization should succeed")
    );
}

fn make_date(day: u8) -> Date {
    Date::from_calendar_date(2025, Month::January, day).expect("valid date")
}

#[test]
fn test_performance_and_lookback_roundtrip() {
    let dates = (1..=6).map(make_date).collect::<Vec<_>>();
    let prices = vec![
        vec![100.0, 101.0, 102.0, 103.5, 104.0, 105.0],
        vec![80.0, 79.5, 80.5, 81.0, 80.75, 81.5],
    ];
    let performance = Performance::new(
        dates,
        prices,
        vec!["AAA".to_string(), "BBB".to_string()],
        Some("BBB"),
        PeriodKind::Daily,
    )
    .expect("performance construction should succeed");

    let restored = roundtrip_json(&performance);
    assert_eq!(restored.dates(), performance.dates());
    assert_eq!(restored.ticker_names(), performance.ticker_names());
    assert_eq!(restored.benchmark_idx(), performance.benchmark_idx());
    assert_eq!(restored.freq(), performance.freq());

    let lookback = LookbackReturns {
        mtd: vec![0.01, 0.02],
        qtd: vec![0.03, 0.04],
        ytd: vec![0.05, 0.06],
        fytd: Some(vec![0.07, 0.08]),
    };
    assert_roundtrip_value(&lookback);
}

#[test]
fn test_analytics_results_and_configs_roundtrip() {
    let dates = vec![make_date(2), make_date(3), make_date(4)];

    assert_roundtrip_value(&PeriodStats {
        best: 0.12,
        worst: -0.08,
        consecutive_wins: 4,
        consecutive_losses: 2,
        win_rate: 0.6,
        avg_return: 0.015,
        avg_win: 0.04,
        avg_loss: -0.02,
        payoff_ratio: 2.0,
        profit_factor: 1.5,
        cpc_ratio: 1.8,
        kelly_criterion: 0.3,
    });

    assert_roundtrip_value(&DrawdownEpisode {
        start: make_date(2),
        valley: make_date(3),
        end: Some(make_date(4)),
        duration_days: 2,
        max_drawdown: -0.15,
        near_recovery_threshold: -0.0015,
        truncated_at_start: false,
    });

    assert_roundtrip_value(&BetaResult {
        beta: 1.12,
        std_err: 0.08,
        ci_lower: 0.96,
        ci_upper: 1.28,
    });

    assert_roundtrip_value(&GreeksResult {
        alpha: 0.02,
        beta: 0.95,
        r_squared: 0.88,
        adjusted_r_squared: 0.84,
    });

    assert_roundtrip_value(&RollingGreeks {
        dates: dates.clone(),
        alphas: vec![0.01, 0.02, 0.03],
        betas: vec![0.8, 0.9, 1.0],
    });

    assert_roundtrip_value(&MultiFactorResult {
        alpha: 0.015,
        betas: vec![0.7, -0.2, 1.1],
        r_squared: 0.82,
        adjusted_r_squared: 0.79,
        residual_vol: 0.11,
    });

    assert_roundtrip_value(&DatedSeries {
        values: vec![1.1, 1.2, 1.3],
        dates: dates.clone(),
    });

    assert_roundtrip_value(&DatedSeries {
        values: vec![0.15, 0.14, 0.13],
        dates: dates.clone(),
    });

    assert_roundtrip_value(&DatedSeries {
        values: vec![1.5, 1.6, 1.7],
        dates,
    });

    assert_roundtrip_value(&AnnualizationConvention::Act365_25);
}
