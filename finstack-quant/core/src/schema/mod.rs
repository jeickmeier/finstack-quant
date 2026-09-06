//! Deterministic JSON Schema assembly helpers.

use serde_json::Value;

use crate::{Error, Result};

mod externalize;
mod generator;
mod llm;
mod registry;
#[cfg(test)]
mod tests;

pub use externalize::{externalize_schema_definitions, ExternalSchemaDefinition};
pub use generator::{
    build_schema_index, deterministic_json_bytes, run_schema_generator, run_schema_index_generator,
    schema_index_row, SchemaGenerationCommand, SchemaGenerationMode, SCHEMA_INDEX_VERSION,
};
pub use llm::{project_llm, LlmProfile, DEFAULT_MAX_INLINE_BYTES, RESOLVES_FROM_KEYWORD};
pub use registry::{
    find_schema_artifact, generated_schema, SchemaArtifact, SchemaKind, SerdeSchema,
    COMMON_SCHEMA_BASE, COMMON_SCHEMA_DEFINITIONS, JSON_SCHEMA_DIALECT,
};

/// A valid but empty market snapshot.
///
/// Deliberately minimal: its job is to show the required-key shape, including
/// the mandatory `hierarchy` key whose value may be an explicit `null`.
fn market_context_state_examples() -> Result<Vec<Value>> {
    let state = crate::market_data::context::MarketContextState::from(
        &crate::market_data::context::MarketContext::new(),
    );
    let value = serde_json::to_value(&state)
        .map_err(|error| Error::Internal(format!("serialize market context example: {error}")))?;
    Ok(vec![value])
}

/// The core crate's schema registry.
///
/// This lives beside the emitter rather than in the generator binary, so the
/// generator, the contract tests and the bindings all render from one
/// definition. Render an entry with [`SchemaArtifact::generate`].
pub const ARTIFACTS: &[SchemaArtifact] = &[SchemaArtifact::new::<
    crate::market_data::context::MarketContextState,
>(
    "schemas/market_data/1/market_context_state.schema.json",
    "https://finstack_quant.dev/schemas/market_data/1/market_context_state.schema.json",
    "Market Context State",
    "Canonical v1 persisted snapshot of a complete market-data context.",
)
.with_kind(SchemaKind::Input)
.with_summary(
    "Curves, surfaces, prices, series and FX for one valuation date; the market input to \
             every pricing, scenario and attribution call.",
)
.with_examples(market_context_state_examples)];
