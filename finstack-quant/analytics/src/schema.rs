//! Schema registry for the analytics crate.
//!
//! Every root serde contract the bindings exchange with a host is registered
//! here; the types it embeds are emitted as `$defs` of those roots. The
//! `gen_analytics_schemas` binary writes one JSON Schema per
//! entry, and the WASM package generates its TypeScript declarations from
//! them. Render an entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate).

use finstack_quant_core::Result;
use finstack_quant_core::dates::{FiscalConfig, PeriodKind};
use finstack_quant_core::schema::{SchemaArtifact, example};
use time::macros::date;

use crate::{
    BetaResult, CagrDayCount, CorrelationResult, DatedSeries, DrawdownEpisode, GreeksResult,
    LookbackReturns, LookbackSample, MultiFactorResult, PanelSeriesKind, Performance,
    PerformancePanel, PeriodStats, PeriodicReturn, ReturnKind, RollingGreeks,
};

fn example_panel() -> PerformancePanel {
    PerformancePanel {
        dates: (0..8)
            .map(|day| date!(2025 - 01 - 01) + time::Duration::days(day))
            .collect(),
        series: vec![
            vec![100.0, 102.0, 99.0, 101.0, 103.0, 102.0, 104.0, 106.0],
            vec![100.0, 101.0, 100.0, 102.0, 101.0, 103.0, 104.0, 105.0],
        ],
        kind: PanelSeriesKind::Prices,
        ticker_names: vec!["FUND".into(), "BENCH".into()],
        benchmark_ticker: Some("BENCH".into()),
        frequency: Some(PeriodKind::Daily),
    }
}

fn example_performance() -> Result<Performance> {
    Performance::new(&example_panel())
}

fn example_lookbacks() -> Result<LookbackReturns> {
    example_performance()?.lookback_returns(None, Some(FiscalConfig::calendar_year()))
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
    .with_examples(|| example(&example_performance()?.correlation_matrix()?)),
    finstack_quant_core::schema_artifact!(
        LookbackSample,
        "analytics",
        "lookback_sample",
        Component,
        "Observation count and actual holding bounds for a lookback return."
    )
    .with_examples(|| example(&example_lookbacks()?.mtd_samples[0])),
    finstack_quant_core::schema_artifact!(
        BetaResult,
        "analytics",
        "beta_result",
        Output,
        "OLS beta result with standard error and 95% confidence interval."
    )
    .with_examples(|| example(&example_performance()?.beta()?[0])),
    finstack_quant_core::schema_artifact!(
        CagrDayCount,
        "analytics",
        "cagr_day_count",
        Component,
        "Day-count convention for CAGR annualization over explicit calendar dates."
    )
    .with_examples(|| example(&CagrDayCount::default())),
    finstack_quant_core::schema_artifact!(
        DatedSeries,
        "analytics",
        "dated_series",
        Component,
        "A dated time-series column: scalar values aligned with window-end dates."
    )
    .with_examples(|| example(&example_performance()?.rolling_returns(0, 3)?)),
    finstack_quant_core::schema_artifact!(
        DrawdownEpisode,
        "analytics",
        "drawdown_episode",
        Component,
        "Drawdown episode with start, valley, optional recovery, and max drawdown."
    )
    .with_examples(|| example(&example_performance()?.drawdown_details(0, 1)?[0])),
    finstack_quant_core::schema_artifact!(
        GreeksResult,
        "analytics",
        "greeks_result",
        Output,
        "Greeks (alpha, beta, R-squared, adjusted R-squared) from a single-factor regression."
    )
    .with_examples(|| example(&example_performance()?.greeks(0.0)?[0])),
    finstack_quant_core::schema_artifact!(
        LookbackReturns,
        "analytics",
        "lookback_returns",
        Component,
        "Lookback returns for each period horizon."
    )
    .with_examples(|| example(&example_lookbacks()?)),
    finstack_quant_core::schema_artifact!(
        MultiFactorResult,
        "analytics",
        "multi_factor_result",
        Output,
        "Result of a multi-factor regression."
    )
    .with_examples(|| {
        let performance = example_performance()?;
        let returns = performance.get_returns();
        example(&performance.multi_factor_greeks(0, &[&returns[1]], 0.0)?)
    }),
    finstack_quant_core::schema_artifact!(
        PerformancePanel,
        "analytics",
        "performance_panel",
        Input,
        "Dated price or return panel, one series per ticker, that a Performance is built from."
    )
    .with_examples(|| example(&example_panel())),
    finstack_quant_core::schema_artifact!(
        Performance,
        "analytics",
        "performance",
        Input,
        "Central performance analytics engine."
    )
    .with_examples(|| example(&example_performance()?)),
    finstack_quant_core::schema_artifact!(
        PeriodStats,
        "analytics",
        "period_stats",
        Output,
        "Period-level aggregate statistics."
    )
    .with_examples(|| example(&example_performance()?.period_stats(0, PeriodKind::Daily, None)?)),
    finstack_quant_core::schema_artifact!(
        PeriodicReturn,
        "analytics",
        "periodic_return",
        Component,
        "One calendar bucket of a periodic-return series."
    )
    .with_examples(|| {
        example(&example_performance()?.periodic_returns(PeriodKind::Monthly)?[0][0])
    }),
    finstack_quant_core::schema_artifact!(
        ReturnKind,
        "analytics",
        "return_kind",
        Component,
        "How the dependent return series is interpreted in a multi-factor regression."
    )
    .with_examples(|| {
        example(&ReturnKind::Total {
            risk_free_rate: 0.02,
        })
    }),
    finstack_quant_core::schema_artifact!(
        RollingGreeks,
        "analytics",
        "rolling_greeks",
        Component,
        "Rolling greeks output."
    )
    .with_examples(|| example(&example_performance()?.rolling_greeks(0, 3, 0.0)?)),
];
