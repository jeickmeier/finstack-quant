//! Node-value lookup helpers shared by check implementations.
//!
//! Checks in this crate and in downstream crates (for example
//! `finstack-quant-statements-analytics`) read evaluated node values out of a
//! [`StatementResult`] the same way; these helpers are the single place that
//! lookup — and its NaN/Inf policy — lives.

use crate::evaluator::StatementResult;
use crate::types::{NodeId, NodeValueType};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::PeriodId;
use indexmap::IndexMap;

/// Resolve one check operand's units and reject stale model/result metadata.
pub(crate) fn validated_input_value_type(
    check_id: &str,
    context: &super::CheckContext<'_>,
    node: &str,
) -> crate::Result<NodeValueType> {
    let model_type = match context.model.get_node(node) {
        Some(spec) => {
            let inferred = crate::types::validated_explicit_value_type(spec)?;
            spec.value_type.or(inferred)
        }
        None => None,
    };
    let result_type = context.results.node_value_types.get(node).copied();
    if let (Some(expected), Some(actual)) = (model_type, result_type) {
        if expected != actual {
            return Err(crate::Error::invalid_input(format!(
                "{check_id}: input '{node}' has result units {actual:?} but model units are \
                 {expected:?}; supply results for this model"
            )));
        }
    }
    Ok(result_type.or(model_type).unwrap_or(NodeValueType::Scalar))
}

/// Resolve total and native instrument currencies from checked model/result data.
pub(crate) fn validated_capital_structure_currencies(
    check_id: &str,
    context: &super::CheckContext<'_>,
) -> crate::Result<(Option<Currency>, IndexMap<String, Currency>)> {
    let mut reporting_currency = context.model.capital_structure_currency()?;
    let mut instrument_currencies = context.model.capital_structure_instrument_currencies()?;
    if let Some(cashflows) = context.results.cs_cashflows.as_ref() {
        if let Some(actual) = cashflows.reporting_currency {
            reporting_currency = Some(reconcile_currency(
                check_id,
                "capital-structure",
                reporting_currency,
                actual,
            )?);
        }
        for breakdown in cashflows.totals.values() {
            breakdown.validate_currency_invariant()?;
            reporting_currency = Some(reconcile_currency(
                check_id,
                "capital-structure total",
                reporting_currency,
                breakdown.interest_expense_cash.currency(),
            )?);
        }
        for (instrument, periods) in &cashflows.by_instrument {
            for breakdown in periods.values() {
                breakdown.validate_currency_invariant()?;
                let currency = reconcile_currency(
                    check_id,
                    &format!("capital-structure instrument '{instrument}'"),
                    instrument_currencies.get(instrument).copied(),
                    breakdown.interest_expense_cash.currency(),
                )?;
                instrument_currencies.insert(instrument.clone(), currency);
            }
        }
    }
    Ok((reporting_currency, instrument_currencies))
}

fn reconcile_currency(
    check_id: &str,
    label: &str,
    expected: Option<Currency>,
    actual: Currency,
) -> crate::Result<Currency> {
    if let Some(expected) = expected.filter(|expected| *expected != actual) {
        return Err(crate::Error::invalid_input(format!(
            "{check_id}: {label} result currency {actual} disagrees with expected \
             {expected}; supply consistent results for this model"
        )));
    }
    Ok(actual)
}

/// Validate numeric policies and compatible units before accounting arithmetic.
pub(crate) fn validate_identity_inputs<'a>(
    check_id: &str,
    context: &super::CheckContext<'_>,
    tolerance: Option<f64>,
    nodes: impl IntoIterator<Item = &'a NodeId>,
) -> crate::Result<()> {
    context.config.validate()?;
    if let Some(tolerance) = tolerance {
        super::types::validate_nonnegative_finite("Identity-check tolerance", tolerance)?;
    }
    let mut reference = None;
    for node in nodes {
        // Missing nodes keep the check's existing missing-input diagnostics.
        if !context.results.nodes.contains_key(node.as_str()) {
            continue;
        }
        let value_type = validated_input_value_type(check_id, context, node.as_str())?;
        if let Some((reference_node, reference_type)) = reference {
            if reference_type != value_type {
                return Err(crate::Error::invalid_input(format!(
                    "{check_id}: inputs '{reference_node}' ({reference_type:?}) and '{node}' \
                     ({value_type:?}) have incompatible units; convert to one currency before reconciliation"
                )));
            }
        } else {
            reference = Some((node, value_type));
        }
    }
    Ok(())
}

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
