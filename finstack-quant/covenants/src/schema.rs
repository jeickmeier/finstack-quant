//! Schema registry for the covenants crate.
//!
//! Every root serde contract the bindings exchange with a host is registered
//! here; the types it embeds are emitted as `$defs` of those roots. The
//! `gen_covenants_schemas` binary writes one JSON Schema per
//! entry, and the WASM package generates its TypeScript declarations from
//! them. Render an entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate).

use finstack_quant_core::schema::SchemaArtifact;

use crate::{
    ConsequenceApplication, CovenantEngine, CovenantForecast, CovenantForecastConfig,
    CovenantReport, DatedCovenantReports, DatedMetrics, FutureBreach, HashMapMetricSource,
};

/// The crate's complete schema registry.
pub const ARTIFACTS: &[SchemaArtifact] = &[
    finstack_quant_core::schema_artifact!(
        ConsequenceApplication,
        "covenants",
        "consequence_application",
        Component,
        "Result of applying a covenant consequence."
    ),
    finstack_quant_core::schema_artifact!(
        CovenantEngine,
        "covenants",
        "covenant_engine",
        Input,
        "Covenant engine for evaluation and consequence application."
    ),
    finstack_quant_core::schema_artifact!(
        CovenantForecast,
        "covenants",
        "covenant_forecast",
        Output,
        "Forecast output with headroom analytics."
    ),
    finstack_quant_core::schema_artifact!(
        CovenantForecastConfig,
        "covenants",
        "covenant_forecast_config",
        Input,
        "Covenant forecast configuration."
    ),
    finstack_quant_core::schema_artifact!(
        CovenantReport,
        "covenants",
        "covenant_report",
        Output,
        "Covenant check result."
    ),
    finstack_quant_core::schema_artifact!(
        DatedCovenantReports,
        "covenants",
        "dated_covenant_reports",
        Output,
        "Covenant reports produced for one test date of a series evaluation."
    ),
    finstack_quant_core::schema_artifact!(
        DatedMetrics,
        "covenants",
        "dated_metrics",
        Input,
        "Covenant metric values observed on one date."
    ),
    finstack_quant_core::schema_artifact!(
        FutureBreach,
        "covenants",
        "future_breach",
        Output,
        "A projected covenant breach."
    ),
    finstack_quant_core::schema_artifact!(
        HashMapMetricSource,
        "covenants",
        "hash_map_metric_source",
        Input,
        "Map-backed metric source for tests, bindings, and simple callers."
    ),
];
