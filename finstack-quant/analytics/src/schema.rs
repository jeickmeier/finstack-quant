//! Schema registry for the analytics crate.
//!
//! Every root serde contract the bindings exchange with a host is registered
//! here; the types it embeds are emitted as `$defs` of those roots. The
//! `gen_analytics_schemas` binary writes one JSON Schema per
//! entry, and the WASM package generates its TypeScript declarations from
//! them. Render an entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate).

use finstack_quant_core::schema::SchemaArtifact;

use crate::{
    BetaResult, CagrDayCount, DatedSeries, DrawdownEpisode, GreeksResult, LookbackReturns,
    MultiFactorResult, Performance, PeriodStats, PeriodicReturn, ReturnKind, RollingGreeks,
};

/// The crate's complete schema registry.
pub const ARTIFACTS: &[SchemaArtifact] = &[
    finstack_quant_core::schema_artifact!(
        BetaResult,
        "analytics",
        "beta_result",
        Output,
        "OLS beta result with standard error and 95% confidence interval."
    ),
    finstack_quant_core::schema_artifact!(
        CagrDayCount,
        "analytics",
        "cagr_day_count",
        Component,
        "Day-count convention for CAGR annualization over explicit calendar dates."
    ),
    finstack_quant_core::schema_artifact!(
        DatedSeries,
        "analytics",
        "dated_series",
        Component,
        "A dated time-series column: scalar values aligned with window-end dates."
    ),
    finstack_quant_core::schema_artifact!(
        DrawdownEpisode,
        "analytics",
        "drawdown_episode",
        Component,
        "Drawdown episode with start, valley, optional recovery, and max drawdown."
    ),
    finstack_quant_core::schema_artifact!(
        GreeksResult,
        "analytics",
        "greeks_result",
        Output,
        "Greeks (alpha, beta, R-squared, adjusted R-squared) from a single-factor regression."
    ),
    finstack_quant_core::schema_artifact!(
        LookbackReturns,
        "analytics",
        "lookback_returns",
        Component,
        "Lookback returns for each period horizon."
    ),
    finstack_quant_core::schema_artifact!(
        MultiFactorResult,
        "analytics",
        "multi_factor_result",
        Output,
        "Result of a multi-factor regression."
    ),
    finstack_quant_core::schema_artifact!(
        Performance,
        "analytics",
        "performance",
        Input,
        "Central performance analytics engine."
    ),
    finstack_quant_core::schema_artifact!(
        PeriodStats,
        "analytics",
        "period_stats",
        Output,
        "Period-level aggregate statistics."
    ),
    finstack_quant_core::schema_artifact!(
        PeriodicReturn,
        "analytics",
        "periodic_return",
        Component,
        "One calendar bucket of a periodic-return series."
    ),
    finstack_quant_core::schema_artifact!(
        ReturnKind,
        "analytics",
        "return_kind",
        Component,
        "How the dependent return series is interpreted in a multi-factor regression."
    ),
    finstack_quant_core::schema_artifact!(
        RollingGreeks,
        "analytics",
        "rolling_greeks",
        Component,
        "Rolling greeks output."
    ),
];
