//! Table export functionality for [`StatementResult`].

use super::{CellStatus, ResultCell, StatementResult};
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

impl StatementResult {
    /// Periods the tables cover: the result's plan in timeline order, or, for
    /// a hand-built result without a plan, the periods its cells use in
    /// ascending identifier order.
    fn table_periods(&self) -> Vec<PeriodId> {
        if let Some(plan) = &self.periods {
            return plan.iter().map(|period| period.id).collect();
        }
        let mut periods: Vec<PeriodId> = self
            .cells
            .values()
            .flat_map(|cells| cells.keys().copied())
            .collect();
        periods.sort();
        periods.dedup();
        periods
    }

    /// Export the exact long-format table.
    ///
    /// Schema: `(node_id, period_id, status, value, amount, currency,
    /// value_type, reason, source)`. One row is emitted for every node and
    /// every period of the result's plan, in declaration and timeline order,
    /// including cells that have no value.
    ///
    /// - `status` is `value`, `missing`, `masked` or `unavailable`.
    /// - `value` is the float view of a value cell and null otherwise.
    /// - `amount` is the exact decimal text of a monetary value cell and
    ///   `currency` its ISO code; both are null for scalar and value-less
    ///   cells.
    /// - `value_type` is `monetary` or `scalar`, from the node's units.
    /// - `reason` names why an unavailable cell has no value.
    /// - `source` is the value source selected for the cell (`value`,
    ///   `forecast`, `formula`), or null when none was.
    ///
    /// # Errors
    ///
    /// Returns a table-construction error if the result cannot be represented
    /// as a valid [`finstack_quant_core::table::TableEnvelope`]. Empty results
    /// are valid and produce an empty table with the full schema.
    pub fn to_table_long(&self) -> Result<TableEnvelope> {
        let periods = self.table_periods();
        let mut node_ids = Vec::new();
        let mut period_ids = Vec::new();
        let mut statuses = Vec::new();
        let mut values = Vec::new();
        let mut amounts = Vec::new();
        let mut currencies = Vec::new();
        let mut value_types = Vec::new();
        let mut reasons = Vec::new();
        let mut sources = Vec::new();

        for (node_id, cells) in &self.cells {
            let monetary = matches!(
                self.node_value_types.get(node_id),
                Some(NodeValueType::Monetary { .. })
            );
            for period_id in &periods {
                let cell = cells
                    .get(period_id)
                    .copied()
                    .unwrap_or_else(|| ResultCell::without_value(CellStatus::Missing));
                let money = cell.value.and_then(|value| value.as_money());
                node_ids.push(node_id.clone());
                period_ids.push(period_id.to_string());
                statuses.push(cell.status.as_str().to_string());
                values.push(cell.numeric());
                amounts.push(money.map(|money| money.amount_decimal().normalize().to_string()));
                currencies.push(money.map(|money| money.currency().to_string()));
                value_types.push(if monetary { "monetary" } else { "scalar" }.to_string());
                reasons.push(cell.reason.map(|reason| reason.as_str().to_string()));
                sources.push(cell.source.map(|source| source.as_str().to_string()));
            }
        }

        TableEnvelope::new(
            vec![
                TableColumn::new("node_id", TableColumnData::String(node_ids))
                    .with_role(TableColumnRole::Dimension),
                TableColumn::new("period_id", TableColumnData::String(period_ids))
                    .with_role(TableColumnRole::Index),
                TableColumn::new("status", TableColumnData::String(statuses))
                    .with_role(TableColumnRole::Attribute),
                TableColumn::new("value", TableColumnData::NullableFloat64(values))
                    .with_role(TableColumnRole::Measure),
                TableColumn::new("amount", TableColumnData::NullableString(amounts))
                    .with_role(TableColumnRole::Measure),
                TableColumn::new("currency", TableColumnData::NullableString(currencies))
                    .with_role(TableColumnRole::Attribute),
                TableColumn::new("value_type", TableColumnData::String(value_types))
                    .with_role(TableColumnRole::Attribute),
                TableColumn::new("reason", TableColumnData::NullableString(reasons))
                    .with_role(TableColumnRole::Attribute),
                TableColumn::new("source", TableColumnData::NullableString(sources))
                    .with_role(TableColumnRole::Attribute),
            ],
            table_metadata("long"),
        )
        .map_err(Into::into)
    }

    /// Export the approximate wide-format numeric view.
    ///
    /// Schema: `(period_id, <node1>, <node2>, ...)`. One row is emitted per
    /// period of the result's plan in timeline order, and node columns follow
    /// declaration order. Each cell is the float view of a value cell, and
    /// null for a cell without a value, so absence is never read as zero.
    /// Each node column's metadata records its units: `value_type`
    /// (`scalar` or `monetary`) and, for money, `currency`. Use
    /// [`to_table_long`](Self::to_table_long) for exact amounts, status and
    /// reasons.
    ///
    /// # Errors
    ///
    /// Returns a table-construction error if a node identifier or result shape
    /// cannot be represented in a valid table envelope. Empty results are valid
    /// and produce a table containing only `period_id`.
    pub fn to_table_wide(&self) -> Result<TableEnvelope> {
        let periods = self.table_periods();
        let period_strings: Vec<String> = periods.iter().map(ToString::to_string).collect();
        let mut columns = vec![
            TableColumn::new("period_id", TableColumnData::String(period_strings))
                .with_role(TableColumnRole::Index),
        ];

        for (node_id, cells) in &self.cells {
            let node_values = periods
                .iter()
                .map(|period_id| cells.get(period_id).and_then(ResultCell::numeric))
                .collect();
            let mut units = IndexMap::new();
            match self.node_value_types.get(node_id) {
                Some(NodeValueType::Monetary { currency }) => {
                    units.insert("value_type".to_string(), json!("monetary"));
                    units.insert("currency".to_string(), json!(currency.to_string()));
                }
                Some(NodeValueType::Scalar) | None => {
                    units.insert("value_type".to_string(), json!("scalar"));
                }
            }
            columns.push(
                TableColumn::new(
                    node_id.clone(),
                    TableColumnData::NullableFloat64(node_values),
                )
                .with_role(TableColumnRole::Measure)
                .with_metadata(units),
            );
        }

        TableEnvelope::new(columns, table_metadata("wide")).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::UnavailableReason;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;

    fn month(index: u8) -> PeriodId {
        PeriodId::month(2025, index).expect("valid period fixture")
    }

    /// STATEMENTS-EVALUATION-013 worked example: monthly March CapEx of
    /// USD 125.01 exports as exact `125.01`, and a missing April observation
    /// stays visible.
    fn capex_result() -> StatementResult {
        let mut results = StatementResult::new();
        results.periods =
            Some(finstack_quant_core::dates::build_periods("2025M03..M05", None).expect("plan"));
        results.set_cell(
            "capex",
            month(3),
            ResultCell::money(Money::from_decimal_str("125.01", Currency::USD).expect("money")),
        );
        results.set_cell(
            "capex",
            month(5),
            ResultCell::unavailable(UnavailableReason::MissingInput),
        );
        results.node_value_types.insert(
            "capex".to_string(),
            NodeValueType::Monetary {
                currency: Currency::USD,
            },
        );
        results.set_scalar("margin", month(3), 0.0).expect("finite");
        results.set_cell(
            "margin",
            month(4),
            ResultCell::without_value(CellStatus::Masked),
        );
        results
            .node_value_types
            .insert("margin".to_string(), NodeValueType::Scalar);
        results
    }

    fn strings(table: &TableEnvelope, column: &str) -> Vec<Option<String>> {
        match &table.column(column).expect("column").data {
            TableColumnData::String(values) => values.iter().cloned().map(Some).collect(),
            TableColumnData::NullableString(values) => values.clone(),
            other => panic!("{column} is not a string column: {other:?}"),
        }
    }

    fn floats(table: &TableEnvelope, column: &str) -> Vec<Option<f64>> {
        match &table.column(column).expect("column").data {
            TableColumnData::NullableFloat64(values) => values.clone(),
            other => panic!("{column} is not a nullable float column: {other:?}"),
        }
    }

    fn some(values: &[Option<&str>]) -> Vec<Option<String>> {
        values
            .iter()
            .map(|value| value.map(str::to_string))
            .collect()
    }

    #[test]
    fn long_table_is_exact_and_keeps_every_cell_with_its_status() {
        let table = capex_result().to_table_long().expect("table");
        assert_eq!(table.metadata["layout"], "long");
        assert_eq!(
            table
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            [
                "node_id",
                "period_id",
                "status",
                "value",
                "amount",
                "currency",
                "value_type",
                "reason",
                "source"
            ]
        );
        // Two nodes over the three periods of the plan: the April CapEx cell
        // is missing from the map and still has a row.
        assert_eq!(table.columns[0].len(), 6);
        assert_eq!(
            strings(&table, "period_id")[..3],
            some(&[Some("2025M03"), Some("2025M04"), Some("2025M05")])[..]
        );
        assert_eq!(
            strings(&table, "status"),
            some(&[
                Some("value"),
                Some("missing"),
                Some("unavailable"),
                Some("value"),
                Some("masked"),
                Some("missing"),
            ])
        );
        assert_eq!(
            strings(&table, "amount")[..3],
            some(&[Some("125.01"), None, None])[..]
        );
        assert_eq!(
            strings(&table, "currency")[..3],
            some(&[Some("USD"), None, None])[..]
        );
        assert_eq!(
            floats(&table, "value"),
            [Some(125.01), None, None, Some(0.0), None, None],
            "a known zero is a value; cells without a value are null"
        );
        assert_eq!(
            strings(&table, "reason")[2],
            Some("missing_input".to_string())
        );
        assert_eq!(
            strings(&table, "value_type"),
            some(&[
                Some("monetary"),
                Some("monetary"),
                Some("monetary"),
                Some("scalar"),
                Some("scalar"),
                Some("scalar"),
            ])
        );
    }

    #[test]
    fn long_table_amounts_round_trip_to_the_exact_money() {
        let result = capex_result();
        let table = result.to_table_long().expect("table");
        let nodes = strings(&table, "node_id");
        let periods = strings(&table, "period_id");
        let amounts = strings(&table, "amount");
        let currencies = strings(&table, "currency");
        let mut checked = 0;
        for row in 0..nodes.len() {
            let (Some(amount), Some(currency)) = (&amounts[row], &currencies[row]) else {
                continue;
            };
            let node = nodes[row].as_deref().expect("node id");
            let period: PeriodId = periods[row]
                .as_deref()
                .expect("period")
                .parse()
                .expect("id");
            let parsed = Money::from_decimal_str(amount, currency.parse().expect("currency"))
                .expect("exact decimal text");
            assert_eq!(Some(parsed), result.get_money(node, &period));
            checked += 1;
        }
        assert!(checked > 0, "the fixture has monetary cells");
    }

    #[test]
    fn wide_table_is_a_numeric_view_with_the_full_period_index_and_units() {
        let table = capex_result().to_table_wide().expect("table");
        assert_eq!(table.metadata["layout"], "wide");
        assert_eq!(
            strings(&table, "period_id"),
            some(&[Some("2025M03"), Some("2025M04"), Some("2025M05")])
        );
        assert_eq!(floats(&table, "capex"), [Some(125.01), None, None]);
        assert_eq!(floats(&table, "margin"), [Some(0.0), None, None]);
        let capex = table.column("capex").expect("column");
        assert_eq!(capex.metadata["value_type"], "monetary");
        assert_eq!(capex.metadata["currency"], "USD");
        assert_eq!(
            table.column("margin").expect("column").metadata["value_type"],
            "scalar"
        );
    }

    #[test]
    fn tables_follow_the_plan_order_when_insertion_order_differs() {
        let mut results = StatementResult::new();
        results.periods =
            Some(finstack_quant_core::dates::build_periods("2025M03..M05", None).expect("plan"));
        for index in [5, 3, 4] {
            results
                .set_scalar("x", month(index), f64::from(index))
                .expect("finite");
        }
        assert_eq!(
            floats(&results.to_table_wide().expect("wide"), "x"),
            [Some(3.0), Some(4.0), Some(5.0)]
        );
        assert_eq!(
            floats(&results.to_table_long().expect("long"), "value"),
            [Some(3.0), Some(4.0), Some(5.0)]
        );
    }

    #[test]
    fn empty_results_export_the_full_schema() {
        let results = StatementResult::new();
        let long = results.to_table_long().expect("long");
        assert!(long.columns[0].is_empty());
        assert_eq!(long.columns.len(), 9);
        let wide = results.to_table_wide().expect("wide");
        assert_eq!(wide.columns.len(), 1);
    }
}
