//! Domain-level validation checks for financial statement models.
//!
//! These checks go beyond the core accounting-identity and data-quality checks
//! in [`finstack_quant_statements::checks::builtins`] and test cross-statement
//! reconciliation, internal consistency, and credit reasonableness.
//!
//! Higher-level conveniences:
//!
//! - [`CreditMapping`] and [`ThreeStatementMapping`] — typed node-id mappings for common model patterns
//! - [`three_statement_checks`], [`credit_underwriting_checks`], and [`lbo_model_checks`] — pre-built check suites
//! - [`render_check_report_text`] and [`render_check_report_html`] — render [`finstack_quant_statements::checks::CheckReport`] as
//!   text or HTML

pub(crate) mod consistency;
pub(crate) mod credit;
pub(crate) mod mappings;
pub(crate) mod reconciliation;
pub(crate) mod renderer;
pub(crate) mod suites;

pub use consistency::{EffectiveTaxRateCheck, GrowthRateConsistency, WorkingCapitalConsistency};
pub use credit::{
    CoverageFloorCheck, FcfSignCheck, LeverageRangeCheck, LiquidityRunwayCheck, TrendCheck,
    TrendDirection,
};
pub use reconciliation::{
    CapexReconciliation, DepreciationReconciliation, DividendReconciliation,
    InterestExpenseReconciliation,
};

pub use mappings::{CreditMapping, ThreeStatementMapping};
pub use renderer::{render_check_report_html, render_check_report_text};
pub use suites::{credit_underwriting_checks, lbo_model_checks, three_statement_checks};

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
