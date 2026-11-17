//! Shared machinery of the accounting-identity checks.
//!
//! An identity compares a typed actual against a typed expected value, each a
//! signed sum of period cells. Monetary sums are exact (`Money` arithmetic in
//! one currency); scalar sums are compensated. A period with a missing
//! required cell is incomplete, never skipped or read as zero, and an invalid
//! input or an unrepresentable sum is an error.

use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::money::Money;
use serde::{Deserialize, Serialize};

use super::types::effective_tolerance;
use super::{CheckComparison, CheckContext, CheckFinding, Materiality, Severity};
use crate::Result;
use crate::types::{AmountOrScalar, NodeId};

/// An explicit opening balance at the first period boundary of a
/// roll-forward identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DatedOpening {
    /// Date of the balance: the start of the model's first period.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// The balance on that date, in the units and currency of the
    /// reconciled balance node. A known zero is a typed zero.
    pub value: AmountOrScalar,
}

/// One signed term of an identity.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Term {
    /// `+1.0` when the term is added, `-1.0` when subtracted.
    pub sign: f64,
    /// The typed value.
    pub value: AmountOrScalar,
}

impl Term {
    pub(crate) fn plus(value: AmountOrScalar) -> Self {
        Self { sign: 1.0, value }
    }
    pub(crate) fn minus(value: AmountOrScalar) -> Self {
        Self { sign: -1.0, value }
    }
    fn negated(self) -> Self {
        Self {
            sign: -self.sign,
            value: self.value,
        }
    }
}

/// Reject node groups with a repeated identifier, an identifier in two
/// groups, or an identifier neither the model nor the results know.
pub(crate) fn require_known_distinct(
    check_id: &str,
    context: &CheckContext<'_>,
    groups: &[(&str, &[NodeId])],
) -> Result<()> {
    let mut seen: indexmap::IndexMap<&str, &str> = indexmap::IndexMap::new();
    for (label, nodes) in groups {
        for node in *nodes {
            if !context.model.has_node(node.as_str()) && !context.results.has_node(node.as_str()) {
                return Err(crate::Error::invalid_input(format!(
                    "{check_id}: '{node}' in '{label}' is not a node of the model"
                )));
            }
            if let Some(first) = seen.insert(node.as_str(), label) {
                return Err(crate::Error::invalid_input(if first == *label {
                    format!("{check_id}: '{node}' appears twice in '{label}'")
                } else {
                    format!("{check_id}: '{node}' appears in both '{first}' and '{label}'")
                }));
            }
        }
    }
    Ok(())
}

/// Read the cells of `nodes` in `period`: the typed values when every cell
/// has one, else the nodes without a value (missing, masked or unavailable).
///
/// # Errors
///
/// Returns an invalid-input error for a non-finite scalar, which is not a
/// value an identity can use.
pub(crate) fn read_cells(
    check_id: &str,
    context: &CheckContext<'_>,
    nodes: &[&NodeId],
    period: &PeriodId,
) -> Result<std::result::Result<Vec<AmountOrScalar>, Vec<NodeId>>> {
    let mut values = Vec::with_capacity(nodes.len());
    let mut missing = Vec::new();
    for node in nodes {
        match context
            .results
            .get_cell(node.as_str(), period)
            .and_then(|cell| cell.value)
        {
            Some(value) if value.value().is_finite() => values.push(value),
            Some(value) => {
                return Err(crate::Error::invalid_input(format!(
                    "{check_id}: '{node}' is not finite in {period} ({})",
                    value.value()
                )));
            }
            None => missing.push((*node).clone()),
        }
    }
    Ok(if missing.is_empty() {
        Ok(values)
    } else {
        Err(missing)
    })
}

/// The exact signed sum of `terms`, as its float view.
///
/// Money in one currency is summed exactly; scalars with compensated
/// summation.
///
/// # Errors
///
/// Returns an invalid-input error for mixed units or currencies and an
/// evaluation error for a sum that cannot be represented.
pub(crate) fn signed_sum(check_id: &str, terms: &[Term]) -> Result<f64> {
    let unrepresentable = || {
        crate::Error::eval(format!(
            "{check_id}: the identity's sum cannot be represented"
        ))
    };
    let Some(first) = terms.first() else {
        return Ok(0.0);
    };
    if let Some(money) = first.value.as_money() {
        let mut total = Money::new(0.0, money.currency())?;
        for term in terms {
            let Some(amount) = term.value.as_money() else {
                return Err(crate::Error::invalid_input(format!(
                    "{check_id}: the identity mixes monetary and scalar values"
                )));
            };
            total = if term.sign < 0.0 {
                total.checked_sub(amount)
            } else {
                total.checked_add(amount)
            }
            .map_err(|error| crate::Error::invalid_input(format!("{check_id}: {error}")))?;
        }
        let view = total.amount();
        return view.is_finite().then_some(view).ok_or_else(unrepresentable);
    }
    if terms.iter().any(|term| term.value.is_amount()) {
        return Err(crate::Error::invalid_input(format!(
            "{check_id}: the identity mixes monetary and scalar values"
        )));
    }
    let total = neumaier_sum(terms.iter().map(|term| term.sign * term.value.value()))
        .map_err(|_| unrepresentable())?;
    total
        .is_finite()
        .then_some(total)
        .ok_or_else(unrepresentable)
}

/// One evaluated comparison of an identity.
pub(crate) struct Comparison<'a> {
    /// Identifier of the owning check.
    pub check_id: &'a str,
    /// Stable snake_case name of the identity.
    pub identity: &'a str,
    /// Period the comparison belongs to.
    pub period: PeriodId,
    /// Terms of the observed side.
    pub actual: &'a [Term],
    /// Terms of the side the identity requires.
    pub expected: &'a [Term],
    /// The identity owner's reference for the relative tolerance and the
    /// relative residual, with no floor.
    pub reference: f64,
    /// Label of the reference in the materiality context.
    pub reference_label: &'a str,
    /// Per-check absolute tolerance override.
    pub tolerance: Option<f64>,
    /// Nodes attached to a failure finding.
    pub nodes: Vec<NodeId>,
}

impl Comparison<'_> {
    /// Compare the two sides: the recorded comparison, and an error finding
    /// when the absolute residual exceeds the tolerance.
    ///
    /// The residual `actual - expected` is one exact signed sum over all
    /// terms, not a difference of two rounded totals.
    pub(crate) fn evaluate(
        self,
        context: &CheckContext<'_>,
        describe: impl FnOnce(f64, f64, f64) -> String,
    ) -> Result<(CheckComparison, Option<CheckFinding>)> {
        let actual = signed_sum(self.check_id, self.actual)?;
        let expected = signed_sum(self.check_id, self.expected)?;
        let all: Vec<Term> = self
            .actual
            .iter()
            .copied()
            .chain(self.expected.iter().map(|term| term.negated()))
            .collect();
        let residual = signed_sum(self.check_id, &all)?;
        let tolerance = effective_tolerance(&context.config, self.tolerance, self.reference);
        if !tolerance.is_finite() {
            return Err(crate::Error::eval(format!(
                "{}: the tolerance cannot be represented",
                self.check_id
            )));
        }
        let comparison = CheckComparison {
            identity: self.identity.to_string(),
            period: Some(self.period),
            actual,
            expected,
            tolerance,
        };
        if residual.abs() <= tolerance {
            return Ok((comparison, None));
        }
        // The relative residual is reported when the reference is non-zero
        // and the percentage is representable; zero otherwise.
        let relative_pct = Some((residual / self.reference).abs() * 100.0)
            .filter(|pct| self.reference != 0.0 && pct.is_finite())
            .unwrap_or(0.0);
        let finding = CheckFinding {
            check_id: self.check_id.to_string(),
            severity: Severity::Error,
            message: describe(actual, expected, residual),
            period: Some(self.period),
            materiality: Some(Materiality {
                absolute: residual.abs(),
                relative_pct,
                reference_value: self.reference,
                reference_label: self.reference_label.to_string(),
            }),
            nodes: self.nodes,
            comparison: Some(comparison.clone()),
            coverage: false,
        };
        Ok((comparison, Some(finding)))
    }
}

/// A coverage finding: the comparison of `period` could not be evaluated
/// because the listed inputs have no value.
pub(crate) fn incomplete(
    check_id: &str,
    what: &str,
    period: PeriodId,
    missing: Vec<NodeId>,
) -> CheckFinding {
    let names: Vec<&str> = missing.iter().map(NodeId::as_str).collect();
    CheckFinding {
        check_id: check_id.to_string(),
        severity: Severity::Warning,
        message: format!(
            "{what} is incomplete for {period}: no value for [{}]",
            names.join(", ")
        ),
        period: Some(period),
        materiality: None,
        nodes: missing,
        comparison: None,
        coverage: true,
    }
}
