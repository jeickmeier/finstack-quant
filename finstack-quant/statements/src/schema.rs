//! Canonical JSON Schema registry owned by the statements crate.

use serde_json::Value;

use crate::adjustments::{NormalizationConfig, NormalizationResult};
use crate::checks::CheckSuiteSpec;
use crate::evaluator::{
    DatedCell, MonteCarloConfig, MonteCarloResults, PricingFlowSelection, RollupSpec,
    StatementResult,
};
use crate::registry::MetricRegistry;
use crate::types::{FinancialModelSpec, ForecastMethod, ForecastSpec, ModelEdit};

/// Stable base URI for statements-owned schemas.
pub const STATEMENTS_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/statements/1/";

/// A runnable two-quarter model with one value node and one formula node.
///
/// Built with the model builder, so the published example is a model the
/// loader admits and the evaluator runs.
fn financial_model_examples() -> finstack_quant_core::Result<Vec<Value>> {
    use finstack_quant_core::dates::PeriodId;

    let mut builder = crate::builder::ModelBuilder::new("example_model");
    builder.periods("2024Q1..Q2", None)?;
    builder.value(
        "revenue",
        &[
            (PeriodId::quarter(2024, 1)?, 100.0.into()),
            (PeriodId::quarter(2024, 2)?, 110.0.into()),
        ],
    );
    builder.compute("gross_profit", "revenue * 0.4")?;
    let model = builder.build()?;

    let value = serde_json::to_value(&model).map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize financial model example: {error}"))
    })?;
    Ok(vec![value])
}

/// A canonical EBITDA add-back configuration.
///
/// Built from the public types rather than hand-written JSON so the published
/// example cannot drift from the contract it illustrates.
fn normalization_examples() -> finstack_quant_core::Result<Vec<Value>> {
    use crate::adjustments::{Adjustment, AdjustmentValue};

    let mut amounts = indexmap::IndexMap::new();
    for (label, amount) in [("2024Q1", 1_500_000.0), ("2024Q2", 1_250_000.0)] {
        let period: finstack_quant_core::dates::PeriodId = label.parse().map_err(|error| {
            finstack_quant_core::Error::Internal(format!("parse example period {label}: {error}"))
        })?;
        amounts.insert(
            period,
            crate::types::AmountOrScalar::amount(
                amount,
                finstack_quant_core::currency::Currency::USD,
            )?,
        );
    }

    let config = NormalizationConfig {
        target_node: "ebitda".to_string(),
        output_node: "adjusted_ebitda".to_string(),
        adjustments: vec![
            Adjustment {
                id: "restructuring".to_string(),
                name: "Restructuring charges".to_string(),
                category: Some("non_recurring".to_string()),
                value: AdjustmentValue::Fixed { amounts },
                cap: None,
            },
            Adjustment {
                id: "run_rate_synergies".to_string(),
                name: "Run-rate cost synergies".to_string(),
                category: Some("synergies".to_string()),
                value: AdjustmentValue::PercentageOfNode {
                    node_id: "revenue".to_string(),
                    percentage: 0.02,
                },
                cap: None,
            },
        ],
    };

    let value = serde_json::to_value(&config).map_err(|error| {
        finstack_quant_core::Error::Internal(format!("serialize normalization example: {error}"))
    })?;
    Ok(vec![value])
}

/// Canonical `StatementResult`: an empty evaluation envelope.
///
/// Minimal on purpose - its job is to show the envelope shape and the policy
/// stamps a caller reads back, not to carry a worked model.
/// One node over two periods: a monetary value cell and a missing cell.
fn statement_result_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<crate::evaluator::StatementResult>(
        serde_json::json!({
            "schema_version": 1,
            "cells": {
                "revenue": {
                    "2025Q1": {
                        "status": "value",
                        "value": { "amount": "125.01", "currency": "USD" },
                        "source": "value"
                    },
                    "2025Q2": { "status": "missing" }
                }
            },
            "node_value_types": { "revenue": { "type": "monetary", "currency": "USD" } },
            "meta": { "num_nodes": 1, "num_periods": 2 }
        }),
    )
}

/// The method tag on its own.
fn forecast_method_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<ForecastMethod>(serde_json::json!(
        "growth_pct"
    ))
}

/// Fade a line to a target level, the shape every parameterized method takes.
fn forecast_spec_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<ForecastSpec>(serde_json::json!({
        "method": "fade_to_target",
        "params": { "target": 0.35, "shape": "linear" }
    }))
}

fn pricing_flow_selection_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<PricingFlowSelection>(serde_json::json!({
        "node_id": "capex",
        "currency": "USD",
        "sign": "outflow_magnitude",
        "periods": ["2025M03", "2025M05"],
        "date_convention": "end"
    }))
}

fn dated_cell_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<DatedCell>(serde_json::json!({
        "date": "2025-03-31",
        "period_id": "2025M03",
        "cell": {
            "status": "value",
            "value": { "amount": "125.01", "currency": "USD" },
            "source": "value"
        }
    }))
}

fn model_edit_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<ModelEdit>(serde_json::json!({
        "edit": "set_observation",
        "node_id": "revenue",
        "period": "2025Q1",
        "value": 110.0,
        "availability_date": "2025-05-15"
    }))
}

fn check_suite_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<crate::checks::CheckSuiteSpec>(
        serde_json::json!({
            "name": "three_statement_integrity",
            "description": "Balance sheet articulation and a leverage sanity bound.",
            "builtin_checks": [{
                "type": "balance_sheet_articulation",
                "assets_nodes": ["total_assets"],
                "liabilities_nodes": ["total_liabilities"],
                "equity_nodes": ["total_equity"]
            }],
            "formula_checks": [{
                "id": "leverage_bound",
                "name": "Net leverage below 8x",
                "category": "credit_reasonableness",
                "severity": "warning",
                "mode": "predicate",
                "scope": "all_periods",
                "formula": "net_debt / ebitda < 8",
                "message_template": "Net leverage exceeds 8x"
            }]
        }),
    )
}

/// A one-metric registry in the `fin` namespace.
fn metric_registry_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<crate::registry::MetricRegistry>(
        serde_json::json!({
            "namespace": "fin",
            "schema_version": 1,
            "metrics": [{
                "id": "gross_margin",
                "name": "Gross Margin",
                "formula": "gross_profit / revenue",
                "description": "Gross profit as a share of revenue.",
                "category": "margins",
                "unit_type": "percentage",
                "requires": ["gross_profit", "revenue"]
            }]
        }),
    )
}

/// A seeded 1,000-path run reporting the 5th, 50th and 95th percentiles of
/// revenue.
fn monte_carlo_config_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<crate::evaluator::MonteCarloConfig>(
        serde_json::json!({
            "n_paths": 1000,
            "seed": "42",
            "metrics": ["revenue"],
            "percentiles": [0.05, 0.5, 0.95]
        }),
    )
}

/// The percentile bands of one metric over two forecast quarters.
fn monte_carlo_results_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<crate::evaluator::MonteCarloResults>(
        serde_json::json!({
            "percentile_results": {
                "revenue": {
                    "metric": "revenue",
                    "value_type": {"type": "scalar"},
                    "sample_count": 1000,
                    "values": {
                        "2025Q1": [[0.05, 940_000.0], [0.5, 1_000_000.0], [0.95, 1_065_000.0]],
                        "2025Q2": [[0.05, 925_000.0], [0.5, 1_020_000.0], [0.95, 1_120_000.0]]
                    }
                }
            },
            "n_paths": 1000,
            "seed": "42",
            "stream_version": 1,
            "metrics": ["revenue"],
            "percentiles": [0.05, 0.5, 0.95],
            "forecast_periods": ["2025Q1", "2025Q2"]
        }),
    )
}

/// Quarterly results rolled up to annual: cash is a balance, margin a ratio.
fn rollup_spec_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<crate::evaluator::RollupSpec>(
        serde_json::json!({
            "frequency": "annual",
            "methods": {"cash": "period_end", "ebitda_margin": "formula"}
        }),
    )
}

/// One quarter of EBITDA with a single restructuring add-back.
fn normalization_result_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<crate::adjustments::NormalizationResult>(
        serde_json::json!({
            "value_type": {"type": "monetary", "currency": "USD"},
            "period": "2024Q1",
            "base_value": {"amount": "12000000", "currency": "USD"},
            "adjustments": [{
                "adjustment_id": "restructuring",
                "name": "Restructuring charges",
                "raw_amount": {"amount": "1500000", "currency": "USD"},
                "capped_amount": {"amount": "1500000", "currency": "USD"},
                "is_capped": false,
                "adjusted_value": {"amount": "13500000", "currency": "USD"}
            }],
            "final_value": {"amount": "13500000", "currency": "USD"}
        }),
    )
}

/// The crate's complete schema registry, in declaration order.
///
/// The shared renderer owns the order artifacts are published in.
///
/// This lives in the library, not the generator binary, so the generator, the
/// contract tests and the bindings all render from one definition. Render an
/// entry with [`finstack_quant_core::schema::SchemaArtifact::generate`].
pub const ARTIFACTS: &[finstack_quant_core::schema::SchemaArtifact] = &[
    finstack_quant_core::schema_artifact!(
        CheckSuiteSpec,
        "statements",
        "check_suite_spec",
        Input,
        "Serializable check suite: built-in checks, formula checks and filtering configuration."
    )
    .with_examples(check_suite_examples),
    finstack_quant_core::schema_artifact!(
        ForecastSpec,
        "statements",
        "forecast_spec",
        Input,
        "One forecast method with its typed parameters, in the wire shape of ForecastSpec."
    )
    .with_examples(forecast_spec_examples),
    finstack_quant_core::schema_artifact!(
        ForecastMethod,
        "statements",
        "forecast_method",
        Component,
        "The method tag of a forecast specification."
    )
    .with_examples(forecast_method_examples),
    finstack_quant_core::schema_artifact!(
        FinancialModelSpec,
        "statements",
        "financial_model_spec",
        Input,
        "Versioned financial statement model specification."
    )
    .with_summary(
        "Statement model graph: periods, nodes and the Value > Forecast > Formula precedence.",
    )
    .with_examples(financial_model_examples),
    finstack_quant_core::schema_artifact!(
        NormalizationConfig,
        "statements",
        "normalization_config",
        Component,
        "Financial statement normalization policy and adjustment catalog."
    )
    .with_summary("Add-back and deduction catalog applied to a target metric such as EBITDA.")
    .with_examples(normalization_examples),
    finstack_quant_core::schema_artifact!(
        StatementResult,
        "statements",
        "statement_result",
        Output,
        "Versioned financial statement evaluation result."
    )
    .with_summary("Evaluated node values per period, with policy stamps.")
    .with_examples(statement_result_examples),
    finstack_quant_core::schema_artifact!(
        MetricRegistry,
        "statements",
        "metric_registry",
        Input,
        "Namespaced catalog of reusable metric definitions."
    )
    .with_examples(metric_registry_examples),
    finstack_quant_core::schema_artifact!(
        MonteCarloConfig,
        "statements",
        "monte_carlo_config",
        Input,
        "Path count, seed and reported percentiles of a Monte Carlo statement-model evaluation."
    )
    .with_examples(monte_carlo_config_examples),
    finstack_quant_core::schema_artifact!(
        MonteCarloResults,
        "statements",
        "monte_carlo_results",
        Output,
        "Percentile summaries of a Monte Carlo statement-model evaluation."
    )
    .with_examples(monte_carlo_results_examples),
    finstack_quant_core::schema_artifact!(
        NormalizationResult,
        "statements",
        "normalization_result",
        Output,
        "Per-period normalization outcome: base value, applied adjustments and final value."
    )
    .with_examples(normalization_result_examples),
    finstack_quant_core::schema_artifact!(
        RollupSpec,
        "statements",
        "rollup_spec",
        Input,
        "Target frequency and per-node methods for rolling statement results up to coarser periods."
    )
    .with_examples(rollup_spec_examples),
    finstack_quant_core::schema_artifact!(
        PricingFlowSelection,
        "statements",
        "pricing_flow_selection",
        Input,
        "The monetary node, expected currency, sign convention, required periods and anchor date a pricing caller reads."
    )
    .with_examples(pricing_flow_selection_examples),
    finstack_quant_core::schema_artifact!(
        DatedCell,
        "statements",
        "dated_cell",
        Output,
        "One period of a dated statement export: the anchor date, the period and its typed cell."
    )
    .with_examples(dated_cell_examples),
    finstack_quant_core::schema_artifact!(
        ModelEdit,
        "statements",
        "model_edit",
        Input,
        "One named change to a financial model, applied atomically with the others in its batch."
    )
    .with_examples(model_edit_examples),
];
