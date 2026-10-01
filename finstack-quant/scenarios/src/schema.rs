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

/// The metadata of the built-in 2008 global-financial-crisis template.
fn template_metadata_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let registry = crate::templates::TemplateRegistry::embedded_builtins()?;
    let template = registry.get("gfc_2008").ok_or_else(|| {
        finstack_quant_core::Error::Internal("built-in template gfc_2008 is missing".to_string())
    })?;
    finstack_quant_core::schema::example(template.metadata())
}

/// One applied operation against an otherwise empty market.
///
/// Small on purpose: it shows the report keys a caller reads back without
/// carrying a calibrated market snapshot.
fn application_envelope_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    let market = serde_json::to_value(
        finstack_quant_core::market_data::context::MarketContextState::from(
            &finstack_quant_core::market_data::context::MarketContext::new(),
        ),
    )
    .map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize example market: {error}"))
    })?;
    finstack_quant_core::schema::example(&crate::ApplicationEnvelope {
        market,
        model: None,
        instruments: None,
        operations_applied: 1,
        user_operations: 1,
        expanded_operations: 1,
        changes: Default::default(),
        warnings: Vec::new(),
        meta: Some(finstack_quant_core::config::results_meta(
            &finstack_quant_core::config::FinstackConfig::default(),
        )),
        time_roll: None,
    })
}

/// A one-month horizon on a deposit whose whole P&L is carry.
fn horizon_result_example() -> finstack_quant_core::Result<crate::HorizonResult> {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;

    let date = |month, day| {
        finstack_quant_core::dates::Date::from_calendar_date(2024, month, day).map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example date: {error}"))
        })
    };
    let pnl = Money::from((12_500_i64, Currency::USD));
    let mut attribution = finstack_quant_attribution::PnlAttribution::new(
        pnl,
        "DEP_3M",
        date(time::Month::January, 1)?,
        date(time::Month::February, 1)?,
        Default::default(),
    );
    attribution.carry = pnl;
    attribution.residual = Money::from((0_i64, Currency::USD));
    Ok(crate::HorizonResult {
        attribution,
        initial_value: Money::from((1_000_000_i64, Currency::USD)),
        terminal_value: Money::from((1_012_500_i64, Currency::USD)),
        horizon_days: Some(31),
        scenario_report: crate::ApplicationReport {
            operations_applied: 1,
            user_operations: 1,
            expanded_operations: 1,
            changes: Default::default(),
            warnings: Vec::new(),
            meta: None,
            time_roll: None,
        },
    })
}

fn horizon_result_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    finstack_quant_core::schema::example(&horizon_result_example()?)
}

/// The return summary of [`horizon_result_example`].
fn horizon_summary_examples() -> finstack_quant_core::Result<Vec<serde_json::Value>> {
    finstack_quant_core::schema::example(&horizon_result_example()?.summary())
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
    .with_kind(finstack_quant_core::schema::SchemaKind::Output)
    .with_examples(template_metadata_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::ApplicationEnvelope>(
        "schemas/scenarios/1/application_envelope.schema.json",
        "https://finstack_quant.dev/schemas/scenarios/1/application_envelope.schema.json",
        "ApplicationEnvelope",
        "Scenario application result: mutated market, model and instruments with the report.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output)
    .with_examples(application_envelope_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::HorizonResult>(
        "schemas/scenarios/1/horizon_result.schema.json",
        "https://finstack_quant.dev/schemas/scenarios/1/horizon_result.schema.json",
        "HorizonResult",
        "Horizon total return: factor-decomposed P&L with scenario context.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output)
    .with_examples(horizon_result_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::HorizonSummary>(
        "schemas/scenarios/1/horizon_summary.schema.json",
        "https://finstack_quant.dev/schemas/scenarios/1/horizon_summary.schema.json",
        "HorizonSummary",
        "Derived total, annualized and per-factor returns of a horizon result.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output)
    .with_examples(horizon_summary_examples),
];
