//! Cash balance reconciliation check.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::checks::identity::{
    Comparison, DatedOpening, Term, coverage_finding, read_cells, require_known_distinct,
    signed_sum,
};
use crate::checks::{Check, CheckCategory, CheckContext, CheckResult};
use crate::types::{AmountOrScalar, NodeId};

/// Reconciles closing cash to opening cash plus classified cash flows and
/// separately identified adjustments, and independently reconciles total
/// classified cash flows to CFO + CFI + CFF.
///
/// The roll-forward is evaluated for every model period:
/// `closing = opening + total cash flow + adjustments`. The first opening is
/// the explicit `opening_balance` at the first period boundary; each later
/// opening is the close of the immediately adjacent prior period. Cash
/// inflows are positive and outflows negative. All inputs share one cash
/// scope, compatible units and one currency; the check performs no FX
/// conversion.
///
/// The component identity `total cash flow = CFO + CFI + CFF` runs for every
/// period when all three component nodes are configured, and not at all when
/// none is; configuring only some of them is an error.
///
/// Monetary comparisons are exact and scalar ones compensated. The relative
/// reference of the tolerance is `|opening cash|` for the roll-forward and
/// `|total cash flow|` for the components, with no floor. A comparison with
/// a missing input is incomplete; the check passes only when every enabled
/// comparison is complete and within tolerance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CashReconciliation {
    /// Node for the closing cash balance of each period.
    pub cash_balance_node: NodeId,
    /// Node for total classified cash flow of each period, inflows positive.
    pub total_cash_flow_node: NodeId,
    /// Opening cash at the start of the model's first period.
    pub opening_balance: DatedOpening,
    /// Signed FX and cash-scope adjustment nodes, each included once.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjustment_nodes: Vec<NodeId>,
    /// Node for cash from operations; configure all three components or
    /// none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfo_node: Option<NodeId>,
    /// Node for cash from investing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfi_node: Option<NodeId>,
    /// Node for cash from financing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cff_node: Option<NodeId>,
    /// Absolute tolerance override in the cash units; replaces both terms of
    /// the suite's [`CheckConfig`](crate::checks::CheckConfig) allowance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f64>,
}

impl Check for CashReconciliation {
    fn id(&self) -> &str {
        "cash_reconciliation"
    }

    fn name(&self) -> &str {
        "Cash Reconciliation"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::AccountingIdentity
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let components: Vec<NodeId> = [&self.cfo_node, &self.cfi_node, &self.cff_node]
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        if !components.is_empty() && components.len() < 3 {
            return Err(crate::Error::invalid_input(format!(
                "{}: only {} of the three CFO/CFI/CFF nodes are configured; configure all \
                 three or none",
                self.id(),
                components.len()
            )));
        }
        let flows = [
            self.cash_balance_node.clone(),
            self.total_cash_flow_node.clone(),
        ];
        require_known_distinct(
            self.id(),
            context,
            &[
                ("cash nodes", &flows),
                ("adjustment_nodes", &self.adjustment_nodes),
                ("cash-flow components", &components),
            ],
        )?;
        crate::checks::helpers::validate_identity_inputs(
            self.id(),
            context,
            self.tolerance,
            flows
                .iter()
                .chain(&self.adjustment_nodes)
                .chain(&components),
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
        let mut evaluated = 0_usize;

        for (index, period) in periods.iter().enumerate() {
            let pid = period.id;
            evaluated += 1;
            // Opening cash: the explicit opening, then each adjacent close.
            let mut missing = Vec::new();
            let opening: Option<AmountOrScalar> = match index.checked_sub(1) {
                None => Some(self.opening_balance.value),
                Some(prior) if periods[prior].end == period.start => {
                    match read_cells(
                        self.id(),
                        context,
                        &[&self.cash_balance_node],
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
                    missing.push(self.cash_balance_node.clone());
                    None
                }
            };
            let operands: Vec<&NodeId> = [&self.cash_balance_node, &self.total_cash_flow_node]
                .into_iter()
                .chain(&self.adjustment_nodes)
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
                    "Cash roll-forward",
                    pid,
                    missing,
                ));
                continue;
            };
            let closing = [Term::plus(values[0])];
            let expected: Vec<Term> = std::iter::once(Term::plus(opening))
                .chain(values[1..].iter().copied().map(Term::plus))
                .collect();
            let (comparison, finding) = Comparison {
                check_id: self.id(),
                identity: "cash_roll_forward",
                period: pid,
                actual: &closing,
                expected: &expected,
                reference: opening.value().abs(),
                reference_label: "opening_cash",
                tolerance: self.tolerance,
                nodes: operands.into_iter().cloned().collect(),
            }
            .evaluate(context, |closing, expected, residual| {
                format!(
                    "Cash does not reconcile in {pid}: closing cash ({closing:.2}) != opening \
                     cash + cash flows and adjustments ({expected:.2}), residual = {residual:.2}"
                )
            })?;
            comparisons.push(comparison);
            findings.extend(finding);
        }

        if components.len() == 3 {
            for period in periods {
                let pid = period.id;
                evaluated += 1;
                let operands: Vec<&NodeId> = std::iter::once(&self.total_cash_flow_node)
                    .chain(&components)
                    .collect();
                let values = match read_cells(self.id(), context, &operands, &pid)? {
                    Ok(values) => values,
                    Err(missing) => {
                        findings.push(coverage_finding(
                            self.id(),
                            "Cash flow component reconciliation",
                            pid,
                            missing,
                        ));
                        continue;
                    }
                };
                let total = [Term::plus(values[0])];
                let parts: Vec<Term> = values[1..].iter().copied().map(Term::plus).collect();
                let (comparison, finding) = Comparison {
                    check_id: self.id(),
                    identity: "cash_flow_components",
                    period: pid,
                    actual: &total,
                    expected: &parts,
                    reference: signed_sum(self.id(), &total)?.abs(),
                    reference_label: "total_cash_flow",
                    tolerance: self.tolerance,
                    nodes: operands.into_iter().cloned().collect(),
                }
                .evaluate(context, |total, parts, residual| {
                    format!(
                        "Cash flow components do not sum to the total in {pid}: total \
                         ({total:.2}) != CFO + CFI + CFF ({parts:.2}), residual = {residual:.2}"
                    )
                })?;
                comparisons.push(comparison);
                findings.extend(finding);
            }
        }

        Ok(CheckResult::new(self, evaluated, findings, comparisons))
    }
}
