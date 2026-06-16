//! Built-in check implementations.
//!
//! Provides structural accounting checks (balance sheet, retained earnings,
//! cash reconciliation) and data quality checks (missing values, sign
//! conventions, non-finite detection).

mod balance_sheet;
mod cash_reconciliation;
mod missing_values;
mod non_finite;
mod retained_earnings;
mod sign_convention;

pub use balance_sheet::BalanceSheetArticulation;
pub use cash_reconciliation::CashReconciliation;
pub use missing_values::MissingValueCheck;
pub use non_finite::NonFiniteCheck;
pub use retained_earnings::RetainedEarningsReconciliation;
pub use sign_convention::SignConventionCheck;

use crate::evaluator::StatementResult;
use crate::types::NodeId;
use finstack_quant_core::dates::PeriodId;

/// Look up a single node's value for a given period.
pub(crate) fn get_node_value(
    results: &StatementResult,
    node: &NodeId,
    period: &PeriodId,
) -> Option<f64> {
    results
        .nodes
        .get(node.as_str())
        .and_then(|m| m.get(period).copied())
}

/// Sum several nodes' values for a given period, treating missing values as zero.
pub(crate) fn sum_nodes(results: &StatementResult, nodes: &[NodeId], period: &PeriodId) -> f64 {
    nodes
        .iter()
        .filter_map(|n| get_node_value(results, n, period))
        .sum()
}
