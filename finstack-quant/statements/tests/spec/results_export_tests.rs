//! Integration tests for results export functionality.

use finstack_quant_statements::prelude::*;
use indexmap::indexmap;
use serde_json::json;

fn string_column<'a>(
    table: &'a finstack_quant_core::table::TableEnvelope,
    name: &str,
) -> &'a [String] {
    table
        .column(name)
        .unwrap()
        .as_strings()
        .expect("expected string column")
}

fn float_column<'a>(table: &'a finstack_quant_core::table::TableEnvelope, name: &str) -> &'a [f64] {
    table
        .column(name)
        .unwrap()
        .as_f64()
        .expect("expected float column")
}

#[test]
fn test_export_to_table_long() {
    let model = ModelBuilder::new("test")
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

    let table = results.to_table_long().unwrap();

    // Should have 3 nodes × 2 periods = 6 rows
    assert_eq!(table.row_count, 6);
    assert_eq!(table.columns.len(), 6); // node_id, period_id, value, value_money, currency, value_type
    assert_eq!(table.columns[0].name, "node_id");
    assert_eq!(table.columns[1].name, "period_id");
    assert_eq!(table.columns[2].name, "value");
    assert_eq!(table.columns[3].name, "value_money");
    assert_eq!(table.columns[4].name, "currency");
    assert_eq!(table.columns[5].name, "value_type");
}

#[test]
fn test_export_to_table_long_filtered() {
    let model = ModelBuilder::new("test")
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

    // Filter to just revenue and cogs
    let table = results
        .to_table_long_filtered(&["revenue", "cogs"])
        .unwrap();

    // Should have 2 nodes × 2 periods = 4 rows
    assert_eq!(table.row_count, 4);
    assert_eq!(table.columns.len(), 6);

    // Verify only revenue and cogs are present
    let node_ids = string_column(&table, "node_id");
    let unique_nodes: std::collections::HashSet<_> = node_ids.iter().map(String::as_str).collect();
    assert_eq!(unique_nodes.len(), 2);
    assert!(unique_nodes.contains("revenue"));
    assert!(unique_nodes.contains("cogs"));
    assert!(!unique_nodes.contains("gross_profit"));
}

#[test]
fn test_export_to_table_wide() {
    let model = ModelBuilder::new("test")
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

    let table = results.to_table_wide().unwrap();

    // Should have 2 periods (rows)
    assert_eq!(table.row_count, 2);
    // Should have 4 columns: period_id + 3 nodes
    assert_eq!(table.columns.len(), 4);

    // Check column names
    let col_names: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();
    assert!(col_names.contains(&"period_id".to_string()));
    assert!(col_names.contains(&"revenue".to_string()));
    assert!(col_names.contains(&"cogs".to_string()));
    assert!(col_names.contains(&"gross_profit".to_string()));

    // Check first row values
    let revenue = float_column(&table, "revenue");
    assert_eq!(revenue[0], 100_000.0);
    assert_eq!(revenue[1], 110_000.0);

    let cogs = float_column(&table, "cogs");
    assert_eq!(cogs[0], 60_000.0);
    assert_eq!(cogs[1], 66_000.0);
}

#[test]
fn test_export_complete_pl_model() {
    let model = ModelBuilder::new("Complete P&L")
        .periods("2025Q1..2025Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::scalar(100_000.0),
            )],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::GrowthPct,
                params: indexmap! { "rate".into() => json!(0.05) },
            },
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .value(
            "opex",
            &[(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(20_000.0))],
        )
        .forecast(
            "opex",
            ForecastSpec {
                method: ForecastMethod::ForwardFill,
                params: indexmap! {},
            },
        )
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .compute("operating_income", "gross_profit - opex")
        .unwrap()
        .compute("gross_margin", "gross_profit / revenue")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Test long format
    let table_long = results.to_table_long().unwrap();
    assert_eq!(table_long.row_count, 24); // 6 nodes × 4 periods

    // Test wide format
    let table_wide = results.to_table_wide().unwrap();
    assert_eq!(table_wide.row_count, 4); // 4 periods
    assert_eq!(table_wide.columns.len(), 7); // period_id + 6 nodes

    // Verify some calculated values
    let gross_margin = float_column(&table_wide, "gross_margin");
    // All quarters should have ~0.4 gross margin (40%)
    for margin in gross_margin.iter().take(4) {
        assert!((margin - 0.4).abs() < 0.001);
    }
}

#[test]
fn test_export_with_multiple_periods() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
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
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(120_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(130_000.0),
                ),
            ],
        )
        .compute("doubled", "revenue * 2")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    // Test long format
    let table_long = results.to_table_long().unwrap();
    assert_eq!(table_long.row_count, 8); // 2 nodes × 4 periods

    // Test wide format
    let table_wide = results.to_table_wide().unwrap();
    assert_eq!(table_wide.row_count, 4); // 4 periods
    assert_eq!(table_wide.columns.len(), 3); // period_id + 2 nodes

    // Verify period ordering in wide format
    let period_ids = string_column(&table_wide, "period_id");
    assert_eq!(period_ids[0], "2025Q1");
    assert_eq!(period_ids[1], "2025Q2");
    assert_eq!(period_ids[2], "2025Q3");
    assert_eq!(period_ids[3], "2025Q4");
}

#[test]
fn test_export_with_builtin_metrics() {
    let model = ModelBuilder::new("test")
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
        .value(
            "cogs",
            &[
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(60_000.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(66_000.0)),
            ],
        )
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .compute("gross_margin", "gross_profit / revenue")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let table_wide = results.to_table_wide().unwrap();

    // Should include revenue, cogs, and all calculated metrics
    assert!(table_wide.row_count > 0);
    let col_names: Vec<String> = table_wide.columns.iter().map(|c| c.name.clone()).collect();
    assert!(col_names.contains(&"revenue".to_string()));
    assert!(col_names.contains(&"cogs".to_string()));
    assert!(col_names.contains(&"gross_profit".to_string()));
    assert!(col_names.contains(&"gross_margin".to_string()));
}

#[test]
fn test_empty_results_export() {
    let results = StatementResult::default();

    let table_long = results.to_table_long().unwrap();
    assert_eq!(table_long.row_count, 0);
    assert_eq!(table_long.columns.len(), 6); // Updated for new columns

    let table_wide = results.to_table_wide().unwrap();
    assert_eq!(table_wide.row_count, 0);
    assert_eq!(table_wide.columns.len(), 1); // Just period_id column
}

#[test]
fn test_export_preserves_period_order() {
    let model = ModelBuilder::new("test")
        .periods("2025Q1..2025Q4", None)
        .unwrap()
        .value(
            "value",
            &[
                (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(4.0)),
                (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(2.0)),
                (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(1.0)),
                (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(3.0)),
            ],
        )
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let results = evaluator.evaluate(&model).unwrap();

    let table_wide = results.to_table_wide().unwrap();

    // Periods should be sorted in chronological order
    let period_ids = string_column(&table_wide, "period_id");
    assert_eq!(period_ids[0], "2025Q1");
    assert_eq!(period_ids[1], "2025Q2");
    assert_eq!(period_ids[2], "2025Q3");
    assert_eq!(period_ids[3], "2025Q4");

    // Values should match the sorted order
    let values = float_column(&table_wide, "value");
    assert_eq!(values[0], 1.0);
    assert_eq!(values[1], 2.0);
    assert_eq!(values[2], 3.0);
    assert_eq!(values[3], 4.0);
}
