//! Embedded JSON Schemas owned by the statements crate.

use std::sync::OnceLock;

use serde_json::Value;

use crate::{Error, Result};
use finstack_quant_core::schema::SerdeSchema;

/// Stable base URI for statements-owned schemas.
pub const STATEMENTS_SCHEMA_BASE: &str = "https://finstack_quant.dev/schemas/statements/1/";

/// Build one statements-owned schema with canonical metadata.
///
/// This is public only so the generator binary and schema parity integration
/// test use exactly the same assembly path.
///
/// # Arguments
///
/// * `filename` - Version-directory filename appended to
///   [`STATEMENTS_SCHEMA_BASE`].
/// * `title` - Stable schema title for the generated root type.
/// * `description` - Stable schema description for the persisted contract.
///
/// # Errors
///
/// Returns [`Error::Serde`] if the schemars output cannot be represented as a
/// JSON object.
#[doc(hidden)]
pub fn generated_schema<T: SerdeSchema>(
    filename: &str,
    title: &str,
    description: &str,
) -> Result<Value> {
    finstack_quant_core::schema::generated_schema::<T>(
        STATEMENTS_SCHEMA_BASE,
        filename,
        title,
        description,
    )
    .map_err(|error| Error::Serde(error.to_string()))
}

fn parse_schema(
    cache: &'static OnceLock<std::result::Result<Value, String>>,
    raw: &'static str,
    filename: &'static str,
) -> Result<&'static Value> {
    cache
        .get_or_init(|| {
            serde_json::from_str(raw)
                .map_err(|error| format!("invalid statements schema JSON at {filename}: {error}"))
        })
        .as_ref()
        .map_err(|error| Error::Serde(error.clone()))
}

/// Return the checked-in schema for [`crate::FinancialModelSpec`].
///
/// # Errors
///
/// Returns [`Error::Serde`] if the embedded schema JSON is malformed.
pub fn financial_model_spec_schema() -> Result<&'static Value> {
    static SCHEMA: OnceLock<std::result::Result<Value, String>> = OnceLock::new();
    parse_schema(
        &SCHEMA,
        include_str!("../schemas/statements/1/financial_model_spec.schema.json"),
        "financial_model_spec.schema.json",
    )
}

/// Return the checked-in schema for [`crate::evaluator::StatementResult`].
///
/// # Errors
///
/// Returns [`Error::Serde`] if the embedded schema JSON is malformed.
pub fn statement_result_schema() -> Result<&'static Value> {
    static SCHEMA: OnceLock<std::result::Result<Value, String>> = OnceLock::new();
    parse_schema(
        &SCHEMA,
        include_str!("../schemas/statements/1/statement_result.schema.json"),
        "statement_result.schema.json",
    )
}

/// Return the checked-in schema for
/// [`crate::adjustments::types::NormalizationConfig`].
///
/// # Errors
///
/// Returns [`Error::Serde`] if the embedded schema JSON is malformed.
pub fn normalization_config_schema() -> Result<&'static Value> {
    static SCHEMA: OnceLock<std::result::Result<Value, String>> = OnceLock::new();
    parse_schema(
        &SCHEMA,
        include_str!("../schemas/statements/1/normalization_config.schema.json"),
        "normalization_config.schema.json",
    )
}

/// A canonical two-period model with one value node and one formula node.
///
/// Small on purpose: it shows the graph shape and the Value > Forecast >
/// Formula precedence without carrying a worked forecast.
fn financial_model_examples() -> finstack_quant_core::Result<Vec<Value>> {
    use finstack_quant_core::dates::{Date, Period, PeriodId};

    let quarter = |label: &str, start: (i32, time::Month, u8), end: (i32, time::Month, u8)| {
        let id: PeriodId = label.parse().map_err(|error| {
            finstack_quant_core::Error::Internal(format!("parse example period {label}: {error}"))
        })?;
        let start = Date::from_calendar_date(start.0, start.1, start.2).map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example start date: {error}"))
        })?;
        let end = Date::from_calendar_date(end.0, end.1, end.2).map_err(|error| {
            finstack_quant_core::Error::Internal(format!("build example end date: {error}"))
        })?;
        Ok::<Period, finstack_quant_core::Error>(Period {
            id,
            start,
            end,
            is_actual: false,
        })
    };

    let periods = vec![
        quarter(
            "2024Q1",
            (2024, time::Month::January, 1),
            (2024, time::Month::April, 1),
        )?,
        quarter(
            "2024Q2",
            (2024, time::Month::April, 1),
            (2024, time::Month::July, 1),
        )?,
    ];
    let model = crate::FinancialModelSpec::new("example_model", periods);

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
    use crate::adjustments::types::{Adjustment, AdjustmentValue, NormalizationConfig};

    let mut amounts = indexmap::IndexMap::new();
    for (label, amount) in [("2024Q1", 1_500_000.0), ("2024Q2", 1_250_000.0)] {
        let period: finstack_quant_core::dates::PeriodId = label.parse().map_err(|error| {
            finstack_quant_core::Error::Internal(format!("parse example period {label}: {error}"))
        })?;
        amounts.insert(period, amount);
    }

    let config = NormalizationConfig {
        target_node: "ebitda".to_string(),
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
fn statement_result_examples() -> finstack_quant_core::Result<Vec<Value>> {
    let value =
        serde_json::to_value(crate::evaluator::StatementResult::default()).map_err(|error| {
            finstack_quant_core::Error::Internal(format!(
                "serialize statement result example: {error}"
            ))
        })?;
    Ok(vec![value])
}

/// A suite with one built-in articulation check and one formula check.
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

/// A seeded 1,000-path run reporting the 5th, 50th and 95th percentiles.
fn monte_carlo_config_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<crate::evaluator::MonteCarloConfig>(
        serde_json::json!({
            "n_paths": 1000,
            "seed": 42,
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
                    "values": {
                        "2025Q1": [[0.05, 940_000.0], [0.5, 1_000_000.0], [0.95, 1_065_000.0]],
                        "2025Q2": [[0.05, 925_000.0], [0.5, 1_020_000.0], [0.95, 1_120_000.0]]
                    }
                }
            },
            "n_paths": 1000,
            "percentiles": [0.05, 0.5, 0.95],
            "forecast_periods": ["2025Q1", "2025Q2"]
        }),
    )
}

/// One quarter of EBITDA with a single restructuring add-back.
fn normalization_result_examples() -> finstack_quant_core::Result<Vec<Value>> {
    finstack_quant_core::schema::example_from_json::<crate::adjustments::types::NormalizationResult>(
        serde_json::json!({
            "value_type": {"type": "monetary", "currency": "USD"},
            "period": "2024Q1",
            "base_value": 12_000_000.0,
            "adjustments": [{
                "adjustment_id": "restructuring",
                "name": "Restructuring charges",
                "raw_amount": 1_500_000.0,
                "capped_amount": 1_500_000.0,
                "is_capped": false
            }],
            "final_value": 13_500_000.0
        }),
    )
}

/// The crate's complete schema registry, sorted by artifact path.
///
/// This lives in the library, not the generator binary, so the generator, the
/// contract tests and the bindings all render from one definition. Render an
/// entry with [`finstack_quant_core::schema::SchemaArtifact::generate`].
pub const ARTIFACTS: &[finstack_quant_core::schema::SchemaArtifact] = &[
    finstack_quant_core::schema::SchemaArtifact::new::<crate::checks::CheckSuiteSpec>(
        "schemas/statements/1/check_suite_spec.schema.json",
        "https://finstack_quant.dev/schemas/statements/1/check_suite_spec.schema.json",
        "CheckSuiteSpec",
        "Serializable check suite: built-in checks, formula checks and filtering configuration.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
    .with_examples(check_suite_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::FinancialModelSpec>(
        "schemas/statements/1/financial_model_spec.schema.json",
        "https://finstack_quant.dev/schemas/statements/1/financial_model_spec.schema.json",
        "FinancialModelSpec",
        "Versioned financial statement model specification.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
    .with_summary(
        "Statement model graph: periods, nodes and the Value > Forecast > Formula precedence.",
    )
    .with_examples(financial_model_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<
        crate::adjustments::types::NormalizationConfig,
    >(
        "schemas/statements/1/normalization_config.schema.json",
        "https://finstack_quant.dev/schemas/statements/1/normalization_config.schema.json",
        "NormalizationConfig",
        "Financial statement normalization policy and adjustment catalog.",
    )
    .with_summary("Add-back and deduction catalog applied to a target metric such as EBITDA.")
    .with_examples(normalization_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::evaluator::StatementResult>(
        "schemas/statements/1/statement_result.schema.json",
        "https://finstack_quant.dev/schemas/statements/1/statement_result.schema.json",
        "StatementResult",
        "Versioned financial statement evaluation result.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output)
    .with_summary("Evaluated node values per period, with policy stamps.")
    .with_examples(statement_result_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::registry::MetricRegistry>(
        "schemas/statements/1/metric_registry.schema.json",
        "https://finstack_quant.dev/schemas/statements/1/metric_registry.schema.json",
        "MetricRegistry",
        "Namespaced catalog of reusable metric definitions.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
    .with_examples(metric_registry_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::evaluator::MonteCarloConfig>(
        "schemas/statements/1/monte_carlo_config.schema.json",
        "https://finstack_quant.dev/schemas/statements/1/monte_carlo_config.schema.json",
        "MonteCarloConfig",
        "Path count, seed and reported percentiles of a Monte Carlo statement-model evaluation.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Input)
    .with_examples(monte_carlo_config_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<crate::evaluator::MonteCarloResults>(
        "schemas/statements/1/monte_carlo_results.schema.json",
        "https://finstack_quant.dev/schemas/statements/1/monte_carlo_results.schema.json",
        "MonteCarloResults",
        "Percentile summaries of a Monte Carlo statement-model evaluation.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output)
    .with_examples(monte_carlo_results_examples),
    finstack_quant_core::schema::SchemaArtifact::new::<
        crate::adjustments::types::NormalizationResult,
    >(
        "schemas/statements/1/normalization_result.schema.json",
        "https://finstack_quant.dev/schemas/statements/1/normalization_result.schema.json",
        "NormalizationResult",
        "Per-period normalization outcome: base value, applied adjustments and final value.",
    )
    .with_kind(finstack_quant_core::schema::SchemaKind::Output)
    .with_examples(normalization_result_examples),
];
