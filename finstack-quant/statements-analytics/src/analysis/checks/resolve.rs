//! Required-cell lookup and currency-inference helpers for statement checks.

/// A required input of `check` in one period: its finite value, or `None`
/// after recording a coverage finding, which makes the check incomplete
/// instead of letting the period pass unexamined.
pub(crate) fn required(
    context: &finstack_quant_statements::checks::CheckContext<'_>,
    check: &dyn finstack_quant_statements::checks::Check,
    node: &finstack_quant_statements::types::NodeId,
    period: &finstack_quant_core::dates::PeriodId,
    findings: &mut Vec<finstack_quant_statements::checks::CheckFinding>,
) -> Option<f64> {
    let value =
        finstack_quant_statements::checks::get_finite_node_value(context.results, node, period);
    if value.is_none() {
        findings.push(finstack_quant_statements::checks::coverage_finding(
            check.id(),
            check.name(),
            *period,
            vec![node.clone()],
        ));
    }
    value
}

/// The currency of `node`'s value in `period`: `None` for a scalar or a cell
/// without a value. A comparison is recorded in the units of its left side.
pub(crate) fn currency_of(
    context: &finstack_quant_statements::checks::CheckContext<'_>,
    node: &finstack_quant_statements::types::NodeId,
    period: &finstack_quant_core::dates::PeriodId,
) -> Option<finstack_quant_core::currency::Currency> {
    context
        .results
        .get_cell(node.as_str(), period)
        .and_then(|cell| cell.value)
        .and_then(|value| value.as_money())
        .map(|money| money.currency())
}
