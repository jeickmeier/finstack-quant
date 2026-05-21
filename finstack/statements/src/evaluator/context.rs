//! Evaluation context that backs formula execution during model evaluation.
//!
//! The context owns per-period state such as evaluated node values,
//! historical lookups, and optional capital-structure cashflows. Evaluator
//! components update and query this structure while traversing the dependency
//! graph.

use crate::error::{Error, Result};
use crate::evaluator::results::EvalWarning;
use crate::types::{NodeId, NodeValueType};
use finstack_core::dates::{PeriodId, PeriodKind};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

/// Evaluation context for a single period.
///
/// Tracks node values for the current period and provides access to historical values.
/// Read-only shared data (`node_to_column`, `historical_results`,
/// `historical_capital_structure_cashflows`) is wrapped in `Arc` so that per-period
/// and per-aggregate-function context construction is O(1) instead of O(P×N).
#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Current period being evaluated
    pub period_id: PeriodId,

    /// Period frequency (quarterly, monthly, etc.)
    pub period_kind: PeriodKind,

    /// Map of node_id → column index for the current period
    pub node_to_column: Arc<IndexMap<NodeId, usize>>,

    /// Historical results: period_id → (node_id → value)
    pub historical_results: Arc<IndexMap<PeriodId, IndexMap<String, f64>>>,

    /// Historical capital-structure snapshots: period_id → cashflows
    pub historical_capital_structure_cashflows:
        Arc<IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>>,

    /// Current period results being built.
    /// Uses `Option<f64>` to distinguish between "not yet evaluated" (`None`) and "evaluated to NaN" (`Some(NaN)`).
    pub current_values: Vec<Option<f64>>,

    /// Track value types for each node (monetary vs scalar).
    /// Wrapped in `Arc` so that per-period context copies are O(1).
    pub node_value_types: Arc<IndexMap<String, NodeValueType>>,

    /// Capital structure cashflows (optional)
    pub capital_structure_cashflows: Option<crate::capital_structure::CapitalStructureCashflows>,

    /// Warnings collected while evaluating this period
    pub warnings: Vec<EvalWarning>,

    /// Per-context memoization of sorted historical lookups.
    ///
    /// Within a single period's evaluation many formulas (rolling, expanding,
    /// statistical, time-series) reach back into the same node's history.
    /// Each lookup previously rebuilt a fresh `BTreeMap<PeriodId, f64>` by
    /// iterating `historical_results`. We now cache the constructed map keyed
    /// by node identifier (or capital-structure reference) so repeated calls
    /// in the same period are O(1) refcount bumps instead of O(P) walks.
    ///
    /// The cache is intentionally `RefCell` (not Mutex) — `EvaluationContext`
    /// is per-period and never crosses thread boundaries; Monte Carlo paths
    /// build fresh contexts inside their own thread closures.
    pub(crate) sorted_history_cache: RefCell<IndexMap<String, Rc<BTreeMap<PeriodId, f64>>>>,
}

impl EvaluationContext {
    /// Create a new evaluation context for a period.
    ///
    /// # Arguments
    /// * `period_id` - Period currently being evaluated
    /// * `node_to_column` - Mapping from node identifiers to their column index
    /// * `historical_results` - Prior period results available for lag/lead lookups
    ///
    /// # Example
    ///
    /// ```rust
    /// # use finstack_statements::evaluator::EvaluationContext;
    /// # use finstack_statements::types::NodeId;
    /// # use finstack_core::dates::PeriodId;
    /// # use indexmap::IndexMap;
    /// # use std::sync::Arc;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let period = PeriodId::quarter(2025, 1);
    /// let mut columns = IndexMap::new();
    /// columns.insert(NodeId::new("revenue"), 0);
    /// let ctx = EvaluationContext::new(period, Arc::new(columns), Arc::new(IndexMap::new()));
    /// assert_eq!(ctx.period_id, period);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(
        period_id: PeriodId,
        node_to_column: Arc<IndexMap<NodeId, usize>>,
        historical_results: Arc<IndexMap<PeriodId, IndexMap<String, f64>>>,
    ) -> Self {
        Self::new_with_history(
            period_id,
            node_to_column,
            historical_results,
            Arc::new(IndexMap::new()),
        )
    }

    /// Create a new evaluation context with shared historical results and
    /// capital-structure cashflow snapshots.
    ///
    /// This constructor is used by the evaluator hot path so each per-period
    /// context can reuse the same `Arc`-backed history maps without cloning
    /// their contents.
    pub fn new_with_history(
        period_id: PeriodId,
        node_to_column: Arc<IndexMap<NodeId, usize>>,
        historical_results: Arc<IndexMap<PeriodId, IndexMap<String, f64>>>,
        historical_capital_structure_cashflows: Arc<
            IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>,
        >,
    ) -> Self {
        let num_nodes = node_to_column.len();
        let period_kind = period_id.kind();
        Self {
            period_id,
            period_kind,
            node_to_column,
            historical_results,
            historical_capital_structure_cashflows,
            current_values: vec![None; num_nodes],
            node_value_types: Arc::new(IndexMap::new()),
            capital_structure_cashflows: None,
            warnings: Vec::new(),
            sorted_history_cache: RefCell::new(IndexMap::new()),
        }
    }

    /// Invalidate the sorted-history cache for a single node.
    ///
    /// Call this whenever the current-period value for `node_id` changes
    /// after the cache has been populated — otherwise downstream rolling /
    /// statistical functions can read a stale "current" entry. Cheap when
    /// the cache is empty (the common case during the first pass over a
    /// period's nodes).
    pub(crate) fn invalidate_sorted_cache(&self, node_id: &str) {
        let mut cache = self.sorted_history_cache.borrow_mut();
        cache.shift_remove(node_id);
    }

    /// Set capital structure cashflows for this context.
    ///
    /// Callers typically invoke this when the evaluator loads supplementary
    /// data from the capital structure module.
    pub fn with_capital_structure(
        mut self,
        cashflows: crate::capital_structure::CapitalStructureCashflows,
    ) -> Self {
        self.capital_structure_cashflows = Some(cashflows);
        self
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
        // Drop any cached sorted-history entry whose "current" view depended on
        // the previous value — repeat sets on the same node within a period are
        // rare but they happen (e.g., normalization rewrites), and a stale
        // cache here would silently feed yesterday's value to rolling /
        // statistical functions.
        self.invalidate_sorted_cache(node_id);
        Ok(())
    }

    /// Get the value for a node in the current period.
    ///
    /// Returns an error if the node has not been evaluated yet (value is `None`).
    /// Returns `Ok(NaN)` if the node was evaluated but resulted in `NaN`.
    ///
    /// # Arguments
    /// * `node_id` - Identifier to query
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

    /// Get the value for a node as a [`rust_decimal::Decimal`] boundary value.
    ///
    /// Converts the internally-stored `f64` to `Decimal` at the caller's
    /// boundary. `NaN` and non-finite values become an error rather than
    /// propagating as `Decimal::ZERO`. Use this helper from downstream
    /// accounting / settlement / regulatory-capital code paths that need
    /// the workspace's money invariants (see INVARIANTS.md §1), while the
    /// evaluator's own storage remains `f64` pending a full Decimal
    /// migration.
    ///
    /// A full migration of `current_values` to `Vec<Option<Decimal>>`
    /// would ripple through 100+ formula / check call sites and remains
    /// deferred; callers that need Decimal can opt in explicitly via
    /// this boundary without forcing every downstream consumer to
    /// migrate.
    pub fn get_value_decimal(&self, node_id: &str) -> Result<rust_decimal::Decimal> {
        let value = self.get_value(node_id)?;
        if !value.is_finite() {
            return Err(Error::eval(format!(
                "Cannot convert non-finite value {value} for node '{node_id}' to Decimal \
                 (period {}); the Decimal boundary requires finite inputs",
                self.period_id
            )));
        }
        rust_decimal::Decimal::try_from(value).map_err(|e| {
            Error::eval(format!(
                "Failed to convert f64 value {value} to Decimal for node '{node_id}' \
                 (period {}): {e}",
                self.period_id
            ))
        })
    }

    /// Get historical value for a node at a specific period.
    ///
    /// # Arguments
    /// * `node_id` - Identifier to query
    /// * `period_id` - Historical period to look up
    pub fn get_historical_value(&self, node_id: &str, period_id: &PeriodId) -> Option<f64> {
        self.historical_results
            .get(period_id)
            .and_then(|period_results| period_results.get(node_id).copied())
    }

    fn lookup_cs_value(
        cashflows: &crate::capital_structure::CapitalStructureCashflows,
        period_id: &PeriodId,
        component: &str,
        instrument_or_total: &str,
    ) -> Result<f64> {
        if instrument_or_total == "total" {
            match component {
                "interest_expense" => cashflows.get_total_interest(period_id),
                "interest_expense_cash" => cashflows.get_total_interest_cash(period_id),
                "interest_expense_pik" => cashflows.get_total_interest_pik(period_id),
                "principal_payment" => cashflows.get_total_principal(period_id),
                "debt_balance" => cashflows.get_total_debt_balance(period_id),
                "fees" => cashflows.get_total_fees(period_id),
                "accrued_interest" => cashflows.get_total_accrued_interest(period_id),
                _ => Err(Error::capital_structure(format!(
                    "Unknown capital structure component: {}. Expected: interest_expense, interest_expense_cash, interest_expense_pik, principal_payment, debt_balance, fees, or accrued_interest",
                    component
                ))),
            }
        } else {
            match component {
                "interest_expense" => cashflows.get_interest(instrument_or_total, period_id),
                "interest_expense_cash" => {
                    cashflows.get_interest_cash(instrument_or_total, period_id)
                }
                "interest_expense_pik" => cashflows.get_interest_pik(instrument_or_total, period_id),
                "principal_payment" => cashflows.get_principal(instrument_or_total, period_id),
                "debt_balance" => cashflows.get_debt_balance(instrument_or_total, period_id),
                "fees" => cashflows.get_fees(instrument_or_total, period_id),
                "accrued_interest" => {
                    cashflows.get_accrued_interest(instrument_or_total, period_id)
                }
                _ => Err(Error::capital_structure(format!(
                    "Unknown capital structure component: {}. Expected: interest_expense, interest_expense_cash, interest_expense_pik, principal_payment, debt_balance, fees, or accrued_interest",
                    component
                ))),
            }
        }
    }

    /// Get historical capital-structure value for a specific period.
    pub fn get_historical_cs_value(
        &self,
        component: &str,
        instrument_or_total: &str,
        period_id: &PeriodId,
    ) -> Result<f64> {
        let cashflows = self
            .historical_capital_structure_cashflows
            .get(period_id)
            .ok_or_else(|| {
                Error::capital_structure(format!(
                    "No historical capital structure data for period {}",
                    period_id
                ))
            })?;
        Self::lookup_cs_value(cashflows, period_id, component, instrument_or_total)
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
    /// ```ignore
    /// use finstack_statements::evaluator::EvaluationContext;
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
    pub fn get_cs_value(&self, component: &str, instrument_or_total: &str) -> Result<f64> {
        let cs_cashflows = self
            .capital_structure_cashflows
            .as_ref()
            .ok_or_else(|| Error::capital_structure("No capital structure defined in model"))?;
        Self::lookup_cs_value(
            cs_cashflows,
            &self.period_id,
            component,
            instrument_or_total,
        )
    }

    /// Get all results as a map.
    ///
    /// Only includes nodes that have been evaluated (Some value).
    /// Nodes with None are skipped (should not happen in valid evaluation).
    pub fn into_results(self) -> (IndexMap<String, f64>, Vec<EvalWarning>) {
        let mut results = IndexMap::new();
        for (node_id, idx) in self.node_to_column.iter() {
            if let Some(value) = self.current_values[*idx] {
                results.insert(node_id.as_str().to_string(), value);
            }
        }
        (results, self.warnings)
    }

    /// Record a warning for the current period.
    pub fn push_warning(&mut self, warning: EvalWarning) {
        self.warnings.push(warning);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeId;

    #[test]
    fn test_context_creation() {
        let mut node_to_column = IndexMap::new();
        node_to_column.insert(NodeId::new("revenue"), 0);
        node_to_column.insert(NodeId::new("cogs"), 1);

        let ctx = EvaluationContext::new(
            PeriodId::quarter(2025, 1),
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
            PeriodId::quarter(2025, 1),
            Arc::new(node_to_column),
            Arc::new(IndexMap::new()),
        );

        ctx.set_value("revenue", 100_000.0)
            .expect("test should succeed");
        assert_eq!(
            ctx.get_value("revenue").expect("test should succeed"),
            100_000.0
        );
    }

    #[test]
    fn test_get_historical_value() {
        let mut node_to_column = IndexMap::new();
        node_to_column.insert(NodeId::new("revenue"), 0);

        let mut historical = IndexMap::new();
        let mut q1_results = IndexMap::new();
        q1_results.insert("revenue".to_string(), 100_000.0);
        historical.insert(PeriodId::quarter(2025, 1), q1_results);

        let ctx = EvaluationContext::new(
            PeriodId::quarter(2025, 2),
            Arc::new(node_to_column),
            Arc::new(historical),
        );

        let value = ctx.get_historical_value("revenue", &PeriodId::quarter(2025, 1));
        assert_eq!(value, Some(100_000.0));
    }

    /// `get_value_decimal` provides a Decimal-at-boundary conversion
    /// for callers that need the workspace's money invariants. Finite
    /// f64 values must round-trip cleanly; non-finite values must be
    /// rejected rather than propagating as `Decimal::ZERO`.
    #[test]
    fn test_get_value_decimal_round_trips_finite_value() {
        let mut node_to_column = IndexMap::new();
        node_to_column.insert(NodeId::new("debt"), 0);

        let mut ctx = EvaluationContext::new(
            PeriodId::quarter(2025, 1),
            Arc::new(node_to_column),
            Arc::new(IndexMap::new()),
        );

        ctx.set_value("debt", 1_234_567.89).expect("set ok");
        let d = ctx.get_value_decimal("debt").expect("decimal ok");
        let roundtrip: f64 = d.try_into().expect("f64 ok");
        assert!((roundtrip - 1_234_567.89).abs() < 1e-4);
    }

    #[test]
    fn test_get_value_decimal_rejects_non_finite() {
        let mut node_to_column = IndexMap::new();
        node_to_column.insert(NodeId::new("bad"), 0);

        let mut ctx = EvaluationContext::new(
            PeriodId::quarter(2025, 1),
            Arc::new(node_to_column),
            Arc::new(IndexMap::new()),
        );

        ctx.set_value("bad", f64::NAN).expect("set NaN");
        let err = ctx.get_value_decimal("bad").expect_err("must reject NaN");
        assert!(
            err.to_string().contains("non-finite"),
            "expected non-finite rejection: {err}"
        );
    }
}
