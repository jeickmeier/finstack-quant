//! Comprehensive serialization tests for all statements types.

use finstack_quant_core::contract::{ContractError, LoadLimits};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
use finstack_quant_statements::evaluator::{EvalStats, Evaluator, StatementResult};
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, NodeSpec, NodeType};
use indexmap::IndexMap;

#[test]
fn test_results_serialization() {
    // Create a StatementResult object
    let mut results = StatementResult::new();
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

    results.nodes.insert(
        "revenue".to_string(),
        [(period, 100_000.0)].into_iter().collect(),
    );
    results.nodes.insert(
        "cogs".to_string(),
        [(period, 60_000.0)].into_iter().collect(),
    );

    results.meta = EvalStats {
        eval_time_ms: Some(42),
        num_nodes: 2,
        num_periods: 1,
        numeric_mode: finstack_quant_statements::evaluator::NumericMode::Float64,
        parallel: false,
        warnings: Vec::new(),
    };

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&results).expect("Failed to serialize Results");
    println!("Serialized Results:\n{}", json);

    // Deserialize back
    let deserialized: StatementResult =
        serde_json::from_str(&json).expect("Failed to deserialize StatementResult");

    // Verify round-trip
    assert_eq!(deserialized.get("revenue", &period), Some(100_000.0));
    assert_eq!(deserialized.get("cogs", &period), Some(60_000.0));
    assert_eq!(deserialized.meta.num_nodes, 2);
    assert_eq!(deserialized.meta.num_periods, 1);
    assert_eq!(deserialized.meta.eval_time_ms, Some(42));
}

#[test]
fn test_capital_structure_cashflows_serialization() {
    let mut cashflows = CapitalStructureCashflows::new();
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

    // Add breakdown for an instrument
    let breakdown = CashflowBreakdown {
        interest_expense_cash: finstack_quant_core::money::Money::new(
            5_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        interest_income_cash: None,
        interest_expense_pik: finstack_quant_core::money::Money::new(
            0.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        principal_payment: finstack_quant_core::money::Money::new(
            10_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        debt_balance: finstack_quant_core::money::Money::new(
            100_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        fees: finstack_quant_core::money::Money::new(
            500.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        accrued_interest: finstack_quant_core::money::Money::new(
            0.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
    };

    let mut period_map = IndexMap::new();
    period_map.insert(period, breakdown.clone());

    cashflows
        .by_instrument
        .insert("BOND-001".to_string(), period_map);
    cashflows.totals.insert(period, breakdown);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&cashflows)
        .expect("Failed to serialize CapitalStructureCashflows");
    println!("Serialized CapitalStructureCashflows:\n{}", json);

    // Deserialize back
    let deserialized: CapitalStructureCashflows =
        serde_json::from_str(&json).expect("Failed to deserialize CapitalStructureCashflows");

    // Verify round-trip
    assert_eq!(
        deserialized
            .get_interest("BOND-001", &period)
            .expect("interest"),
        5_000.0
    );
    assert_eq!(
        deserialized
            .get_principal("BOND-001", &period)
            .expect("principal"),
        10_000.0
    );
    assert_eq!(
        deserialized
            .get_debt_balance("BOND-001", &period)
            .expect("balance"),
        100_000.0
    );
    assert_eq!(
        deserialized
            .get_total_interest(&period)
            .expect("total interest"),
        5_000.0
    );
}

#[test]
fn test_model_spec_full_serialization() {
    // Create a complete model
    let model = ModelBuilder::new("test_model")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .build()
        .unwrap();

    // Serialize to JSON
    let json =
        serde_json::to_string_pretty(&model).expect("Failed to serialize FinancialModelSpec");
    println!("Serialized FinancialModelSpec:\n{}", json);

    // Deserialize back
    let deserialized: FinancialModelSpec =
        serde_json::from_str(&json).expect("Failed to deserialize FinancialModelSpec");

    // Verify structure
    assert_eq!(deserialized.id, "test_model");
    assert_eq!(deserialized.periods.len(), 2);
    assert_eq!(deserialized.nodes.len(), 3);
    assert!(deserialized.has_node("revenue"));
    assert!(deserialized.has_node("cogs"));
    assert!(deserialized.has_node("gross_profit"));

    // Verify that the nodes map keys serialize as plain JSON strings (not objects)
    let json = serde_json::to_string_pretty(&deserialized).expect("re-serialize");
    assert!(json.contains(r#""revenue""#));
    assert!(json.contains(r#""cogs""#));
    assert!(json.contains(r#""gross_profit""#));
    // Ensure no nested {"0": ...} structure (i.e., keys are strings, not structs)
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse JSON");
    let nodes_obj = &parsed["nodes"];
    assert!(nodes_obj.is_object(), "nodes should be a JSON object");
    assert!(
        nodes_obj["revenue"].is_object(),
        "revenue node should be an object"
    );
}

#[test]
fn financial_model_missing_version_is_rejected_by_all_loaders() {
    let model = ModelBuilder::new("missing-version")
        .periods("2025Q1..Q1", None)
        .expect("periods")
        .build()
        .expect("model");
    let mut json = serde_json::to_value(model).expect("model serializes");
    json.as_object_mut()
        .expect("model is object")
        .remove("schema_version");

    serde_json::from_value::<FinancialModelSpec>(json.clone())
        .expect_err("ordinary serde requires schema_version");

    let bytes = serde_json::to_vec(&json).expect("fixture serializes");
    let error = FinancialModelSpec::from_slice_strict(&bytes, &LoadLimits::default())
        .expect_err("strict loading requires schema_version");
    let ContractError::Report(report) = error else {
        panic!("expected structured report");
    };
    assert_eq!(report.diagnostics[0].code, "contract/version-missing");
}

#[test]
fn financial_model_accepts_explicit_null_optional_capital_structure() {
    let model = ModelBuilder::new("null-capital-structure")
        .periods("2025Q1..Q1", None)
        .expect("periods")
        .build()
        .expect("model");
    let mut json = serde_json::to_value(model).expect("model serializes");
    json["schema_version"] = serde_json::json!(1);
    json["capital_structure"] = serde_json::Value::Null;

    let bytes = serde_json::to_vec(&json).expect("fixture serializes");
    let (loaded, report) = FinancialModelSpec::from_slice_strict(&bytes, &LoadLimits::default())
        .expect("null optional capital structure is absent");

    assert_eq!(u32::from(loaded.schema_version), 1);
    assert!(loaded.capital_structure.is_none());
    assert!(report.diagnostics.is_empty());
}

#[test]
fn financial_model_strict_loader_rejects_zero_future_malformed_and_invalid_semantics() {
    let model = ModelBuilder::new("strict")
        .periods("2025Q1..Q1", None)
        .expect("periods")
        .build()
        .expect("model");
    let base = serde_json::to_value(model).expect("model serializes");

    for version in [0, 3] {
        let mut json = base.clone();
        json.as_object_mut()
            .expect("model is object")
            .insert("schema_version".into(), serde_json::json!(version));
        let bytes = serde_json::to_vec(&json).expect("fixture serializes");
        let error = FinancialModelSpec::from_slice_strict(&bytes, &LoadLimits::default())
            .expect_err("unsupported version must fail");
        let ContractError::Report(report) = error else {
            panic!("expected structured report");
        };
        assert_eq!(report.diagnostics[0].code, "contract/version-unsupported");
    }

    let malformed = FinancialModelSpec::from_slice_strict(b"{", &LoadLimits::default())
        .expect_err("malformed JSON must fail");
    let ContractError::Report(report) = malformed else {
        panic!("expected structured report");
    };
    assert_eq!(report.diagnostics[0].code, "contract/parse-error");

    let mut empty_periods = base;
    empty_periods
        .as_object_mut()
        .expect("model is object")
        .insert("periods".into(), serde_json::json!([]));
    assert!(
        FinancialModelSpec::from_slice_strict(
            &serde_json::to_vec(&empty_periods).expect("fixture"),
            &LoadLimits::default(),
        )
        .is_err(),
        "strict loading must run semantic validation"
    );
}

#[test]
fn financial_model_requires_exact_v1_marker() {
    let model = ModelBuilder::new("strict-v1")
        .periods("2025Q1..Q2", None)
        .expect("valid periods")
        .build()
        .expect("valid model");
    let valid = serde_json::to_value(model).expect("serialize model");
    FinancialModelSpec::from_slice_strict(
        &serde_json::to_vec(&valid).expect("serialize fixture"),
        &LoadLimits::default(),
    )
    .expect("schema_version 1 must load");

    let mut invalid_documents = Vec::new();
    let mut missing = valid.clone();
    missing
        .as_object_mut()
        .expect("model object")
        .remove("schema_version");
    invalid_documents.push(missing);
    for invalid in [
        serde_json::json!(0),
        serde_json::json!(2),
        serde_json::Value::Null,
    ] {
        let mut document = valid.clone();
        document["schema_version"] = invalid;
        invalid_documents.push(document);
    }

    for document in invalid_documents {
        assert!(
            serde_json::from_value::<FinancialModelSpec>(document.clone()).is_err(),
            "ordinary serde must reject a missing or non-v1 marker"
        );
        assert!(
            FinancialModelSpec::from_slice_strict(
                &serde_json::to_vec(&document).expect("serialize invalid fixture"),
                &LoadLimits::default(),
            )
            .is_err(),
            "strict loading must reject a missing or non-v1 marker"
        );
    }
}

#[test]
fn test_model_with_results_serialization() {
    // Create and evaluate a model
    let model = ModelBuilder::new("profit_model")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1).expect("valid period fixture"),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2).expect("valid period fixture"),
                    AmountOrScalar::scalar(110_000.0),
                ),
            ],
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Serialize both model and results
    let model_json = serde_json::to_string_pretty(&model).expect("Failed to serialize model");
    let results_json = serde_json::to_string_pretty(&results).expect("Failed to serialize results");

    println!("Serialized Model:\n{}", model_json);
    println!("\nSerialized Results:\n{}", results_json);

    // Deserialize both
    let deserialized_model: FinancialModelSpec =
        serde_json::from_str(&model_json).expect("Failed to deserialize model");
    let deserialized_results: StatementResult =
        serde_json::from_str(&results_json).expect("Failed to deserialize results");

    // Verify model structure
    assert_eq!(deserialized_model.id, "profit_model");
    assert_eq!(deserialized_model.nodes.len(), 3);

    // Verify results
    let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
    let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

    assert_eq!(deserialized_results.get("revenue", &q1), Some(100_000.0));
    assert_eq!(deserialized_results.get("revenue", &q2), Some(110_000.0));
    assert_eq!(deserialized_results.get("cogs", &q1), Some(60_000.0));
    assert_eq!(deserialized_results.get("cogs", &q2), Some(66_000.0));
    assert_eq!(
        deserialized_results.get("gross_profit", &q1),
        Some(40_000.0)
    );
    assert_eq!(
        deserialized_results.get("gross_profit", &q2),
        Some(44_000.0)
    );
}

#[test]
fn test_node_spec_with_currency_serialization() {
    // Create a node with currency-aware values
    let mut node = NodeSpec::new("cash", NodeType::Value);
    let mut values = IndexMap::new();
    values.insert(
        PeriodId::quarter(2025, 1).expect("valid period fixture"),
        AmountOrScalar::amount(50_000.0, Currency::USD).expect("valid amount fixture"),
    );
    values.insert(
        PeriodId::quarter(2025, 2).expect("valid period fixture"),
        AmountOrScalar::amount(55_000.0, Currency::USD).expect("valid amount fixture"),
    );
    node.values = Some(values);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&node).expect("Failed to serialize NodeSpec");
    println!("Serialized NodeSpec with currency:\n{}", json);

    // Deserialize back
    let deserialized: NodeSpec =
        serde_json::from_str(&json).expect("Failed to deserialize NodeSpec");

    // Verify round-trip
    assert_eq!(deserialized.node_id, "cash");
    assert!(matches!(deserialized.node_type, NodeType::Value));

    let values = deserialized.values.as_ref().unwrap();
    let q1_value = values
        .get(&PeriodId::quarter(2025, 1).expect("valid period fixture"))
        .unwrap();
    assert_eq!(q1_value.value(), 50_000.0);
    assert_eq!(q1_value.currency(), Some(Currency::USD));

    // Verify NodeId serializes as a plain string in the JSON
    let json = serde_json::to_string_pretty(&deserialized).expect("re-serialize");
    assert!(json.contains(r#""node_id": "cash""#));
}

#[test]
fn test_empty_results_serialization() {
    // Test edge case: empty results
    let results = StatementResult::new();

    let json = serde_json::to_string(&results).expect("Failed to serialize empty StatementResult");
    let deserialized: StatementResult =
        serde_json::from_str(&json).expect("Failed to deserialize empty StatementResult");

    assert_eq!(deserialized.nodes.len(), 0);
    assert_eq!(deserialized.meta.num_nodes, 0);
    assert_eq!(deserialized.meta.num_periods, 0);
    assert_eq!(deserialized.meta.eval_time_ms, None);
}

#[test]
fn test_results_to_json_file() {
    // Test that Results can be saved to and loaded from a JSON file
    let mut results = StatementResult::new();
    let period = PeriodId::quarter(2025, 1).expect("valid period fixture");

    results.nodes.insert(
        "revenue".to_string(),
        [(period, 100_000.0)].into_iter().collect(),
    );

    // Serialize to JSON string
    let json = serde_json::to_string_pretty(&results).expect("Failed to serialize");

    // Verify it's valid JSON and reasonably sized
    assert!(!json.is_empty());
    assert!(json.contains("revenue"));
    assert!(json.contains("2025Q1"));

    // Deserialize back
    let deserialized: StatementResult = serde_json::from_str(&json).expect("Failed to deserialize");

    // Verify round-trip
    assert_eq!(deserialized.get("revenue", &period), Some(100_000.0));
}

#[test]
fn test_capital_structure_json_roundtrip() {
    // Test CashflowBreakdown JSON serialization
    let breakdown = CashflowBreakdown {
        interest_expense_cash: finstack_quant_core::money::Money::new(
            5_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        interest_income_cash: None,
        interest_expense_pik: finstack_quant_core::money::Money::new(
            0.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        principal_payment: finstack_quant_core::money::Money::new(
            10_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        debt_balance: finstack_quant_core::money::Money::new(
            100_000.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        fees: finstack_quant_core::money::Money::new(
            500.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
        accrued_interest: finstack_quant_core::money::Money::new(
            0.0,
            finstack_quant_core::currency::Currency::USD,
        )
        .expect("valid money fixture"),
    };

    let json = serde_json::to_string(&breakdown).expect("Failed to serialize");
    let deserialized: CashflowBreakdown =
        serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(deserialized.interest_expense_cash.amount(), 5_000.0);
    assert_eq!(deserialized.interest_expense_pik.amount(), 0.0);
    assert_eq!(deserialized.principal_payment.amount(), 10_000.0);
    assert_eq!(deserialized.debt_balance.amount(), 100_000.0);
    assert_eq!(deserialized.fees.amount(), 500.0);
}
