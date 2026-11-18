//! Evaluation context that backs formula execution during model evaluation.
//!
//! The context owns per-period state such as evaluated node values,
//! historical lookups, and optional capital-structure cashflows. Evaluator
//! components update and query this structure while traversing the dependency
//! graph.

use crate::error::{Error, Result};
use crate::evaluator::PeriodHistory;
use crate::evaluator::results::EvalWarning;
use crate::types::NodeId;
use finstack_quant_core::dates::{PeriodId, PeriodKind, PeriodPlan};
use indexmap::IndexMap;
use std::sync::Arc;

/// Evaluation context for a single period.
///
/// Tracks node values for the current period and provides access to historical values.
/// Read-only shared data (`node_to_column`, `history`,
/// `historical_capital_structure_cashflows`) is wrapped in `Arc` so that per-period
/// and per-aggregate-function context construction is O(1) instead of O(P×N).
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Current period being evaluated
    pub period_id: PeriodId,

    /// Period frequency (quarterly, monthly, etc.)
    pub(crate) period_kind: PeriodKind,

    /// Map of node_id → column index for the current period
    pub node_to_column: Arc<IndexMap<NodeId, usize>>,

    /// Columnar historical node values sharing the context's column layout.
    pub(crate) history: Arc<PeriodHistory>,

    /// Historical capital-structure snapshots: period_id → cashflows
    pub historical_capital_structure_cashflows:
        Arc<IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>>,

    /// Current period results being built.
    /// Uses `Option<f64>` to distinguish between "not yet evaluated" (`None`) and "evaluated to NaN" (`Some(NaN)`).
    pub(crate) current_values: Vec<Option<f64>>,

    /// State of each current-period cell, in the same column layout as
    /// `current_values`. `None` for values set outside the evaluator's
    /// precedence loop. This is the authoritative state; `current_values` is
    /// its numeric view for the formula kernels.
    pub(crate) current_cells: Vec<Option<super::cell::Cell>>,

    /// Evaluation layer that produced each current-period value, in the same
    /// column layout as `current_values`. `None` for values set outside the
    /// evaluator's precedence loop.
    pub(crate) current_sources: Vec<Option<super::CellSource>>,

    /// Capital structure cashflows (optional)
    pub capital_structure_cashflows: Option<crate::capital_structure::CapitalStructureCashflows>,

    /// Warnings collected while evaluating this period
    pub warnings: Vec<EvalWarning>,
}

impl EvaluationContext {
    /// Create a new evaluation context for a period.
    ///
    /// # Arguments
    /// * `period_id` - Period currently being evaluated
    /// * `periods` - The model's periods with their dates, in timeline order.
    ///   Calendar-window functions such as `ttm` read them to find a window's
    ///   periods; with an empty slice those functions return `NaN`.
    /// * `node_to_column` - Mapping from node identifiers to their column index
    /// * `named_maps` - Prior period results keyed by node name, available for
    ///   lag, rolling-window, and forecast lookups. Historical-only node names
    ///   are appended to the shared column layout.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_quant_statements::evaluator::EvaluationContext;
    /// # use finstack_quant_statements::types::NodeId;
    /// # use finstack_quant_core::dates::PeriodId;
    /// # use indexmap::IndexMap;
    /// # use std::sync::Arc;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
    /// let mut columns = IndexMap::new();
    /// columns.insert(NodeId::new("revenue"), 0);
    /// let plan = finstack_quant_core::dates::build_periods("2025Q1..Q1", None)?;
    /// let ctx = EvaluationContext::new(period, &plan, Arc::new(columns), Arc::new(IndexMap::new()));
    /// assert_eq!(ctx.period_id, period);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(
        period_id: PeriodId,
        plan: &PeriodPlan,
        node_to_column: Arc<IndexMap<NodeId, usize>>,
        named_maps: Arc<IndexMap<PeriodId, IndexMap<String, f64>>>,
    ) -> Self {
        let history = Arc::new(PeriodHistory::from_named_maps(
            node_to_column,
            plan,
            named_maps.as_ref(),
        ));
        Self::new_with_history(period_id, history, Arc::new(IndexMap::new()))
    }

    /// Create a new evaluation context with shared columnar history and
    /// capital-structure cashflow snapshots.
    ///
    /// This constructor is used by the evaluator hot path so each per-period
    /// context can reuse the same `Arc`-backed history without cloning it.
    pub(crate) fn new_with_history(
        period_id: PeriodId,
        history: Arc<PeriodHistory>,
        historical_capital_structure_cashflows: Arc<
            IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>,
        >,
    ) -> Self {
        let node_to_column = history.shared_node_to_column();
        let num_nodes = node_to_column.len();
        let period_kind = period_id.kind();
        Self {
            period_id,
            period_kind,
            node_to_column,
            history,
            historical_capital_structure_cashflows,
            current_values: vec![None; num_nodes],
            current_cells: vec![None; num_nodes],
            current_sources: vec![None; num_nodes],
            capital_structure_cashflows: None,
            warnings: Vec::new(),
        }
    }

    /// Replace the current-period capital-structure cashflows. The waterfall
    /// revises them after the first evaluation pass.
    pub(crate) fn set_capital_structure_cashflows(
        &mut self,
        cashflows: crate::capital_structure::CapitalStructureCashflows,
    ) {
        self.capital_structure_cashflows = Some(cashflows);
    }

    /// Set the value for a node in the current period.
    ///
    /// Accepts any `f64` value, including `NaN`. Values are stored even when they
    /// originate from forecasts or capital-structure flows so that precedence
    /// resolution can make informed decisions later.
    ///
    /// Non-finite values (`NaN`, `Inf`, `-Inf`) are accepted but emit a warning
    /// so downstream consumers can detect propagation of bad values.
    ///
    /// # Arguments
    /// * `node_id` - Identifier of the node being updated
    /// * `value` - Numeric result to store for the current period
    ///
    /// # Errors
    ///
    /// Returns a node-not-found error if `node_id` is not part of this
    /// evaluation plan. Non-finite `value`s are deliberately stored and
    /// reported as warnings rather than errors, allowing DSL guards such as
    /// `coalesce` or conditionals to decide how to handle them.
    pub fn set_value(&mut self, node_id: &str, value: f64) -> Result<()> {
        let idx = self
            .node_to_column
            .get(node_id)
            .ok_or_else(|| Error::node_not_found(node_id))?;

        // Finiteness validation: detect NaN / Inf early and emit a warning.
        // We still store the value so that downstream formulas can decide how
        // to handle it (e.g., coalesce, if-then-else guards), but the warning
        // makes it visible in results metadata.
        if !value.is_finite() {
            self.warnings.push(EvalWarning::NonFiniteValue {
                node_id: node_id.to_string(),
                period: self.period_id,
                value,
            });
        }

        self.current_values[*idx] = Some(value);
        self.current_cells[*idx] = None;
        Ok(())
    }

    /// Get the value for a node in the current period.
    ///
    /// Returns an error if the node has not been evaluated yet (value is `None`).
    /// Returns `Ok(NaN)` if the node was evaluated but resulted in `NaN`.
    ///
    /// # Arguments
    /// * `node_id` - Identifier to query
    ///
    /// # Errors
    ///
    /// Returns a node-not-found error if `node_id` is outside the evaluation
    /// plan, or an evaluation error if it is in the plan but has not yet been
    /// computed for this period. A computed `NaN` is returned as `Ok(NaN)` so
    /// callers can distinguish it from an absent value.
    pub fn get_value(&self, node_id: &str) -> Result<f64> {
        let idx = self
            .node_to_column
            .get(node_id)
            .ok_or_else(|| Error::node_not_found(node_id))?;

        match self.current_values[*idx] {
            Some(value) => Ok(value),
            None => Err(Error::eval(format!(
                "Node '{}' has not been evaluated yet in period {}. \
                 This usually indicates a circular dependency or missing value.",
                node_id, self.period_id
            ))),
        }
    }

    /// The typed value a formula reads for a node in the current period.
    ///
    /// A cell stored by the evaluator keeps its type and state. A value stored
    /// as a float (a history row, a check or rollup context) is an untyped
    /// number, or unavailable when it is not finite.
    pub(crate) fn get_operand(&self, node_id: &str) -> Result<super::value::Value> {
        let idx = *self
            .node_to_column
            .get(node_id)
            .ok_or_else(|| Error::node_not_found(node_id))?;
        match self.current_cells[idx] {
            Some(cell) => Ok(super::value::Value::from_cell(cell)),
            None => self.get_value(node_id).map(super::value::Value::from_float),
        }
    }

    /// Get capital structure value for the current period.
    ///
    /// # Arguments
    /// * `component` - Component type: "interest_expense", "interest_expense_cash", "interest_expense_pik",
    ///   "principal_payment", or "debt_balance"
    /// * `instrument_or_total` - Instrument ID or "total" for aggregate
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use finstack_quant_statements::evaluator::EvaluationContext;
    ///
    /// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    /// # let context: EvaluationContext = unimplemented!("obtained during evaluation");
    /// // Total interest (cash + PIK)
    /// let total_interest = context.get_cs_value("interest_expense", "total")?;
    ///
    /// // Cash interest only
    /// let cash_interest = context.get_cs_value("interest_expense_cash", "total")?;
    ///
    /// // PIK interest only
    /// let pik_interest = context.get_cs_value("interest_expense_pik", "total")?;
    /// # let _ = (total_interest, cash_interest, pik_interest);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The component names `fees`, `accrued_interest`, `interest_arrears`,
    /// `principal_pending`, `principal_arrears` and
    /// `principal_advance_reserve` are also supported.
    /// Per-instrument values use their native currencies; `total` values use
    /// the capital structure's reporting-currency aggregation convention.
    ///
    /// # Errors
    ///
    /// Returns a capital-structure error if this evaluation has no cashflow
    /// state, `component` is unsupported, the requested instrument or period
    /// cashflow is absent, or an aggregate cannot satisfy the currency
    /// invariant.
    pub fn get_cs_value(&self, component: &str, instrument_or_total: &str) -> Result<f64> {
        let cs_cashflows = self
            .capital_structure_cashflows
            .as_ref()
            .ok_or_else(|| Error::capital_structure("No capital structure defined in model"))?;
        cs_cashflows.get_component(component, instrument_or_total, &self.period_id)
    }

    /// The typed value of a `cs.<component>.<scope>` reference this period.
    ///
    /// An exact amount in the scope's currency. A consolidated value whose
    /// period could not be consolidated (a missing period-close FX quote, or
    /// no reporting currency) is unavailable; native values still resolve.
    pub(crate) fn get_cs(&self, component: &str, scope: &str) -> Result<super::value::Value> {
        let cs_cashflows = self
            .capital_structure_cashflows
            .as_ref()
            .ok_or_else(|| Error::capital_structure("No capital structure defined in model"))?;
        cs_value(cs_cashflows, component, scope, &self.period_id)
    }

    /// The typed value of a `cs.<component>.<scope>` reference in a completed
    /// period, with the availability rules of [`Self::get_cs`].
    pub(crate) fn get_historical_cs(
        &self,
        component: &str,
        scope: &str,
        period_id: &PeriodId,
    ) -> Result<super::value::Value> {
        let cashflows = self
            .historical_capital_structure_cashflows
            .get(period_id)
            .ok_or_else(|| {
                Error::capital_structure(format!(
                    "No historical capital structure data for period {period_id}"
                ))
            })?;
        cs_value(cashflows, component, scope, period_id)
    }

    /// Record which evaluation layer produced the current-period value of
    /// `node_id`. Unknown nodes are ignored: `set_value` has already rejected
    /// them by the time the evaluator records a source.
    /// Store an evaluated cell and its numeric view for the current period.
    ///
    /// A cell without a value is stored with a `NaN` numeric view, which the
    /// formula kernels read as "no value". No warning is recorded: the cell's
    /// state already says why it has none.
    pub(crate) fn set_cell(&mut self, node_id: &str, cell: super::cell::Cell) -> Result<()> {
        let idx = *self
            .node_to_column
            .get(node_id)
            .ok_or_else(|| Error::node_not_found(node_id))?;
        self.current_values[idx] = Some(cell.numeric().unwrap_or(f64::NAN));
        self.current_cells[idx] = Some(cell);
        Ok(())
    }

    pub(crate) fn set_source(&mut self, node_id: &str, source: super::CellSource) {
        if let Some(idx) = self.node_to_column.get(node_id) {
            self.current_sources[*idx] = Some(source);
        }
    }

    /// The period's published cells, with their value sources, and the
    /// warnings raised while evaluating it.
    ///
    /// A value set outside the precedence loop becomes a scalar value cell,
    /// or a missing cell when it is not finite.
    pub(crate) fn into_results(self) -> (IndexMap<String, super::ResultCell>, Vec<EvalWarning>) {
        let mut results = IndexMap::new();
        for (node_id, idx) in self.node_to_column.iter() {
            let cell = match (self.current_cells[*idx], self.current_values[*idx]) {
                (Some(cell), _) => super::ResultCell::from_cell(cell, self.current_sources[*idx]),
                (None, Some(value)) => super::ResultCell::scalar(value).unwrap_or_else(|_| {
                    super::ResultCell::without_value(super::CellStatus::Missing)
                }),
                (None, None) => continue,
            };
            results.insert(node_id.as_str().to_string(), cell);
        }
        (results, self.warnings)
    }
}

/// One `cs.<component>.<scope>` value of `period_id` read from `cashflows`.
fn cs_value(
    cashflows: &crate::capital_structure::CapitalStructureCashflows,
    component: &str,
    scope: &str,
    period_id: &PeriodId,
) -> Result<super::value::Value> {
    use crate::capital_structure::CsScope;
    let scope: CsScope = scope.parse()?;
    if scope == CsScope::Consolidated && cashflows.consolidation_unavailable.contains_key(period_id)
    {
        return Ok(super::value::Value::Unavailable(
            super::UnavailableReason::MissingFxQuote,
        ));
    }
    cashflows
        .get_money(component.parse()?, &scope, period_id)
        .map(super::value::Value::Money)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::PeriodHistory;
    use crate::types::NodeId;

    #[test]
    fn test_context_creation() {
        let mut node_to_column = IndexMap::new();
        node_to_column.insert(NodeId::new("revenue"), 0);
        node_to_column.insert(NodeId::new("cogs"), 1);

        let ctx = EvaluationContext::new(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            Arc::new(node_to_column),
            Arc::new(IndexMap::new()),
        );

        assert_eq!(ctx.current_values.len(), 2);
    }

    #[test]
    fn test_set_and_get_value() {
        let mut node_to_column = IndexMap::new();
        node_to_column.insert(NodeId::new("revenue"), 0);

        let mut ctx = EvaluationContext::new(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            Arc::new(node_to_column),
            Arc::new(IndexMap::new()),
        );

        ctx.set_value("revenue", 100_000.0).unwrap();
        assert_eq!(ctx.get_value("revenue").unwrap(), 100_000.0);
    }

    #[test]
    fn test_get_historical_value() {
        let mut node_to_column = IndexMap::new();
        node_to_column.insert(NodeId::new("revenue"), 0);

        let mut historical = IndexMap::new();
        let mut q1_results = IndexMap::new();
        q1_results.insert("revenue".to_string(), 100_000.0);
        historical.insert(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            q1_results,
        );

        let ctx = EvaluationContext::new(
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            Arc::new(node_to_column),
            Arc::new(historical),
        );

        let value = ctx.history.get_value(
            "revenue",
            &PeriodId::quarter(2025, 1).expect("valid period fixture"),
        );
        assert_eq!(value, Some(100_000.0));
    }

    #[test]
    fn named_constructor_adopts_layout_expanded_for_historical_only_nodes() {
        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let mut node_to_column = IndexMap::new();
        node_to_column.insert(NodeId::new("revenue"), 0);
        let historical = IndexMap::from_iter([(
            q1,
            IndexMap::from_iter([
                ("revenue".to_string(), 100.0),
                ("legacy_margin".to_string(), 25.0),
            ]),
        )]);

        let ctx = EvaluationContext::new(
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            Arc::new(node_to_column),
            Arc::new(historical),
        );

        assert_eq!(ctx.current_values.len(), 2);
        assert_eq!(ctx.history.get_value("revenue", &q1), Some(100.0));
        assert_eq!(ctx.history.get_value("legacy_margin", &q1), Some(25.0));
        assert!(Arc::ptr_eq(
            &ctx.node_to_column,
            &ctx.history.shared_node_to_column()
        ));
    }

    #[test]
    fn internal_constructor_shares_history_and_its_layout() {
        let columns = Arc::new(IndexMap::from_iter([(NodeId::new("revenue"), 0)]));
        let history = Arc::new(PeriodHistory::new(Arc::clone(&columns)));
        let ctx = EvaluationContext::new_with_history(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            Arc::clone(&history),
            Arc::new(IndexMap::new()),
        );

        assert!(Arc::ptr_eq(&ctx.history, &history));
        assert!(Arc::ptr_eq(&ctx.node_to_column, &columns));
    }
}
