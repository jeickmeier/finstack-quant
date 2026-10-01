//! Schema registry for the portfolio crate.
//!
//! The registry lives in the library rather than the generator binary so the
//! generator, the contract tests and the language bindings all render from one
//! definition. Render an entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate).

use crate::{
    attribution::ReconciliationReport, cashflows::CashflowAggregationOptions,
    cashflows::PortfolioCashflows, factor_model::CreditVolReport,
    factor_model::FactorAssignmentReport, factor_model::PositionChange, factor_model::StressPnl,
    factor_model::StressResult, factor_model::WhatIfResult,
    optimization::PortfolioOptimizationSpec, primitive::PortfolioPrimitiveExposureReport,
    replay::ReplayConfig, replay::ReplayResult, scenarios::ScenarioPnlBatchItem,
    scenarios::ScenarioPnlView, scenarios::ScenarioRevalueView, sensitivity::FactorPnlProfile,
    sensitivity::SensitivityMatrixJson, CarinoLinkedAttribution, CellConfig, DatedCashflow,
    DurationCellTable, ExcessReturnPosition, ExcessReturnResult, FactorBrinsonInput,
    FactorBrinsonResult, FiAttributionConfig, FiCarinoLinkedResult, FiPeriodInput,
    FiReconciliationReport, GridCarinoLinkedResult, GridPosition, LinkedReturn, MarketFactorKey,
    PortfolioMarginResult, PortfolioResult, ReferenceReturn, SectorPeriod, TwrrPeriod,
    WeightAllocationResult, WeightAllocationSpec,
};
use finstack_quant_core::schema::{
    externalize_schema_definitions, ExternalSchemaDefinition, SchemaArtifact, SchemaKind,
};
use finstack_quant_core::Result;
use finstack_quant_valuations::instruments::{InstrumentEnvelope, InstrumentJson};
use finstack_quant_valuations::results::ValuationResult;
use serde_json::Value;

const INSTRUMENT_SCHEMA_URI: &str =
    "https://finstack_quant.dev/schemas/instrument/1/instrument.schema.json";

const INSTRUMENT_JSON_SCHEMA_URI: &str =
    "https://finstack_quant.dev/schemas/common/1/instrument_json.schema.json";

const VALUATION_RESULT_SCHEMA_URI: &str =
    "https://finstack_quant.dev/schemas/results/1/valuation_result.schema.json";

const EXTERNAL_DEFINITIONS: &[ExternalSchemaDefinition] = &[
    ExternalSchemaDefinition::new::<InstrumentEnvelope>(
        "InstrumentEnvelope",
        INSTRUMENT_SCHEMA_URI,
    ),
    ExternalSchemaDefinition::new::<InstrumentJson>("InstrumentJson", INSTRUMENT_JSON_SCHEMA_URI),
    ExternalSchemaDefinition::new::<ValuationResult>(
        "ValuationResult",
        VALUATION_RESULT_SCHEMA_URI,
    ),
];

/// Replace embedded instrument and valuation-result definitions with references
/// to their published valuations artifacts.
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
    SchemaArtifact::new::<crate::materialization::MaterializationReport>(
        "schemas/portfolio/1/materialization_report.schema.json",
        "https://finstack_quant.dev/schemas/portfolio/1/materialization_report.schema.json",
        "Materialization Report",
        "Outcome metadata for one successful portfolio materialization.",
    )
    .with_kind(SchemaKind::Output)
    .with_summary("Retained diagnostics, bundle counts and phase timings from a portfolio load."),
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
    finstack_quant_core::schema_artifact!(
        CarinoLinkedAttribution,
        "portfolio",
        "carino_linked_attribution",
        Output,
        "Compounded portfolio vs."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        CashflowAggregationOptions,
        "portfolio",
        "cashflow_aggregation_options",
        Input,
        "Options for `aggregate_full_cashflows`."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        CellConfig,
        "portfolio",
        "cell_config",
        Input,
        "Configuration for `cell_returns_from_reference`."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        CreditVolReport,
        "portfolio",
        "credit_vol_report",
        Output,
        "Aggregated credit risk grouped by hierarchy level."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        DatedCashflow,
        "portfolio",
        "dated_cashflow",
        Input,
        "A dated cashflow amount for money-weighted return calculations."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        DurationCellTable,
        "portfolio",
        "duration_cell_table",
        Output,
        "A full duration-cell base-return curve built from a reference universe."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        ExcessReturnPosition,
        "portfolio",
        "excess_return_position",
        Input,
        "One position's beginning-of-period duration, weight and realized total return, the input to `excess_returns`."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        ExcessReturnResult,
        "portfolio",
        "excess_return_result",
        Output,
        "Per-position and portfolio-level duration-matched credit excess return result, produced by `excess_returns`."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        FactorAssignmentReport,
        "portfolio",
        "factor_assignment_report",
        Output,
        "Assignment results for a portfolio-level factor mapping pass."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        FactorBrinsonInput,
        "portfolio",
        "factor_brinson_input",
        Input,
        "Inputs to `factor_brinson_attribution`: per-asset returns, a factor exposure matrix, and portfolio/benchmark weights."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        FactorBrinsonResult,
        "portfolio",
        "factor_brinson_result",
        Output,
        "Factor-Brinson unified attribution result."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        FactorPnlProfile,
        "portfolio",
        "factor_pnl_profile",
        Output,
        "P&L profile for one factor across a scenario grid."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        FiAttributionConfig,
        "portfolio",
        "fi_attribution_config",
        Input,
        "Configuration for `campisi_attribution`."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        FiCarinoLinkedResult,
        "portfolio",
        "fi_carino_linked_result",
        Output,
        "Multi-period Carino-linked Campisi attribution."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        FiPeriodInput,
        "portfolio",
        "fi_period_input",
        Input,
        "One attribution period's raw inputs for multi-period linking."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        FiReconciliationReport,
        "portfolio",
        "fi_reconciliation_report",
        Output,
        "Report from reconciling the five effect totals against the active return, mirroring `crate::attribution` reconciliation conventions."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        GridCarinoLinkedResult,
        "portfolio",
        "grid_carino_linked_result",
        Output,
        "Multi-period Carino-linked hierarchical grid attribution."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        GridPosition,
        "portfolio",
        "grid_position",
        Input,
        "One position (or pre-aggregated bucket) in a duration-cell x sector grid, for one period and one side (portfolio or benchmark)."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        LinkedReturn,
        "portfolio",
        "linked_return",
        Output,
        "Result of geometrically linking sub-period returns."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        MarketFactorKey,
        "portfolio",
        "market_factor_key",
        Component,
        "Normalized market factor key for portfolio-level dependency tracking."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        PortfolioCashflows,
        "portfolio",
        "portfolio_cashflows",
        Component,
        "Rich portfolio cashflow ladder preserving event classifications."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        PortfolioMarginResult,
        "portfolio",
        "portfolio_margin_result",
        Output,
        "Portfolio-wide margin calculation results."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        PortfolioOptimizationSpec,
        "portfolio",
        "portfolio_optimization_spec",
        Input,
        "JSON-serializable specification for a portfolio optimization problem."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        PortfolioPrimitiveExposureReport,
        "portfolio",
        "portfolio_primitive_exposure_report",
        Output,
        "Portfolio primitive decomposition retaining both path and concentration views."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        PortfolioResult,
        "portfolio",
        "portfolio_result",
        Output,
        "Complete results from portfolio evaluation."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        PositionChange,
        "portfolio",
        "position_change",
        Output,
        "Position edits supported by `WhatIfEngine::position_what_if`."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        ReconciliationReport,
        "portfolio",
        "reconciliation_report",
        Output,
        "Report from reconciling position-level P&L attribution against portfolio totals."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        ReferenceReturn,
        "portfolio",
        "reference_return",
        Input,
        "One reference (e.g."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        ReplayConfig,
        "portfolio",
        "replay_config",
        Input,
        "Configuration for a replay run."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        ReplayResult,
        "portfolio",
        "replay_result",
        Output,
        "Full output of a replay run."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        ScenarioPnlBatchItem,
        "portfolio",
        "scenario_pnl_batch_item",
        Output,
        "One ordered result from `scenario_pnl_batch`."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        ScenarioPnlView,
        "portfolio",
        "scenario_pnl_view",
        Output,
        "Scenario-P&L view returned by binding surfaces."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        ScenarioRevalueView,
        "portfolio",
        "scenario_revalue_view",
        Output,
        "Scenario-and-revalue view returned by binding surfaces."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        SectorPeriod,
        "portfolio",
        "sector_period",
        Input,
        "Per-sector portfolio and benchmark weights and returns for a single attribution period."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        SensitivityMatrixJson,
        "portfolio",
        "sensitivity_matrix_json",
        Output,
        "Canonical wire form of a factor-sensitivity matrix, shared by both hosts."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        StressPnl,
        "portfolio",
        "stress_pnl",
        Output,
        "P&L-only result of a factor-stress scenario."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        StressResult,
        "portfolio",
        "stress_result",
        Output,
        "Result of a factor-stress scenario."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        TwrrPeriod,
        "portfolio",
        "twrr_period",
        Output,
        "A single sub-period of a portfolio, with the information needed to compute a Modified-Dietz return."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        WeightAllocationResult,
        "portfolio",
        "weight_allocation_result",
        Output,
        "Strategy allocation result."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        WeightAllocationSpec,
        "portfolio",
        "weight_allocation_spec",
        Input,
        "JSON specification for strategy-level allocation."
    )
    .with_packager(package_materialization_schema),
    finstack_quant_core::schema_artifact!(
        WhatIfResult,
        "portfolio",
        "what_if_result",
        Output,
        "Result of a position what-if scenario."
    )
    .with_packager(package_materialization_schema),
];
