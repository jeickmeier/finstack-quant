//! JSON-Schema packaging shared by every crate that publishes schemas built
//! on the valuations wire types.
//!
//! `package_valuations_schema` is the packager the umbrella crate's
//! `gen_schemas` binary and `finstack-quant-calibration` hand to
//! [`finstack_quant_core::schema::SchemaArtifact`]: it replaces the common,
//! cashflow and pricing-override definitions a generated schema embeds with
//! their published `$id`s.

use serde_json::Value;

const PRICING_OVERRIDE_SCHEMA_DEFINITIONS:
    &[finstack_quant_core::schema::ExternalSchemaDefinition] = &[
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<
        crate::instruments::InstrumentPricingOverrides,
    >(
        "InstrumentPricingOverrides",
        "https://finstack_quant.dev/schemas/common/1/instrument_pricing_overrides.schema.json",
    ),
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<
        crate::instruments::MetricPricingOverrides,
    >(
        "MetricPricingOverrides",
        "https://finstack_quant.dev/schemas/common/1/metric_pricing_overrides.schema.json",
    ),
    finstack_quant_core::schema::ExternalSchemaDefinition::new::<
        crate::instruments::ScenarioPricingOverrides,
    >(
        "ScenarioPricingOverrides",
        "https://finstack_quant.dev/schemas/common/1/scenario_pricing_overrides.schema.json",
    ),
];

/// Package a derived valuation schema using canonical shared definitions.
///
/// This pass changes only reference placement: common and cashflow definitions
/// are replaced by their equivalent published `$id`, then newly unreachable
/// local definitions are removed.
///
/// # Arguments
///
/// * `schema` - Complete schema generated from a valuation serde type.
///
/// # Errors
///
/// Returns the error of
/// [`finstack_quant_core::schema::externalize_schema_definitions`] when a
/// definition cannot be rewritten.
pub fn package_valuations_schema(schema: &mut Value) -> finstack_quant_core::Result<()> {
    let mut definitions = finstack_quant_core::schema::COMMON_SCHEMA_DEFINITIONS.to_vec();
    definitions.extend_from_slice(PRICING_OVERRIDE_SCHEMA_DEFINITIONS);
    definitions.extend_from_slice(finstack_quant_cashflows::schema::CASHFLOW_SCHEMA_DEFINITIONS);
    finstack_quant_core::schema::externalize_schema_definitions(schema, &definitions)
}
