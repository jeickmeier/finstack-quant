//! JSON Schema generation helpers for scenario contracts.

use finstack_quant_core::schema::SerdeSchema;
use serde_json::Value;

/// Stable base URI for scenario-owned schemas.
pub const SCENARIO_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/scenarios/1/";
/// Filename of the published scenario envelope schema.
pub const SCENARIO_SCHEMA_FILENAME: &str = "scenario.schema.json";
/// Canonical title of the published scenario envelope schema.
pub const SCENARIO_SCHEMA_TITLE: &str = "Finstack Quant Scenario Specification";
/// Canonical description of the published scenario envelope schema.
pub const SCENARIO_SCHEMA_DESCRIPTION: &str =
    "Versioned scenario specification with typed market, instrument, statement, and time-roll operations.";

/// Build the scenario-owned schema with canonical metadata.
///
/// This is public so the generator binary and schema parity integration test
/// use exactly the same assembly path.
///
/// # Arguments
///
/// * `filename` - Version-directory filename appended to
///   [`SCENARIO_SCHEMA_BASE`].
/// * `title` - Stable human-readable title for the generated root type.
/// * `description` - Stable description for the persisted contract.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Internal`] if schemars output cannot
/// be serialized as a JSON object.
#[doc(hidden)]
pub fn generated_schema<T: SerdeSchema>(
    filename: &str,
    title: &str,
    description: &str,
) -> finstack_quant_core::Result<Value> {
    finstack_quant_core::schema::generated_schema::<T>(
        SCENARIO_SCHEMA_BASE,
        filename,
        title,
        description,
    )
}

/// A canonical single-operation scenario: a 50 bp parallel shock.
fn scenario_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let scenario = crate::ScenarioSpec {
        id: "usd_rates_stress".into(),
        name: Some("USD +50bp parallel".into()),
        description: Some("Parallel shock to the USD discount curve.".into()),
        operations: vec![crate::OperationSpec::CurveParallelBp {
            curve_kind: crate::CurveKind::Discount,
            curve_id: "USD-OIS".into(),
            discount_curve_id: None,
            bp: 50.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
        hazard_bump_mode: Default::default(),
    };
    let value = serde_json::to_value(crate::ScenarioEnvelope::new(scenario)).map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize scenario example: {error}"))
    })?;
    Ok(vec![value])
}

/// The crate's complete schema registry.
///
/// This lives in the library, not the generator binary, so the generator, the
/// contract tests and the bindings all render from one definition. Render an
/// entry with [`finstack_quant_core::schema::SchemaArtifact::generate`];
/// `generated_schema` produces only the raw derived document.
pub const ARTIFACTS: &[finstack_quant_core::schema::SchemaArtifact] = &[
    finstack_quant_core::schema::SchemaArtifact::new::<crate::ScenarioEnvelope>(
        "schemas/scenarios/1/scenario.schema.json",
        "https://finstack_quant.dev/schemas/scenarios/1/scenario.schema.json",
        SCENARIO_SCHEMA_TITLE,
        SCENARIO_SCHEMA_DESCRIPTION,
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
    .with_summary("Ordered shock and roll operations over market, statement and valuation targets.")
    .with_examples(scenario_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::TemplateMetadata>(
        "schemas/scenarios/1/template_metadata.schema.json",
        "https://finstack_quant.dev/schemas/scenarios/1/template_metadata.schema.json",
        "TemplateMetadata",
        "Descriptive metadata of a built-in scenario template.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::ApplicationEnvelope>(
        "schemas/scenarios/1/application_envelope.schema.json",
        "https://finstack_quant.dev/schemas/scenarios/1/application_envelope.schema.json",
        "ApplicationEnvelope",
        "Scenario application result: mutated market, model and instruments with the report.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::HorizonResult>(
        "schemas/scenarios/1/horizon_result.schema.json",
        "https://finstack_quant.dev/schemas/scenarios/1/horizon_result.schema.json",
        "HorizonResult",
        "Horizon total return: factor-decomposed P&L with scenario context.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::HorizonSummary>(
        "schemas/scenarios/1/horizon_summary.schema.json",
        "https://finstack_quant.dev/schemas/scenarios/1/horizon_summary.schema.json",
        "HorizonSummary",
        "Derived total, annualized and per-factor returns of a horizon result.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output),
];
