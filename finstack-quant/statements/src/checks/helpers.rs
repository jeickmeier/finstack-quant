//! Node-value lookup helpers shared by check implementations.
//!
//! Checks in this crate and in downstream crates (for example
//! `finstack-quant-statements-analytics`) read evaluated node values out of a
//! [`StatementResult`] the same way; these helpers are the single place that
//! lookup — and its NaN/Inf policy — lives.

use crate::evaluator::StatementResult;
use crate::types::NodeId;
use finstack_quant_core::dates::PeriodId;

/// Look up a single node's value for a given period.
///
/// # Arguments
///
/// * `results` - Evaluated statement results to read from.
/// * `node` - Identifier of the node whose value is requested.
/// * `period` - Period to read; `None` is returned when the node or the
///   period has no evaluated value.
pub fn get_node_value(results: &StatementResult, node: &NodeId, period: &PeriodId) -> Option<f64> {
    results
        .nodes
        .get(node.as_str())
        .and_then(|m| m.get(period).copied())
}

/// Look up a node value that can participate in an accounting identity:
/// present **and** finite.
///
/// A NaN/Inf operand poisons the identity arithmetic — the diff becomes NaN
/// and `NaN > tolerance` is `false`, so a genuinely broken statement would
/// silently pass — exactly the fail-open a missing operand causes by summing
/// to zero. The skip-with-warning guards therefore treat both the same way.
///
/// # Arguments
///
/// * `results` - Evaluated statement results to read from.
/// * `node` - Identifier of the node whose value is requested.
/// * `period` - Period to read; `None` is returned when the value is missing
///   or non-finite.
pub fn get_finite_node_value(
    results: &StatementResult,
    node: &NodeId,
    period: &PeriodId,
) -> Option<f64> {
    get_node_value(results, node, period).filter(|v| v.is_finite())
}

/// Sum several nodes' values for a given period, treating missing values as zero.
///
/// # Arguments
///
/// * `results` - Evaluated statement results to read from.
/// * `nodes` - Node identifiers to sum; nodes without a value for `period`
///   contribute zero.
/// * `period` - Reporting-period identifier used to select each node's value from the evaluated results.
pub fn sum_nodes(results: &StatementResult, nodes: &[NodeId], period: &PeriodId) -> f64 {
    nodes
        .iter()
        .filter_map(|n| get_node_value(results, n, period))
        .sum()
}
