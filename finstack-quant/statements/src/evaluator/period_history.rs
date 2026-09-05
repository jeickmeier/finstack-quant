//! Columnar per-period evaluation history.
//!
//! Periods are stored as rows of `Option<f64>` aligned to interned [`NodeId`]
//! columns. Lookups use column indices — no per-period `String` maps.

use crate::types::NodeId;
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;
use std::sync::Arc;

/// Columnar history of evaluated node values, one row per period.
#[derive(Debug, Clone, Default)]
pub struct PeriodHistory {
    node_to_column: Arc<IndexMap<NodeId, usize>>,
    period_index: IndexMap<PeriodId, usize>,
    rows: Vec<Vec<Option<f64>>>,
}

impl PeriodHistory {
    /// Create an empty history with a fixed column layout.
    ///
    /// # Arguments
    ///
    /// * `node_to_column` - Interned node identifiers mapped to column indices.
    ///   Every later row is stored in this layout.
    #[must_use]
    pub fn new(node_to_column: Arc<IndexMap<NodeId, usize>>) -> Self {
        Self {
            node_to_column,
            period_index: IndexMap::new(),
            rows: Vec::new(),
        }
    }

    /// Build history from the named per-period maps used by tests and the
    /// public [`EvaluationContext::new`](super::EvaluationContext::new)
    /// constructor.
    ///
    /// # Arguments
    ///
    /// * `node_to_column` - Column layout for interned nodes. Extra keys in
    ///   `named` are appended so historical lookups still resolve them.
    /// * `named` - Period → (node name → value) maps, typically built by
    ///   tests or by converting a completed [`StatementResult`](super::StatementResult).
    #[must_use]
    pub fn from_named_maps(
        node_to_column: Arc<IndexMap<NodeId, usize>>,
        named: &IndexMap<PeriodId, IndexMap<String, f64>>,
    ) -> Self {
        let mut col_map = (*node_to_column).clone();
        for period_map in named.values() {
            for key in period_map.keys() {
                if !col_map.contains_key(key.as_str()) {
                    let idx = col_map.len();
                    col_map.insert(NodeId::new(key), idx);
                }
            }
        }
        let node_to_column = Arc::new(col_map);
        let n = node_to_column.len();
        let mut history = Self::new(Arc::clone(&node_to_column));
        history.rows.reserve(named.len());
        history.period_index.reserve(named.len());
        for (period, values) in named {
            let mut row = vec![None; n];
            for (name, value) in values {
                if let Some(&idx) = node_to_column.get(name.as_str()) {
                    if let Some(slot) = row.get_mut(idx) {
                        *slot = Some(*value);
                    }
                }
            }
            history.push_row(*period, row);
        }
        history
    }

    /// Append or replace the row for `period`.
    ///
    /// # Arguments
    ///
    /// * `period` - Period identifier for this row. Re-pushing the same
    ///   period overwrites the previous row.
    /// * `row` - Values aligned to [`node_to_column`](Self::node_to_column).
    ///   Shorter rows are padded with `None`; extra slots are dropped.
    pub fn push_row(&mut self, period: PeriodId, mut row: Vec<Option<f64>>) {
        row.resize(self.node_to_column.len(), None);
        if let Some(&idx) = self.period_index.get(&period) {
            if let Some(slot) = self.rows.get_mut(idx) {
                *slot = row;
            }
        } else {
            self.period_index.insert(period, self.rows.len());
            self.rows.push(row);
        }
    }

    /// Number of stored periods.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no periods have been stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Whether a row exists for `period`.
    ///
    /// # Arguments
    ///
    /// * `period` - Period to look up.
    #[must_use]
    pub fn contains_key(&self, period: &PeriodId) -> bool {
        self.period_index.contains_key(period)
    }

    /// Interned column layout shared with the evaluator.
    #[must_use]
    pub fn node_to_column(&self) -> &IndexMap<NodeId, usize> {
        &self.node_to_column
    }

    /// Shared interned column layout used by every stored row.
    #[must_use]
    pub fn shared_node_to_column(&self) -> Arc<IndexMap<NodeId, usize>> {
        Arc::clone(&self.node_to_column)
    }

    /// Period identifiers in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &PeriodId> {
        self.period_index.keys()
    }

    /// Row for `period`, if present.
    ///
    /// # Arguments
    ///
    /// * `period` - Period whose column vector is requested.
    #[must_use]
    pub fn row(&self, period: &PeriodId) -> Option<&[Option<f64>]> {
        let idx = *self.period_index.get(period)?;
        self.rows.get(idx).map(Vec::as_slice)
    }

    /// Value of `node_id` at `period`, if that slot was evaluated.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Node name matching an interned column.
    /// * `period` - Historical period to read.
    #[must_use]
    pub fn get_value(&self, node_id: &str, period: &PeriodId) -> Option<f64> {
        let col = *self.node_to_column.get(node_id)?;
        self.row(period)?.get(col).copied().flatten()
    }

    /// Iterate `(period, row)` pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (PeriodId, &[Option<f64>])> {
        self.period_index
            .iter()
            .filter_map(|(period, &idx)| self.rows.get(idx).map(|row| (*period, row.as_slice())))
    }
}

#[cfg(test)]
mod tests {
    use super::PeriodHistory;
    use crate::types::NodeId;
    use finstack_quant_core::dates::PeriodId;
    use indexmap::IndexMap;
    use std::sync::Arc;

    fn columns(names: &[&str]) -> Arc<IndexMap<NodeId, usize>> {
        Arc::new(
            names
                .iter()
                .enumerate()
                .map(|(i, name)| (NodeId::new(*name), i))
                .collect(),
        )
    }

    #[test]
    fn push_row_round_trips_by_node_and_period() {
        let cols = columns(&["revenue", "cogs"]);
        let mut history = PeriodHistory::new(Arc::clone(&cols));
        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        history.push_row(q1, vec![Some(100.0), Some(40.0)]);

        assert_eq!(history.len(), 1);
        assert!(history.contains_key(&q1));
        assert_eq!(history.get_value("revenue", &q1), Some(100.0));
        assert_eq!(history.get_value("cogs", &q1), Some(40.0));
        assert_eq!(history.get_value("missing", &q1), None);
        assert_eq!(
            history.get_value(
                "revenue",
                &PeriodId::quarter(2025, 2).expect("valid period fixture")
            ),
            None
        );
    }

    #[test]
    fn from_named_maps_preserves_lookups_without_string_maps_at_read() {
        let cols = columns(&["revenue"]);
        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let mut named = IndexMap::new();
        named.insert(q1, IndexMap::from_iter([("revenue".to_string(), 10.0)]));
        named.insert(q2, IndexMap::from_iter([("revenue".to_string(), 12.0)]));

        let history = PeriodHistory::from_named_maps(Arc::clone(&cols), &named);
        assert_eq!(history.get_value("revenue", &q1), Some(10.0));
        assert_eq!(history.get_value("revenue", &q2), Some(12.0));
        let keys: Vec<_> = history.keys().copied().collect();
        assert_eq!(keys, vec![q1, q2]);
    }

    #[test]
    fn none_slots_are_absent_not_zero() {
        let cols = columns(&["revenue", "cogs"]);
        let mut history = PeriodHistory::new(cols);
        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        history.push_row(q1, vec![Some(100.0), None]);
        assert_eq!(history.get_value("revenue", &q1), Some(100.0));
        assert_eq!(history.get_value("cogs", &q1), None);
    }

    #[test]
    fn shared_node_to_column_returns_the_exact_layout_arc() {
        let cols = columns(&["revenue", "cogs"]);
        let history = PeriodHistory::new(Arc::clone(&cols));

        assert!(Arc::ptr_eq(&cols, &history.shared_node_to_column()));
    }
}
