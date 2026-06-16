//! Domain-level validation checks for financial statement models.
//!
//! These checks go beyond the core accounting-identity and data-quality checks
//! in [`finstack_quant_statements::checks::builtins`] and test cross-statement
//! reconciliation, internal consistency, and credit reasonableness.
//!
//! Higher-level conveniences:
//!
//! - [`FormulaCheck`] — user-defined expression checks evaluated per period
//! - [`CreditMapping`] and [`ThreeStatementMapping`] — typed node-id mappings for common model patterns
//! - [`three_statement_checks`], [`credit_underwriting_checks`], and [`lbo_model_checks`] — pre-built check suites
//! - [`corkscrew_as_checks`] — convert corkscrew configs into structural checks
//! - [`CheckReportRenderer`] — render [`finstack_quant_statements::checks::CheckReport`] as
//!   text or HTML

pub(crate) mod consistency;
pub(crate) mod corkscrew_adapter;
pub(crate) mod credit;
pub(crate) mod formula_check;
pub(crate) mod mappings;
pub(crate) mod reconciliation;
pub(crate) mod renderer;
pub(crate) mod suites;

// Re-export all check structs at the `checks` level.

pub use consistency::{EffectiveTaxRateCheck, GrowthRateConsistency, WorkingCapitalConsistency};
pub use credit::{
    CoverageFloorCheck, FcfSignCheck, LeverageRangeCheck, LiquidityRunwayCheck, TrendCheck,
    TrendDirection,
};
pub use reconciliation::{
    CapexReconciliation, DepreciationReconciliation, DividendReconciliation,
    InterestExpenseReconciliation,
};

pub use corkscrew_adapter::corkscrew_as_checks;
pub use formula_check::FormulaCheck;
pub use mappings::{CreditMapping, ThreeStatementMapping};
pub use renderer::CheckReportRenderer;
pub use suites::{
    credit_underwriting_checks, lbo_model_checks, resolve_check_suite, three_statement_checks,
};

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::NodeId;

/// Look up a single node's value for a given period.
fn get_node_value(results: &StatementResult, node: &NodeId, period: &PeriodId) -> Option<f64> {
    results
        .nodes
        .get(node.as_str())
        .and_then(|m| m.get(period).copied())
}

/// Sum several nodes' values for a given period, treating missing values as zero.
fn sum_nodes(results: &StatementResult, nodes: &[NodeId], period: &PeriodId) -> f64 {
    nodes
        .iter()
        .filter_map(|n| get_node_value(results, n, period))
        .sum()
}
