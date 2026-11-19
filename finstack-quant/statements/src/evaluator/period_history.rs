//! Columnar per-period evaluation history.
//!
//! Periods are stored as rows of `Option<f64>` aligned to interned [`NodeId`]
//! columns. Lookups use column indices — no per-period `String` maps.

use crate::types::NodeId;
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::{Period, PeriodId, PeriodPlan};
use indexmap::IndexMap;
use std::sync::Arc;

/// Columnar history of evaluated node values, one row per period.
#[derive(Debug, Clone, Default)]
pub struct PeriodHistory {
    node_to_column: Arc<IndexMap<NodeId, usize>>,
    period_index: IndexMap<PeriodId, usize>,
    rows: Vec<Vec<Option<f64>>>,
    /// Typed cells of each row, aligned with `rows`. A slot is `None` when
    /// the row was stored as floats only.
    cells: Vec<Vec<Option<super::cell::Cell>>>,
    period_bounds: Arc<IndexMap<PeriodId, (Date, Date)>>,
    /// The model's period plan; `None` only for an empty history.
    plan: Option<Arc<PeriodPlan>>,
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
            cells: Vec::new(),
            period_bounds: Arc::default(),
            plan: None,
        }
    }

    /// Create history retaining the model's explicit calendar boundaries.
    ///
    /// Fiscal daily identifiers alone do not identify Gregorian dates or leap
    /// years. The shared bounds preserve that information through historical
    /// expression evaluation without copying the model timeline per context.
    pub(crate) fn with_periods(
        node_to_column: Arc<IndexMap<NodeId, usize>>,
        plan: &PeriodPlan,
    ) -> Self {
        let periods: &[Period] = plan;
        let mut history = Self::new(node_to_column);
        history.period_bounds = Arc::new(
            periods
                .iter()
                .map(|period| (period.id, (period.start, period.end)))
                .collect(),
        );
        history.plan = Some(Arc::new(plan.clone()));
        history
    }

    /// The model's period plan with its calendar context.
    ///
    /// # Errors
    ///
    /// Returns an evaluation error for an empty history, which has no plan.
    pub(crate) fn model_plan(&self) -> crate::error::Result<&PeriodPlan> {
        self.plan
            .as_deref()
            .ok_or_else(|| crate::error::Error::eval("evaluation history has no period plan"))
    }

    /// Explicit start and exclusive end dates for a model period.
    pub(crate) fn period_bounds(&self, period: &PeriodId) -> Option<(Date, Date)> {
        self.period_bounds.get(period).copied()
    }

    /// Build history from the named per-period maps used by tests and the
    /// public [`EvaluationContext::new`](super::EvaluationContext::new)
    /// constructor.
    ///
    /// # Arguments
    ///
    /// * `node_to_column` - Column layout for interned nodes. Extra keys in
    ///   `named` are appended so historical lookups still resolve them.
    /// * `periods` - The model's periods with their dates, in timeline order,
    ///   retained for calendar-window lookups; may be empty
    /// * `named` - Period → (node name → value) maps, typically built by
    ///   tests or by converting a completed [`StatementResult`](super::StatementResult).
    #[must_use]
    pub fn from_named_maps(
        node_to_column: Arc<IndexMap<NodeId, usize>>,
        plan: &PeriodPlan,
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
        let mut history = Self::with_periods(Arc::clone(&node_to_column), plan);
        history.rows.reserve(named.len());
        history.period_index.reserve(named.len());
        for (period, values) in named {
            let mut row = vec![None; n];
            for (name, value) in values {
                if let Some(&idx) = node_to_column.get(name.as_str())
                    && let Some(slot) = row.get_mut(idx)
                {
                    *slot = Some(*value);
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
    pub fn push_row(&mut self, period: PeriodId, row: Vec<Option<f64>>) {
        self.push_typed_row(period, row, Vec::new());
    }

    /// Insert or replace the row for `period` with its typed cells.
    ///
    /// `cells` is aligned with `row`; a shorter vector leaves the remaining
    /// slots float-only.
    pub(crate) fn push_typed_row(
        &mut self,
        period: PeriodId,
        mut row: Vec<Option<f64>>,
        mut cells: Vec<Option<super::cell::Cell>>,
    ) {
        row.resize(self.node_to_column.len(), None);
        cells.resize(self.node_to_column.len(), None);
        if let Some(&idx) = self.period_index.get(&period) {
            if let Some(slot) = self.rows.get_mut(idx) {
                *slot = row;
            }
            if let Some(slot) = self.cells.get_mut(idx) {
                *slot = cells;
            }
        } else {
            self.period_index.insert(period, self.rows.len());
            self.rows.push(row);
            self.cells.push(cells);
        }
    }

    /// Typed cells of the row for `period`, if present.
    pub(crate) fn cell_row(&self, period: &PeriodId) -> Option<&[Option<super::cell::Cell>]> {
        let idx = *self.period_index.get(period)?;
        self.cells.get(idx).map(Vec::as_slice)
    }

    /// The typed value a formula reads for `node_id` at a past `period`:
    /// the stored cell, or the float as an untyped number. `None` when the
    /// slot was never evaluated.
    pub(crate) fn get_operand(
        &self,
        node_id: &str,
        period: &PeriodId,
    ) -> Option<super::value::Value> {
        let col = *self.node_to_column.get(node_id)?;
        if let Some(cell) = self.cell_row(period)?.get(col).copied().flatten() {
            return Some(super::value::Value::from_cell(cell));
        }
        self.row(period)?
            .get(col)
            .copied()
            .flatten()
            .map(super::value::Value::from_float)
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
    #[cfg(test)]
    #[must_use]
    pub fn get_value(&self, node_id: &str, period: &PeriodId) -> Option<f64> {
        let col = *self.node_to_column.get(node_id)?;
        self.row(period)?.get(col).copied().flatten()
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

        assert_eq!(history.keys().copied().collect::<Vec<_>>(), vec![q1]);
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

        let plan = finstack_quant_core::dates::build_periods("2025Q1..Q2", None).expect("plan");
        let history = PeriodHistory::from_named_maps(Arc::clone(&cols), &plan, &named);
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
