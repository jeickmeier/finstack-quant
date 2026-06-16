//! Table export functionality for [`StatementResult`].

use super::StatementResult;
use crate::error::Result;
use crate::types::NodeValueType;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::table::{TableColumn, TableColumnData, TableColumnRole, TableEnvelope};
use indexmap::IndexMap;
use serde_json::json;

fn table_metadata(layout: &str) -> IndexMap<String, serde_json::Value> {
    let mut metadata = IndexMap::new();
    metadata.insert("layout".to_string(), json!(layout));
    metadata.insert("source".to_string(), json!("statement_result"));
    metadata
}

fn build_long_table(results: &StatementResult, node_filter: &[&str]) -> Result<TableEnvelope> {
    let mut node_ids = Vec::new();
    let mut period_ids = Vec::new();
    let mut values = Vec::new();
    let mut money_values = Vec::new();
    let mut currencies = Vec::new();
    let mut value_types = Vec::new();

    for (node_id, period_map) in &results.nodes {
        if !node_filter.is_empty() && !node_filter.contains(&node_id.as_str()) {
            continue;
        }

        let node_value_type = results.node_value_types.get(node_id);

        for (period_id, value) in period_map {
            node_ids.push(node_id.clone());
            period_ids.push(period_id.to_string());
            values.push(*value);

            if let Some(NodeValueType::Monetary { currency }) = node_value_type {
                money_values.push(Some(*value));
                currencies.push(Some(currency.to_string()));
                value_types.push("monetary".to_string());
            } else {
                money_values.push(None);
                currencies.push(None);
                value_types.push("scalar".to_string());
            }
        }
    }

    TableEnvelope::new_with_metadata(
        vec![
            TableColumn::new("node_id", TableColumnData::String(node_ids))
                .with_role(TableColumnRole::Dimension),
            TableColumn::new("period_id", TableColumnData::String(period_ids))
                .with_role(TableColumnRole::Index),
            TableColumn::new("value", TableColumnData::Float64(values))
                .with_role(TableColumnRole::Measure),
            TableColumn::new(
                "value_money",
                TableColumnData::NullableFloat64(money_values),
            )
            .with_role(TableColumnRole::Measure),
            TableColumn::new("currency", TableColumnData::NullableString(currencies))
                .with_role(TableColumnRole::Attribute),
            TableColumn::new("value_type", TableColumnData::String(value_types))
                .with_role(TableColumnRole::Attribute),
        ],
        table_metadata("long"),
    )
    .map_err(Into::into)
}

/// Export results to a long-format table.
///
/// Schema: `(node_id, period_id, value, value_money, currency, value_type)`.
/// Monetary nodes populate both `value` and `value_money`; scalar nodes leave
/// `value_money` and `currency` null.
pub(crate) fn to_table_long(results: &StatementResult) -> Result<TableEnvelope> {
    build_long_table(results, &[])
}

/// Export results to a long-format table with node filtering.
///
/// Returns the same schema as [`to_table_long`] after filtering rows to the
/// requested node ids.
pub(crate) fn to_table_long_filtered(
    results: &StatementResult,
    node_filter: &[&str],
) -> Result<TableEnvelope> {
    build_long_table(results, node_filter)
}

/// Export results to a wide-format table.
///
/// Returns a table with one row per unique period and one measure column per
/// node. Missing `(node, period)` values are encoded as `NaN`.
pub(crate) fn to_table_wide(results: &StatementResult) -> Result<TableEnvelope> {
    let mut all_periods: Vec<PeriodId> = results
        .nodes
        .values()
        .flat_map(|period_map| period_map.keys().cloned())
        .collect();
    all_periods.sort();
    all_periods.dedup();

    let period_strings: Vec<String> = all_periods.iter().map(ToString::to_string).collect();
    let mut columns = vec![
        TableColumn::new("period_id", TableColumnData::String(period_strings))
            .with_role(TableColumnRole::Index),
    ];

    for (node_id, period_map) in &results.nodes {
        let mut node_values = Vec::with_capacity(all_periods.len());
        for period_id in &all_periods {
            node_values.push(period_map.get(period_id).copied().unwrap_or(f64::NAN));
        }

        columns.push(
            TableColumn::new(node_id.clone(), TableColumnData::Float64(node_values))
                .with_role(TableColumnRole::Measure),
        );
    }

    TableEnvelope::new_with_metadata(columns, table_metadata("wide")).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::dates::PeriodId;
    use indexmap::IndexMap;

    fn create_test_results() -> StatementResult {
        let mut nodes = IndexMap::new();

        // Revenue
        let mut revenue_periods = IndexMap::new();
        revenue_periods.insert(PeriodId::quarter(2025, 1), 100_000.0);
        revenue_periods.insert(PeriodId::quarter(2025, 2), 110_000.0);
        nodes.insert("revenue".to_string(), revenue_periods);

        // COGS
        let mut cogs_periods = IndexMap::new();
        cogs_periods.insert(PeriodId::quarter(2025, 1), 60_000.0);
        cogs_periods.insert(PeriodId::quarter(2025, 2), 66_000.0);
        nodes.insert("cogs".to_string(), cogs_periods);

        // Gross profit
        let mut gp_periods = IndexMap::new();
        gp_periods.insert(PeriodId::quarter(2025, 1), 40_000.0);
        gp_periods.insert(PeriodId::quarter(2025, 2), 44_000.0);
        nodes.insert("gross_profit".to_string(), gp_periods);

        StatementResult {
            nodes,
            ..StatementResult::default()
        }
    }

    fn string_column<'a>(table: &'a TableEnvelope, name: &str) -> &'a [String] {
        table
            .column(name)
            .expect("column should exist")
            .as_strings()
            .expect("column should be string")
    }

    fn nullable_string_column<'a>(table: &'a TableEnvelope, name: &str) -> &'a [Option<String>] {
        table
            .column(name)
            .expect("column should exist")
            .as_nullable_strings()
            .expect("column should be nullable string")
    }

    fn float_column<'a>(table: &'a TableEnvelope, name: &str) -> &'a [f64] {
        table
            .column(name)
            .expect("column should exist")
            .as_f64()
            .expect("column should be float")
    }

    fn nullable_float_column<'a>(table: &'a TableEnvelope, name: &str) -> &'a [Option<f64>] {
        table
            .column(name)
            .expect("column should exist")
            .as_nullable_f64()
            .expect("column should be nullable float")
    }

    #[test]
    fn test_to_table_long() {
        let results = create_test_results();
        let table = to_table_long(&results).expect("should convert to table");

        assert_eq!(table.row_count, 6); // 3 nodes × 2 periods
        assert_eq!(table.columns.len(), 6);

        let node_ids = string_column(&table, "node_id");
        assert_eq!(node_ids[0], "revenue");

        let values = float_column(&table, "value");
        assert_eq!(values[0], 100_000.0);
    }

    #[test]
    fn test_to_table_long_filtered() {
        let results = create_test_results();
        let table = to_table_long_filtered(&results, &["revenue", "cogs"])
            .expect("should convert to table");

        assert_eq!(table.row_count, 4); // 2 nodes × 2 periods
        assert_eq!(table.columns.len(), 6);

        let node_ids = string_column(&table, "node_id");
        let unique_nodes: std::collections::HashSet<String> = node_ids.iter().cloned().collect();
        assert_eq!(unique_nodes.len(), 2);
        assert!(unique_nodes.contains("revenue"));
        assert!(unique_nodes.contains("cogs"));
    }

    #[test]
    fn test_to_table_long_filtered_empty_includes_all() {
        let results = create_test_results();
        let table = to_table_long_filtered(&results, &[]).expect("should convert to table");

        assert_eq!(table.row_count, 6); // All 3 nodes × 2 periods
    }

    #[test]
    fn test_to_table_wide() {
        let results = create_test_results();
        let table = to_table_wide(&results).expect("should convert to table");

        assert_eq!(table.row_count, 2); // 2 periods
        assert_eq!(table.columns.len(), 4); // period_id + 3 nodes

        let col_names: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();
        assert!(col_names.contains(&"period_id".to_string()));
        assert!(col_names.contains(&"revenue".to_string()));
        assert!(col_names.contains(&"cogs".to_string()));
        assert!(col_names.contains(&"gross_profit".to_string()));

        let revenue = float_column(&table, "revenue");
        assert_eq!(revenue[0], 100_000.0);

        let cogs = float_column(&table, "cogs");
        assert_eq!(cogs[0], 60_000.0);
    }

    #[test]
    fn test_to_table_wide_period_order() {
        let results = create_test_results();
        let table = to_table_wide(&results).expect("should convert to table");

        let period_ids = string_column(&table, "period_id");
        assert_eq!(period_ids[0], "2025Q1");
        assert_eq!(period_ids[1], "2025Q2");
    }

    #[test]
    fn test_empty_results() {
        let results = StatementResult::default();

        let table_long = to_table_long(&results).expect("test should succeed");
        assert_eq!(table_long.row_count, 0);
        assert_eq!(table_long.columns.len(), 6);
        assert_eq!(nullable_float_column(&table_long, "value_money").len(), 0);
        assert_eq!(nullable_string_column(&table_long, "currency").len(), 0);

        let table_wide = to_table_wide(&results).expect("test should succeed");
        assert_eq!(table_wide.row_count, 0);
        assert_eq!(table_wide.columns.len(), 1);
    }
}
