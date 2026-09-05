//! Schema registry for the portfolio crate.
//!
//! The registry lives in the library rather than the generator binary so the
//! generator, the contract tests and the language bindings all render from one
//! definition. Render an entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate).

use finstack_quant_core::schema::{
    externalize_schema_definitions, ExternalSchemaDefinition, SchemaArtifact, SchemaKind,
};
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::InstrumentEnvelope;
use serde_json::Value;

const INSTRUMENT_SCHEMA_URI: &str =
    "https://finstack_quant.dev/schemas/instrument/1/instrument.schema.json";

const EXTERNAL_DEFINITIONS: &[ExternalSchemaDefinition] =
    &[ExternalSchemaDefinition::new::<InstrumentEnvelope>(
        "InstrumentEnvelope",
        INSTRUMENT_SCHEMA_URI,
    )];

/// Replace the embedded instrument envelope with its published reference.
///
/// # Arguments
///
/// * `schema` - Generated portfolio schema to repoint in place.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] if a replaced definition
/// is not assertion-equivalent to the published artifact.
pub fn package_materialization_schema(schema: &mut Value) -> Result<()> {
    externalize_schema_definitions(schema, EXTERNAL_DEFINITIONS)
}

/// A one-position portfolio bundle, materialized from the public builder.
///
/// Built from real constructors and then round-tripped through
/// [`Portfolio::to_materialization`](crate::Portfolio::to_materialization), so
/// the published example is a payload the loader actually accepts rather than
/// hand-written JSON that can drift.
fn materialization_examples() -> Result<Vec<Value>> {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use std::sync::Arc;

    let as_of = finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::January, 1)
        .map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example as-of date: {error}"))
        })?;
    let maturity =
        finstack_quant_core::dates::Date::from_calendar_date(2024, time::Month::February, 1)
            .map_err(|error| {
                finstack_quant_core::Error::Internal(format!(
                    "build example maturity date: {error}"
                ))
            })?;

    let deposit = Deposit::builder()
        .id("DEP_1M".into())
        .notional(Money::from((1000000_i64, Currency::USD)))
        .start_date(as_of)
        .maturity(maturity)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD-OIS".into())
        .build()
        .map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example deposit: {error}"))
        })?;

    let position = crate::Position::new(
        "POS_001",
        "ACME_CORP",
        "DEP_1M",
        Arc::new(deposit),
        1.0,
        crate::PositionUnit::Units,
    )
    .map_err(|error| {
        finstack_quant_core::Error::Internal(format!("build example position: {error}"))
    })?;

    let portfolio = crate::Portfolio::builder("EXAMPLE_FUND")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(crate::Entity::new("ACME_CORP"))
        .position(position)
        .build()
        .map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example portfolio: {error}"))
        })?;

    let envelope = portfolio.to_materialization().map_err(|error| {
        finstack_quant_core::Error::Internal(format!("materialize example portfolio: {error}"))
    })?;
    let value = serde_json::to_value(&envelope).map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize example portfolio: {error}"))
    })?;
    Ok(vec![value])
}

/// A canonical optimizer result: a single rebalancing trade.
fn optimization_result_examples() -> Result<Vec<Value>> {
    use indexmap::IndexMap;

    let position: crate::PositionId = "POS_001".into();
    let mut optimal = IndexMap::new();
    optimal.insert(position.clone(), 0.60);
    let mut current = IndexMap::new();
    current.insert(position.clone(), 0.55);
    let mut deltas = IndexMap::new();
    deltas.insert(position.clone(), 0.05);
    let mut quantities = IndexMap::new();
    quantities.insert(position.clone(), 600_000.0);
    let mut metrics = IndexMap::new();
    metrics.insert("expected_return".to_string(), 0.0425);
    let mut slacks = IndexMap::new();
    slacks.insert("max_weight".to_string(), 0.40);

    let result = crate::optimization::PortfolioOptimizationResultWire {
        schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
        status: crate::optimization::OptimizationStatus::Optimal,
        status_label: "optimal".to_string(),
        label: None,
        is_feasible: true,
        objective_value: 0.0425,
        turnover: 0.05,
        optimal_weights: optimal,
        current_weights: current,
        weight_deltas: deltas,
        implied_quantities: quantities,
        metric_values: metrics,
        trades: vec![crate::optimization::TradeSpec {
            position_id: position,
            instrument_id: "DEP_1M".to_string(),
            trade_type: crate::optimization::TradeType::Existing,
            current_quantity: 550_000.0,
            target_quantity: 600_000.0,
            delta_quantity: 50_000.0,
            direction: crate::optimization::TradeDirection::Buy,
            current_weight: 0.55,
            target_weight: 0.60,
        }],
        constraint_slacks: slacks,
        binding_constraints: Vec::new(),
    };
    let value = serde_json::to_value(&result).map_err(|error| {
        finstack_quant_core::Error::Internal(format!(
            "serialize optimization result example: {error}"
        ))
    })?;
    Ok(vec![value])
}

/// The crate's complete schema registry, sorted by artifact path.
pub const ARTIFACTS: &[SchemaArtifact] = &[
    SchemaArtifact::new::<crate::PortfolioMaterializationEnvelope>(
        "schemas/portfolio/1/portfolio_materialization.schema.json",
        "https://finstack_quant.dev/schemas/portfolio/1/portfolio_materialization.schema.json",
        "Portfolio Materialization Envelope",
        "Strict, versioned, content-addressed portfolio materialization bundle.",
    )
    .with_packager(package_materialization_schema)
    .with_kind(SchemaKind::Input)
    .with_summary("Positions, books and their instruments as one content-addressed bundle.")
    .with_examples(materialization_examples),
    SchemaArtifact::new::<crate::optimization::PortfolioOptimizationResultWire>(
        "schemas/portfolio/1/portfolio_optimization_result.schema.json",
        "https://finstack_quant.dev/schemas/portfolio/1/portfolio_optimization_result.schema.json",
        "Portfolio Optimization Result",
        "Canonical typed v1 output from portfolio optimization.",
    )
    .with_kind(SchemaKind::Output)
    .with_summary("Optimizer weights and diagnostics.")
    .with_examples(optimization_result_examples),
];
