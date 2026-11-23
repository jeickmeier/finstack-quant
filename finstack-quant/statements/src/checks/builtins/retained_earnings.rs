//! Retained earnings reconciliation check.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::checks::identity::{
    Comparison, DatedOpening, Term, coverage_finding, read_cells, require_known_distinct,
};
use crate::checks::{Check, CheckCategory, CheckContext, CheckResult, SignConventionPolicy};
use crate::types::{AmountOrScalar, NodeId};

/// Reconciles retained earnings in every model period:
/// `closing = opening + net income + dividend effect + direct adjustments`.
///
/// The first opening is the explicit `opening_balance` at the first period
/// boundary; each later opening is the close of the immediately adjacent
/// prior period. All inputs share one ownership scope, compatible units and
/// one currency; the check performs no FX conversion.
///
/// # Sign convention
///
/// * `net_income_node` is signed: a profit increases retained earnings and
///   a loss decreases it.
/// * `dividends_node` is, by default
///   ([`SignConventionPolicy::MagnitudePositive`]), a non-negative magnitude
///   that the identity subtracts; a negative magnitude is rejected. Under
///   [`SignConventionPolicy::InflowPositive`] it is a signed amount the
///   identity adds.
/// * `other_adjustments` are signed entries recorded directly in retained
///   earnings, each a distinct node included once. A restatement appears
///   once: in an already-restated opening balance or as one such entry.
///
/// Omit a dividend or adjustment series only when it does not apply; a
/// configured series without a value makes the period coverage_finding, and a
/// known zero is a typed zero. Monetary comparisons are exact and scalar
/// ones compensated. The relative reference of the tolerance is
/// `|opening retained earnings|`, with no floor. The check passes only when
/// every period is complete and within tolerance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct RetainedEarningsReconciliation {
    /// Node for the closing retained-earnings balance of each period.
    pub retained_earnings_node: NodeId,
    /// Node for net income of each period, signed.
    pub net_income_node: NodeId,
    /// Opening retained earnings at the start of the model's first period.
    pub opening_balance: DatedOpening,
    /// Node for dividends recognized in retained earnings in each period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dividends_node: Option<NodeId>,
    /// Signed direct retained-earnings entries, each a distinct node.
    #[serde(default)]
    pub other_adjustments: Vec<NodeId>,
    /// Absolute tolerance override in the balance units; replaces both terms
    /// of the suite's [`CheckConfig`](crate::checks::CheckConfig) allowance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f64>,
    /// Sign convention of `dividends_node`. Defaults to
    /// [`SignConventionPolicy::MagnitudePositive`].
    #[serde(default)]
    pub dividends_sign_convention: SignConventionPolicy,
}

impl Check for RetainedEarningsReconciliation {
    fn id(&self) -> &str {
        "retained_earnings_reconciliation"
    }

    fn name(&self) -> &str {
        "Retained Earnings Reconciliation"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::AccountingIdentity
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let core = [
            self.retained_earnings_node.clone(),
            self.net_income_node.clone(),
        ];
        let dividends: Vec<NodeId> = self.dividends_node.iter().cloned().collect();
        require_known_distinct(
            self.id(),
            context,
            &[
                ("retained earnings nodes", &core),
                ("dividends_node", &dividends),
                ("other_adjustments", &self.other_adjustments),
            ],
        )?;
        crate::checks::lookup::validate_identity_inputs(
            self.id(),
            context,
            self.tolerance,
            core.iter().chain(&dividends).chain(&self.other_adjustments),
        )?;
        let periods = context.periods();
        if let Some(first) = periods.first()
            && self.opening_balance.date != first.start
        {
            return Err(crate::Error::invalid_input(format!(
                "{}: opening_balance is dated {} but the first period starts on {}",
                self.id(),
                self.opening_balance.date,
                first.start
            )));
        }

        let mut findings = Vec::new();
        let mut comparisons = Vec::new();

        for (index, period) in periods.iter().enumerate() {
            let pid = period.id;
            let mut missing = Vec::new();
            let opening: Option<AmountOrScalar> = match index.checked_sub(1) {
                None => Some(self.opening_balance.value),
                Some(prior) if periods[prior].end == period.start => {
                    match read_cells(
                        self.id(),
                        context,
                        &[&self.retained_earnings_node],
                        &periods[prior].id,
                    )? {
                        Ok(values) => Some(values[0]),
                        Err(nodes) => {
                            missing.extend(nodes);
                            None
                        }
                    }
                }
                // No adjacent prior close: this period has no opening.
                Some(_) => {
                    missing.push(self.retained_earnings_node.clone());
                    None
                }
            };
            let operands: Vec<&NodeId> = core
                .iter()
                .chain(&dividends)
                .chain(&self.other_adjustments)
                .collect();
            let values = match read_cells(self.id(), context, &operands, &pid)? {
                Ok(values) => Some(values),
                Err(nodes) => {
                    missing.extend(nodes);
                    None
                }
            };
            let (Some(opening), Some(values)) = (opening, values) else {
                findings.push(coverage_finding(
                    self.id(),
                    "Retained earnings reconciliation",
                    pid,
                    missing,
                ));
                continue;
            };

            let closing = [Term::plus(values[0])];
            let mut expected = vec![Term::plus(opening), Term::plus(values[1])];
            let mut rest = values[2..].iter().copied();
            if let Some(node) = &self.dividends_node {
                let Some(dividend) = rest.next() else {
                    return Err(crate::Error::eval(format!(
                        "{}: dividend operand was not read",
                        self.id()
                    )));
                };
                expected.push(match self.dividends_sign_convention {
                    SignConventionPolicy::MagnitudePositive => {
                        if dividend.value() < 0.0 {
                            return Err(crate::Error::invalid_input(format!(
                                "{}: dividends '{node}' are {} in {pid}; a dividend magnitude \
                                 cannot be negative. Select the inflow_positive convention for \
                                 signed dividends",
                                self.id(),
                                dividend.value()
                            )));
                        }
                        Term::minus(dividend)
                    }
                    SignConventionPolicy::InflowPositive => Term::plus(dividend),
                });
            }
            expected.extend(rest.map(Term::plus));

            let (comparison, finding) = Comparison {
                check_id: self.id(),
                identity: "retained_earnings_roll_forward",
                period: pid,
                actual: &closing,
                expected: &expected,
                reference: opening.value().abs(),
                reference_label: "opening_retained_earnings",
                tolerance: self.tolerance,
                nodes: operands.into_iter().cloned().collect(),
            }
            .evaluate(context, |closing, expected, residual| {
                format!(
                    "Retained earnings do not reconcile in {pid}: closing ({closing:.2}) != \
                     opening + net income + dividends and adjustments ({expected:.2}), residual \
                     = {residual:.2}"
                )
            })?;
            comparisons.push(comparison);
            findings.extend(finding);
        }

        let evaluated = periods.len();
        Ok(CheckResult::new(self, evaluated, findings, comparisons))
    }
}
