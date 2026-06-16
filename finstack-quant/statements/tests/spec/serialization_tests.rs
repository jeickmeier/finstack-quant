//! Comprehensive serialization tests for all statements types.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
use finstack_quant_statements::evaluator::{Evaluator, ResultsMeta, StatementResult};
use finstack_quant_statements::types::{AmountOrScalar, FinancialModelSpec, NodeSpec, NodeType};
use indexmap::IndexMap;

#[test]
fn test_results_serialization() {
    // Create a StatementResult object
    let mut results = StatementResult::new();
    let period = PeriodId::quarter(2025, 1);

    results.nodes.insert(
        "revenue".to_string(),
        [(period, 100_000.0)].into_iter().collect(),
    );
    results.nodes.insert(
        "cogs".to_string(),
        [(period, 60_000.0)].into_iter().collect(),
    );

    results.meta = ResultsMeta {
        eval_time_ms: Some(42),
        num_nodes: 2,
        num_periods: 1,
        numeric_mode: finstack_quant_statements::evaluator::NumericMode::Float64,
        rounding_context: None,
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
    let period = PeriodId::quarter(2025, 1);

    // Add breakdown for an instrument
    let breakdown = CashflowBreakdown {
        interest_expense_cash: finstack_quant_core::money::Money::new(
            5_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        interest_expense_pik: finstack_quant_core::money::Money::new(
            0.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        principal_payment: finstack_quant_core::money::Money::new(
            10_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        debt_balance: finstack_quant_core::money::Money::new(
            100_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        fees: finstack_quant_core::money::Money::new(
            500.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        accrued_interest: finstack_quant_core::money::Money::new(
            0.0,
            finstack_quant_core::currency::Currency::USD,
        ),
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
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
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
fn test_model_with_results_serialization() {
    // Create and evaluate a model
    let model = ModelBuilder::new("profit_model")
        .periods("2025Q1..Q2", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
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
    let q1 = PeriodId::quarter(2025, 1);
    let q2 = PeriodId::quarter(2025, 2);

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
        PeriodId::quarter(2025, 1),
        AmountOrScalar::amount(50_000.0, Currency::USD),
    );
    values.insert(
        PeriodId::quarter(2025, 2),
        AmountOrScalar::amount(55_000.0, Currency::USD),
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
    let q1_value = values.get(&PeriodId::quarter(2025, 1)).unwrap();
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
    let period = PeriodId::quarter(2025, 1);

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
        ),
        interest_expense_pik: finstack_quant_core::money::Money::new(
            0.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        principal_payment: finstack_quant_core::money::Money::new(
            10_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        debt_balance: finstack_quant_core::money::Money::new(
            100_000.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        fees: finstack_quant_core::money::Money::new(
            500.0,
            finstack_quant_core::currency::Currency::USD,
        ),
        accrued_interest: finstack_quant_core::money::Money::new(
            0.0,
            finstack_quant_core::currency::Currency::USD,
        ),
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
