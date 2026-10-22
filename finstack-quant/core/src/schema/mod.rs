//! Deterministic JSON Schema assembly helpers.

use serde_json::Value;

use crate::dates::{PeriodPlan, Schedule, ScheduleSpec};
use crate::market_data::context::MarketContextState;
use crate::market_data::surfaces::VolCubeExpirySlice;
use crate::rating_scales::ScorecardScale;
use crate::table::TableEnvelope;
use crate::{Error, Result};

mod externalize;
mod generator;
mod llm;
mod registry;
#[cfg(test)]
mod tests;

pub use externalize::{externalize_schema_definitions, ExternalSchemaDefinition};
pub use generator::{
    build_schema_index, deterministic_json_bytes, run_schema_generator, schema_index_row,
    sync_generated_files, SchemaGenerationCommand, SchemaGenerationMode, SCHEMA_INDEX_VERSION,
};
pub use llm::{project_llm, LlmProfile, DEFAULT_MAX_INLINE_BYTES, RESOLVES_FROM_KEYWORD};
pub use registry::{
    example, example_from_json, find_schema_artifact, SchemaArtifact, SchemaKind, SerdeSchema,
    COMMON_SCHEMA_BASE, COMMON_SCHEMA_DEFINITIONS, JSON_SCHEMA_DIALECT,
};

/// Register one schema artifact for a contract type under a crate's family.
///
/// Expands to a [`SchemaArtifact`] at
/// `schemas/<family>/1/<file>.schema.json` whose `$id` is the matching
/// `https://finstack_quant.dev/schemas/<family>/1/<file>.schema.json`. The
/// title is the Rust type name (or the trailing `title = "..."` argument),
/// the description is `summary`, and the index kind is the named
/// [`SchemaKind`] variant.
///
/// # Arguments
///
/// * `$ty` - Contract type in scope at the call site; must implement
///   [`SerdeSchema`].
/// * `$family` - Schema family directory (for example `"analytics"`).
/// * `$file` - Snake-case file stem inside the family's `1/` directory.
/// * `$kind` - [`SchemaKind`] variant name: `Input`, `Output` or `Component`.
/// * `$summary` - One sentence describing the contract.
/// * `title = $title` - Optional schema title when it differs from the type name.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::market_data::context::MarketContextState;
/// use finstack_quant_core::schema::SchemaArtifact;
///
/// const ENTRY: SchemaArtifact = finstack_quant_core::schema_artifact!(
///     MarketContextState,
///     "market_data",
///     "market_context_state",
///     Input,
///     "Complete market-data snapshot."
/// );
/// assert_eq!(ENTRY.title, "MarketContextState");
/// ```
#[macro_export]
macro_rules! schema_artifact {
    ($ty:ident, $family:literal, $file:literal, $kind:ident, $summary:literal) => {
        $crate::schema_artifact!(
            $ty,
            $family,
            $file,
            $kind,
            $summary,
            title = stringify!($ty)
        )
    };
    ($ty:ident, $family:literal, $file:literal, $kind:ident, $summary:literal, title = $title:expr) => {
        $crate::schema::SchemaArtifact::new::<$ty>(
            concat!("schemas/", $family, "/1/", $file, ".schema.json"),
            concat!(
                "https://finstack_quant.dev/schemas/",
                $family,
                "/1/",
                $file,
                ".schema.json"
            ),
            $title,
            $summary,
        )
        .with_kind($crate::schema::SchemaKind::$kind)
    };
}

/// A valid but empty market snapshot.
///
/// Deliberately minimal: its job is to show the required-key shape, including
/// the mandatory `hierarchy` key whose value may be an explicit `null`.
fn market_context_state_examples() -> Result<Vec<Value>> {
    let state = crate::market_data::context::MarketContextState::try_from(
        &crate::market_data::context::MarketContext::new(),
    )?;
    let value = serde_json::to_value(&state)
        .map_err(|error| Error::Internal(format!("serialize market context example: {error}")))?;
    Ok(vec![value])
}

/// A two-row table: one string dimension and one float measure.
fn table_envelope_examples() -> Result<Vec<Value>> {
    use crate::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
    let table = TableEnvelope::new(vec![
        TableColumn::new(
            "ticker",
            TableColumnData::String(vec!["AAPL".to_string(), "MSFT".to_string()]),
        )
        .with_role(TableColumnRole::Dimension),
        TableColumn::new("weight", TableColumnData::Float64(vec![0.6, 0.4]))
            .with_role(TableColumnRole::Measure),
    ])?;
    example(&table)
}

/// Two calendar quarters, the first marked actual.
fn period_plan_examples() -> Result<Vec<Value>> {
    let plan = crate::dates::build_periods("2025Q1..Q2", Some("2025Q1"))?;
    example(&plan)
}

/// A one-year schedule with the builder defaults.
fn schedule_spec_example() -> Result<crate::dates::ScheduleSpec> {
    let start = crate::dates::create_date(2025, time::Month::January, 15)?;
    let end = crate::dates::create_date(2026, time::Month::January, 15)?;
    crate::dates::ScheduleSpec::new(start, end)
}

/// The persisted inputs of [`schedule_examples`].
fn schedule_spec_examples() -> Result<Vec<Value>> {
    example(&schedule_spec_example()?)
}

/// The schedule generated from [`schedule_spec_example`].
fn schedule_examples() -> Result<Vec<Value>> {
    example(&crate::dates::ScheduleBuilder::from(schedule_spec_example()?).build()?)
}

/// The default scorecard scale of the rating-scale registry compiled into the library.
fn scorecard_scale_examples() -> Result<Vec<Value>> {
    let registry = crate::rating_scales::embedded_registry()?;
    example(registry.rating_scale(registry.default_scale_id())?)
}

/// A one-year-expiry normal-volatility slice over two tenors and three strikes.
fn vol_cube_expiry_slice_examples() -> Result<Vec<Value>> {
    use crate::market_data::surfaces::{VolCubeExpirySlice, VolQuoteType};
    let slice = VolCubeExpirySlice::from_grid(
        "USD-SWAPTION-1Y",
        1.0,
        &[2.0, 5.0],
        &[0.02, 0.03, 0.04],
        &[0.0085, 0.008, 0.0082, 0.009, 0.0086, 0.0088],
        VolQuoteType::Normal,
        None,
    )?;
    example(&slice)
}

/// The core crate's schema registry.
///
/// This lives beside the emitter rather than in the generator binary, so the
/// generator, the contract tests and the bindings all render from one
/// definition. Render an entry with [`SchemaArtifact::generate`].
pub const ARTIFACTS: &[SchemaArtifact] = &[
    crate::schema_artifact!(
    MarketContextState,
    "market_data",
    "market_context_state",
    Input,
    "Canonical v1 persisted snapshot of a complete market-data context.",
    title = "Market Context State"
)
    .with_summary(
        "Curves, surfaces, prices, series and FX for one valuation date; the market input to \
             every pricing, scenario and attribution call.",
    )
    .with_examples(market_context_state_examples),
    crate::schema_artifact!(
    TableEnvelope,
    "table",
    "table_envelope",
    Output,
    "Column-oriented table with a shared row count and typed column storage."
)
    .with_examples(table_envelope_examples),
    crate::schema_artifact!(
    PeriodPlan,
    "dates",
    "period_plan",
    Output,
    "Ordered reporting periods with their calendar bounds and actual/forecast flags."
)
    .with_examples(period_plan_examples),
    crate::schema_artifact!(
    ScheduleSpec,
    "dates",
    "schedule_spec",
    Input,
    "Persisted inputs of a date schedule: range, frequency, stub, adjustment and lags."
)
    .with_examples(schedule_spec_examples),
    crate::schema_artifact!(
    Schedule,
    "dates",
    "schedule",
    Output,
    "Generated accrual, payment and fixing dates with any construction warnings."
)
    .with_examples(schedule_examples),
    crate::schema_artifact!(
    ScorecardScale,
    "rating_scales",
    "scorecard_scale",
    Output,
    "Named scorecard rating scale: rating levels ordered best to worst with score thresholds."
)
    .with_examples(scorecard_scale_examples),
    crate::schema_artifact!(
    VolCubeExpirySlice,
    "market_data",
    "vol_cube_expiry_slice",
    Output,
    "Tenor-by-strike volatility quotes at a fixed option expiry, preserving quote convention and displacement.",
    title = "Volatility Cube Expiry Slice"
)
    .with_examples(vol_cube_expiry_slice_examples),
];

/// Shared diagnostics for compiled JSON Schema validators.
#[cfg(feature = "jsonschema-validate")]
pub mod diagnostics;
