//! Schema registry for the analytics crate.
//!
//! Every root serde contract the bindings exchange with a host is registered
//! here; the types it embeds are emitted as `$defs` of those roots. The
//! `gen_analytics_schemas` binary writes one JSON Schema per
//! entry, and the WASM package generates its TypeScript declarations from
//! them. Render an entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate).

use finstack_quant_core::dates::{Date, DayCount, PeriodKind};
use finstack_quant_core::schema::{SchemaArtifact, example};
use finstack_quant_core::{Error, Result};
use serde_json::Value;

use crate::{
    BetaResult, CagrDayCount, CorrelationResult, DatedSeries, DrawdownEpisode, GreeksResult,
    LookbackReturns, LookbackSample, MultiFactorResult, PanelSeriesKind, Performance,
    PerformancePanel, PeriodStats, PeriodicReturn, ReturnKind, RollingGreeks,
};

/// Six weekdays of daily prices for a fund and its benchmark.
///
/// Every computed example derives from this panel through the public
/// [`Performance`] API, so each published payload is one the contract itself
/// produced.
fn example_panel() -> Result<PerformancePanel> {
    let dates = [6, 7, 8, 9, 10, 13]
        .into_iter()
        .map(|day| Date::from_calendar_date(2025, time::Month::January, day))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| Error::Internal(format!("build example date: {error}")))?;
    Ok(PerformancePanel {
        dates,
        series: vec![
            vec![100.0, 101.0, 99.5, 100.5, 102.0, 101.5],
            vec![100.0, 100.6, 99.8, 100.3, 101.2, 101.0],
        ],
        kind: PanelSeriesKind::Prices,
        ticker_names: vec!["FUND".into(), "SPX".into()],
        benchmark_ticker: Some("SPX".into()),
        frequency: Some(PeriodKind::Daily),
    })
}

fn example_performance() -> Result<Performance> {
    Performance::new(&example_panel()?)
}

/// Publish the first entry of a per-ticker result (the fund).
fn first_example<T: serde::Serialize>(items: &[T], what: &str) -> Result<Vec<Value>> {
    let first = items
        .first()
        .ok_or_else(|| Error::Internal(format!("{what} example is empty")))?;
    example(first)
}

fn correlation_result_examples() -> Result<Vec<Value>> {
    example(&example_performance()?.correlation_matrix()?)
}

fn lookback_returns_value() -> Result<LookbackReturns> {
    example_performance()?.lookback_returns(None, None)
}

fn lookback_sample_examples() -> Result<Vec<Value>> {
    first_example(
        &lookback_returns_value()?.mtd_samples,
        "month-to-date sample",
    )
}

fn beta_result_examples() -> Result<Vec<Value>> {
    first_example(&example_performance()?.beta()?, "beta")
}

fn cagr_day_count_examples() -> Result<Vec<Value>> {
    let mut examples = example(&CagrDayCount::Act365_25)?;
    examples.extend(example(&CagrDayCount::DayCount(DayCount::Act365F))?);
    Ok(examples)
}

fn dated_series_examples() -> Result<Vec<Value>> {
    example(&example_performance()?.rolling_volatility(0, 3)?)
}

fn drawdown_episode_examples() -> Result<Vec<Value>> {
    first_example(
        &example_performance()?.drawdown_details(0, 1)?,
        "drawdown episode",
    )
}

fn greeks_result_examples() -> Result<Vec<Value>> {
    first_example(&example_performance()?.greeks(0.0)?, "greeks")
}

fn lookback_returns_examples() -> Result<Vec<Value>> {
    example(&lookback_returns_value()?)
}

/// The fund regressed on market and size factor returns, one per interval.
fn multi_factor_result_examples() -> Result<Vec<Value>> {
    let market = [0.006, -0.008, 0.005, 0.009, -0.002];
    let size = [0.001, 0.002, -0.003, 0.001, 0.002];
    example(&example_performance()?.multi_factor_greeks(0, &[&market, &size], 0.0)?)
}

fn performance_panel_examples() -> Result<Vec<Value>> {
    example(&example_panel()?)
}

fn performance_examples() -> Result<Vec<Value>> {
    example(&example_performance()?)
}

fn period_stats_examples() -> Result<Vec<Value>> {
    example(&example_performance()?.period_stats(0, PeriodKind::Weekly, None)?)
}

fn periodic_return_examples() -> Result<Vec<Value>> {
    let series = example_performance()?.periodic_returns(PeriodKind::Weekly)?;
    let fund = series
        .first()
        .ok_or_else(|| Error::Internal("periodic return example is empty".into()))?;
    first_example(fund, "periodic return")
}

fn return_kind_examples() -> Result<Vec<Value>> {
    let mut examples = example(&ReturnKind::Excess)?;
    examples.extend(example(&ReturnKind::Total {
        risk_free_rate: 0.02,
    })?);
    Ok(examples)
}

fn rolling_greeks_examples() -> Result<Vec<Value>> {
    example(&example_performance()?.rolling_greeks(0, 3, 0.0)?)
}

/// The crate's complete schema registry.
pub const ARTIFACTS: &[SchemaArtifact] = &[
    finstack_quant_core::schema_artifact!(
        CorrelationResult,
        "analytics",
        "correlation_result",
        Output,
        "Common-sample Pearson correlation with ticker and sample provenance."
    )
    .with_examples(correlation_result_examples),
    finstack_quant_core::schema_artifact!(
        LookbackSample,
        "analytics",
        "lookback_sample",
        Component,
        "Observation count and actual holding bounds for a lookback return."
    )
    .with_examples(lookback_sample_examples),
    finstack_quant_core::schema_artifact!(
        BetaResult,
        "analytics",
        "beta_result",
        Output,
        "OLS beta result with standard error and 95% confidence interval."
    )
    .with_examples(beta_result_examples),
    finstack_quant_core::schema_artifact!(
        CagrDayCount,
        "analytics",
        "cagr_day_count",
        Component,
        "Day-count convention for CAGR annualization over explicit calendar dates."
    )
    .with_examples(cagr_day_count_examples),
    finstack_quant_core::schema_artifact!(
        DatedSeries,
        "analytics",
        "dated_series",
        Component,
        "A dated time-series column: scalar values aligned with window-end dates."
    )
    .with_examples(dated_series_examples),
    finstack_quant_core::schema_artifact!(
        DrawdownEpisode,
        "analytics",
        "drawdown_episode",
        Component,
        "Drawdown episode with start, valley, optional recovery, and max drawdown."
    )
    .with_examples(drawdown_episode_examples),
    finstack_quant_core::schema_artifact!(
        GreeksResult,
        "analytics",
        "greeks_result",
        Output,
        "Greeks (alpha, beta, R-squared, adjusted R-squared) from a single-factor regression."
    )
    .with_examples(greeks_result_examples),
    finstack_quant_core::schema_artifact!(
        LookbackReturns,
        "analytics",
        "lookback_returns",
        Component,
        "Lookback returns for each period horizon."
    )
    .with_examples(lookback_returns_examples),
    finstack_quant_core::schema_artifact!(
        MultiFactorResult,
        "analytics",
        "multi_factor_result",
        Output,
        "Result of a multi-factor regression."
    )
    .with_examples(multi_factor_result_examples),
    finstack_quant_core::schema_artifact!(
        PerformancePanel,
        "analytics",
        "performance_panel",
        Input,
        "Dated price or return panel, one series per ticker, that a Performance is built from."
    )
    .with_examples(performance_panel_examples),
    finstack_quant_core::schema_artifact!(
        Performance,
        "analytics",
        "performance",
        Input,
        "Central performance analytics engine."
    )
    .with_examples(performance_examples),
    finstack_quant_core::schema_artifact!(
        PeriodStats,
        "analytics",
        "period_stats",
        Output,
        "Period-level aggregate statistics."
    )
    .with_examples(period_stats_examples),
    finstack_quant_core::schema_artifact!(
        PeriodicReturn,
        "analytics",
        "periodic_return",
        Component,
        "One calendar bucket of a periodic-return series."
    )
    .with_examples(periodic_return_examples),
    finstack_quant_core::schema_artifact!(
        ReturnKind,
        "analytics",
        "return_kind",
        Component,
        "How the dependent return series is interpreted in a multi-factor regression."
    )
    .with_examples(return_kind_examples),
    finstack_quant_core::schema_artifact!(
        RollingGreeks,
        "analytics",
        "rolling_greeks",
        Component,
        "Rolling greeks output."
    )
    .with_examples(rolling_greeks_examples),
];
