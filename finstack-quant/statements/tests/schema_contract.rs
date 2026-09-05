//! JSON Schema contract tests for statement model inputs and results.

use std::fs;
use std::path::{Path, PathBuf};

use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Month, PeriodId};
use finstack_quant_core::money::Money;
use finstack_quant_core::ContractDescriptor;
use finstack_quant_statements::adjustments::types::CapBaseMode;
use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
use finstack_quant_statements::checks::{
    CheckCategory, CheckFinding, CheckReport, CheckResult, CheckSummary, Materiality, Severity,
};
use finstack_quant_statements::evaluator::{
    CapitalStructureWarning, EvalStats, EvalWarning, StatementResult,
};
use finstack_quant_statements::schema::{
    financial_model_spec_schema, normalization_config_schema, statement_result_schema,
    STATEMENTS_SCHEMA_BASE,
};
use finstack_quant_statements::types::NodeValueType;
use finstack_quant_statements::FinancialModelSpec;
use indexmap::IndexMap;
use serde_json::{json, Value};

const CANONICAL_DECIMAL_PATTERN: &str = r"^-?\d+(\.\d+)?([eE][+-]?\d+)?$";

fn validate_fixture(schema: &Value, fixture: &Value) {
    let validator = jsonschema::validator_for(schema).expect("checked-in schema must compile");
    let errors: Vec<_> = validator
        .iter_errors(fixture)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "fixture failed schema validation: {errors:#?}"
    );
}

fn collect_schema_files(directory: &Path, paths: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read schema directory {}: {error}", directory.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|error| {
                    panic!("read schema entry in {}: {error}", directory.display())
                })
                .path()
        })
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_schema_files(&path, paths);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            paths.push(path);
        }
    }
}

fn external_schema_resources() -> Vec<(String, jsonschema::Resource)> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let roots = [
        manifest_dir.join("../cashflows/schemas/cashflow/1"),
        manifest_dir.join("../valuations/schemas/common/1"),
        manifest_dir.join("../valuations/schemas/instruments/1"),
    ];
    let mut paths = Vec::new();
    for root in roots {
        collect_schema_files(&root, &mut paths);
    }
    paths
        .into_iter()
        .map(|path| {
            let raw = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read schema {}: {error}", path.display()));
            let schema: Value = serde_json::from_str(&raw)
                .unwrap_or_else(|error| panic!("parse schema {}: {error}", path.display()));
            let id = schema
                .get("$id")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("schema {} is missing $id", path.display()))
                .to_string();
            let resource = jsonschema::Resource::from_contents(schema)
                .unwrap_or_else(|error| panic!("build resource {}: {error}", path.display()));
            (id, resource)
        })
        .collect()
}

fn validate_model_fixture(fixture: &Value) -> Result<(), Vec<String>> {
    let validator = jsonschema::options()
        .with_resources(external_schema_resources().into_iter())
        .build(financial_model_spec_schema().expect("financial model schema parses"))
        .expect("financial model schema and external references compile");
    let errors: Vec<_> = validator
        .iter_errors(fixture)
        .map(|error| error.to_string())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn model_fixture_with_bond() -> Value {
    let mut model: Value =
        serde_json::from_str(include_str!("data/canonical/financial_model.json"))
            .expect("canonical financial model fixture parses");
    let bond_envelope: Value = serde_json::from_str(include_str!(
        "../../valuations/tests/instruments/json_examples/bond.json"
    ))
    .expect("canonical bond fixture parses");
    model["capital_structure"] = json!({
        "debt_instruments": [{
            "id": "US912828XG33",
            "spec": bond_envelope["instrument"].clone()
        }]
    });
    model
}

fn normalization_fixture() -> Value {
    json!({
        "target_node": "ebitda",
        "adjustments": [
            {
                "id": "restructuring",
                "name": "Restructuring add-back",
                "value": {
                    "type": "fixed",
                    "amounts": {
                        "2025Q1": 50.0,
                        "2025Q2": 45.0,
                        "2025Q3": 40.0
                    }
                },
                "cap": {
                    "base_node": "ebitda",
                    "value": 0.10
                }
            },
            {
                "id": "management_fee",
                "name": "Management fee",
                "value": {
                    "type": "percentage_of_node",
                    "node_id": "revenue",
                    "percentage": 0.01
                }
            }
        ]
    })
}

fn representative_statement_result() -> StatementResult {
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let usd = Money::new(100_000.0, Currency::USD).expect("valid money fixture");
    let mut result = StatementResult::default();
    result
        .nodes
        .insert("revenue".to_string(), IndexMap::from([(period, 100_000.0)]));
    result
        .monetary_nodes
        .insert("revenue".to_string(), IndexMap::from([(period, usd)]));
    result.node_value_types.insert(
        "revenue".to_string(),
        NodeValueType::Monetary {
            currency: Currency::USD,
        },
    );

    let breakdown = CashflowBreakdown {
        interest_expense_cash: Money::new(5_000.0, Currency::USD).expect("valid money fixture"),
        interest_income_cash: Some(Money::new(250.0, Currency::USD).expect("valid money fixture")),
        interest_expense_pik: Money::new(500.0, Currency::USD).expect("valid money fixture"),
        principal_payment: Money::new(10_000.0, Currency::USD).expect("valid money fixture"),
        fees: Money::new(100.0, Currency::USD).expect("valid money fixture"),
        debt_balance: Money::new(990_000.0, Currency::USD).expect("valid money fixture"),
        accrued_interest: Money::new(1_000.0, Currency::USD).expect("valid money fixture"),
    };
    let mut cashflows = CapitalStructureCashflows::new();
    cashflows.by_instrument.insert(
        "US912828XG33".to_string(),
        IndexMap::from([(period, breakdown.clone())]),
    );
    cashflows.totals.insert(period, breakdown);
    cashflows.reporting_currency = Some(Currency::USD);
    cashflows.equity_distribution.insert(
        period,
        Money::new(25_000.0, Currency::USD).expect("valid money fixture"),
    );
    result.cs_cashflows = Some(cashflows);

    result.check_report = Some(CheckReport {
        results: vec![CheckResult {
            check_id: "balance-sheet".to_string(),
            check_name: "Balance sheet balances".to_string(),
            category: CheckCategory::AccountingIdentity,
            passed: false,
            findings: vec![CheckFinding {
                check_id: "balance-sheet".to_string(),
                severity: Severity::Warning,
                message: "Assets differ from liabilities and equity".to_string(),
                period: Some(period),
                materiality: Some(Materiality {
                    absolute: 100.0,
                    relative_pct: 0.01,
                    reference_value: 1_000_000.0,
                    reference_label: "total_assets".to_string(),
                }),
                nodes: vec!["total_assets".into(), "total_liabilities".into()],
            }],
        }],
        summary: CheckSummary {
            total_checks: 1,
            passed: 0,
            failed: 1,
            errors: 0,
            warnings: 1,
            infos: 0,
        },
    });
    result.meta = EvalStats {
        eval_time_ms: Some(3),
        num_nodes: 1,
        num_periods: 1,
        parallel: false,
        warnings: vec![EvalWarning::DivisionByZero {
            node_id: "interest_coverage".to_string(),
            period,
        }],
        ..EvalStats::default()
    };
    result
}

/// Render a checked-in artifact through the registry.
///
/// Only the registry path applies the packager, the single-branch-union
/// collapse and examples, so `generated_schema` would drift from the bytes on
/// disk.
fn expected_schema(filename: &str) -> Value {
    finstack_quant_statements::schema::ARTIFACTS
        .iter()
        .find(|artifact| artifact.relative_path.ends_with(filename))
        .unwrap_or_else(|| panic!("no registry entry for {filename}"))
        .generate()
        .expect("registry artifact renders")
}

#[test]
fn root_types_generate_typed_schemas() {
    let model = serde_json::to_value(schemars::schema_for!(FinancialModelSpec))
        .expect("financial model schema serializes");
    let result = serde_json::to_value(schemars::schema_for!(StatementResult))
        .expect("statement result schema serializes");

    assert_eq!(model["title"], "FinancialModelSpec");
    assert_eq!(result["title"], "StatementResult");
    assert!(model["properties"]["periods"]["items"].is_object());
    assert!(result["properties"]["meta"].is_object());
    assert_eq!(
        model["$defs"]["DebtInstrumentSpec"]["properties"]["spec"]["$ref"],
        "#/$defs/FinancialStatementInstrument"
    );
    let supported_types = model["$defs"]["FinancialStatementInstrument"]["oneOf"]
        .as_array()
        .expect("financial statement instrument schema should be a oneOf")
        .iter()
        .map(|variant| {
            variant["properties"]["type"]["const"]
                .as_str()
                .expect("instrument variant should have a type discriminator")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        supported_types,
        vec![
            "bond",
            "convertible_bond",
            "revolving_credit",
            "term_loan",
            "interest_rate_swap",
            "cap_floor",
            "swaption",
        ]
    );
    assert!(model["$defs"].get("InstrumentJson").is_none());
    assert!(model["$defs"].get("RangeAccrual").is_none());
    assert_eq!(model["$defs"]["SchemaVersion"]["minimum"], 1);
    assert_eq!(
        model["$defs"]["SchemaVersion"]["maximum"],
        ContractDescriptor::VERSION
    );
}

#[test]
fn capital_structure_warning_uses_typed_canonical_values() {
    let warning = EvalWarning::CapitalStructure {
        period: PeriodId::quarter(2025, 1).expect("valid period fixture"),
        warning: CapitalStructureWarning::CashflowIgnored {
            cashflow_kind: CFKind::Recovery,
            cashflow_date: Date::from_calendar_date(2025, Month::March, 31)
                .expect("valid fixture date"),
        },
    };

    let json = serde_json::to_value(warning).expect("warning should serialize");
    assert_eq!(
        json["capital_structure"]["warning"]["kind"],
        "cashflow_ignored"
    );
    assert_eq!(
        json["capital_structure"]["warning"]["cashflow_kind"],
        "recovery"
    );
    assert_eq!(
        json["capital_structure"]["warning"]["cashflow_date"],
        "2025-03-31"
    );
}

#[test]
fn nan_warning_uses_canonical_acronym_spelling() {
    let warning = EvalWarning::NaNPropagated {
        node_id: "ratio".to_string(),
        period: PeriodId::quarter(2025, 1).expect("valid period fixture"),
    };
    let value = serde_json::to_value(warning).expect("serialize NaN warning");
    assert!(value.get("nan_propagated").is_some());

    // schema-rejection-test
    assert!(serde_json::from_value::<EvalWarning>(json!({
        "na_n_propagated": {
            "node_id": "ratio",
            "period": "2025Q1"
        }
    }))
    .is_err());
}

#[test]
fn checked_in_schemas_have_stable_metadata() {
    let model = financial_model_spec_schema().expect("financial model schema parses");
    let result = statement_result_schema().expect("statement result schema parses");

    assert_eq!(
        model["$id"],
        format!("{STATEMENTS_SCHEMA_BASE}financial_model_spec.schema.json")
    );
    assert_eq!(
        result["$id"],
        format!("{STATEMENTS_SCHEMA_BASE}statement_result.schema.json")
    );
    assert_eq!(
        model["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        result["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );

    let normalization = normalization_config_schema().expect("normalization config schema parses");
    assert_eq!(
        normalization["$id"],
        format!("{STATEMENTS_SCHEMA_BASE}normalization_config.schema.json")
    );
    assert_eq!(
        normalization["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
}

#[test]
fn checked_in_schemas_apply_canonical_decimal_and_date_normalization() {
    let model = financial_model_spec_schema().expect("financial model schema parses");

    assert_eq!(
        model.pointer("/$defs/DecimalWire/pattern"),
        Some(&json!(CANONICAL_DECIMAL_PATTERN))
    );
    assert_eq!(
        model.pointer("/$defs/DateWire/format"),
        Some(&json!("date"))
    );
}

#[test]
fn normalization_config_schema_exposes_base_mode_reported_default() {
    let schema = normalization_config_schema().expect("normalization schema parses");
    let default_mode =
        serde_json::to_value(CapBaseMode::default()).expect("CapBaseMode default serializes");
    // Pin the wire value so a future default-variant change is caught here,
    // not just in the schema comparison below.
    assert_eq!(default_mode, json!("reported"));
    assert_eq!(
        schema["$defs"]["AdjustmentCap"]["properties"]["base_mode"]["default"],
        default_mode
    );
}

#[test]
fn normalization_config_schema_matches_generated_type() {
    let expected = expected_schema("normalization_config.schema.json");
    assert_eq!(
        normalization_config_schema().expect("normalization schema parses"),
        &expected
    );
}

#[test]
fn normalization_config_schema_validates_supported_variants() {
    validate_fixture(
        normalization_config_schema().expect("normalization schema parses"),
        &normalization_fixture(),
    );
}

#[test]
fn normalization_config_schema_rejects_unknown_adjustment_variant() {
    let mut fixture = normalization_fixture();
    fixture["adjustments"][0]["value"]["type"] = json!("run_rate");
    let validator = jsonschema::validator_for(
        normalization_config_schema().expect("normalization schema parses"),
    )
    .expect("checked-in normalization schema must compile");
    assert!(
        validator.iter_errors(&fixture).next().is_some(),
        "an unsupported AdjustmentValue tag must be rejected"
    );
}

#[test]
fn checked_in_schemas_match_generated_types() {
    let expected_model = expected_schema("financial_model_spec.schema.json");
    let expected_result = expected_schema("statement_result.schema.json");

    assert_eq!(
        financial_model_spec_schema().expect("financial model schema parses"),
        &expected_model
    );
    assert_eq!(
        statement_result_schema().expect("statement result schema parses"),
        &expected_result
    );
}

#[test]
fn checked_in_schemas_validate_canonical_fixtures() {
    let model_fixture = model_fixture_with_bond();
    let result_fixture = serde_json::to_value(representative_statement_result())
        .expect("representative result serializes");

    validate_model_fixture(&model_fixture)
        .unwrap_or_else(|errors| panic!("financial model fixture failed validation: {errors:#?}"));
    validate_fixture(
        statement_result_schema().expect("statement result schema parses"),
        &result_fixture,
    );
    assert_eq!(result_fixture["schema_version"], 1);
}

#[test]
fn financial_model_schema_rejects_invalid_versions_and_debt_specs() {
    let mut fixture = model_fixture_with_bond();
    fixture["schema_version"] = json!(0);
    assert!(
        validate_model_fixture(&fixture).is_err(),
        "schema_version 0 must be rejected"
    );

    fixture["schema_version"] = json!(ContractDescriptor::VERSION + 1);
    assert!(
        validate_model_fixture(&fixture).is_err(),
        "schema versions above the current version must be rejected"
    );

    fixture["schema_version"] = json!(ContractDescriptor::VERSION);
    fixture["capital_structure"]["debt_instruments"][0]["spec"] =
        json!({"type": "bond", "spec": {}});
    assert!(
        validate_model_fixture(&fixture).is_err(),
        "typed debt instruments must satisfy the canonical instrument schema"
    );

    fixture["capital_structure"]["debt_instruments"][0]["spec"] = json!({
        "type": "range_accrual",
        "spec": {}
    });
    assert!(
        validate_model_fixture(&fixture).is_err(),
        "financial statement debt instruments must reject unsupported instrument types"
    );
}
