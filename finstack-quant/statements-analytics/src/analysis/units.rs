//! Shared unit checks for ratios of statement nodes.

use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements::types::NodeValueType;
use finstack_quant_statements::{Error, Result};

/// Reject ratios between different representations or monetary currencies.
///
/// # Arguments
///
/// * `results` - Statement results whose node type metadata determines units.
/// * `numerator` - Numerator node identifier to compare with `denominator`.
/// * `denominator` - Denominator node identifier requiring identical units.
/// * `period` - Reporting period whose typed monetary values also determine
///   units when optional metadata is absent.
pub(super) fn validate_matching_units(
    results: &StatementResult,
    numerator: &str,
    denominator: &str,
    period: &PeriodId,
) -> Result<()> {
    if node_unit_at(results, numerator, period)? != node_unit_at(results, denominator, period)? {
        return Err(Error::invalid_input(format!(
            "Ratio nodes '{numerator}' and '{denominator}' must have matching types and currencies"
        )));
    }
    Ok(())
}

/// Resolve units without allowing metadata or numeric projections to contradict
/// a monetary payload.
///
/// # Arguments
///
/// * `results` - Statement values and optional unit metadata to check.
/// * `node` - Identifier whose amount and declared type must agree.
/// * `period` - Period containing the monetary payload to inspect.
pub(super) fn node_unit_at(
    results: &StatementResult,
    node: &str,
    period: &PeriodId,
) -> Result<Option<NodeValueType>> {
    let declared = results.node_value_types.get(node).copied();
    if let Some(amount) = results.get_money(node, period) {
        if let Some(value) = results.get(node, period) {
            // Evaluated numeric cells may precede currency rounding. Exact
            // typed projections also cover full-precision Decimal amounts.
            if value.partial_cmp(&amount.amount()) != Some(std::cmp::Ordering::Equal) {
                let projected = finstack_quant_core::money::Money::new(value, amount.currency())
                    .map_err(|error| Error::eval(error.to_string()))?;
                if projected.amount().partial_cmp(&amount.amount())
                    != Some(std::cmp::Ordering::Equal)
                {
                    return Err(Error::eval(format!(
                        "Node '{node}' has contradictory numeric and monetary values at {period}"
                    )));
                }
            }
        }
        let actual = NodeValueType::Monetary {
            currency: amount.currency(),
        };
        if declared.is_some_and(|unit| unit != actual) {
            return Err(Error::invalid_input(format!(
                "Node '{node}' unit metadata contradicts its monetary value at {period}"
            )));
        }
        return Ok(Some(actual));
    }
    if matches!(declared, Some(NodeValueType::Monetary { .. }))
        && results.get(node, period).is_some()
    {
        return Err(Error::invalid_input(format!(
            "Monetary node '{node}' is missing its typed amount at {period}"
        )));
    }
    Ok(declared)
}
