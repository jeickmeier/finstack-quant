//! Helpers for reading finite scalar values out of statement cells.

/// A cell's finite number, or `None` when the cell is absent or has no value
/// (missing, masked or unavailable): for output a reader sees, where a cell
/// without a value is a gap or a missing-data error, never `NaN`.
pub(crate) fn cell_value(
    results: &finstack_quant_statements::evaluator::StatementResult,
    node_id: &str,
    period_id: &finstack_quant_core::dates::PeriodId,
) -> Option<f64> {
    cell_float(results, node_id, period_id).filter(|value| value.is_finite())
}

/// The float view analytics computes on: a value cell's number, `NaN` for a
/// cell that is present without a value (missing, masked or unavailable), and
/// `None` when the result has no cell for the node and period.
///
/// Analytics already treats a `NaN` input as invalid or undefined, so a cell
/// without a value keeps that treatment. The typed cell stays authoritative.
pub(crate) fn cell_float(
    results: &finstack_quant_statements::evaluator::StatementResult,
    node_id: &str,
    period_id: &finstack_quant_core::dates::PeriodId,
) -> Option<f64> {
    results
        .get_cell(node_id, period_id)
        .map(|cell| cell.numeric().unwrap_or(f64::NAN))
}
