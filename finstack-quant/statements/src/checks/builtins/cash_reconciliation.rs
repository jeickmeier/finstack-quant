//! Cash balance reconciliation check.

use serde::{Deserialize, Serialize};

use super::get_node_value;
use crate::checks::types::effective_tolerance;
use crate::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
};
use crate::types::NodeId;
use crate::Result;

/// Checks Cash(t) = Cash(t−1) + TotalCF(t) and optionally
/// TotalCF = CFO + CFI + CFF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashReconciliation {
    /// Node for cash balance.
    pub cash_balance_node: NodeId,
    /// Node for total cash flow.
    pub total_cash_flow_node: NodeId,
    /// Optional node for cash from operations.
    pub cfo_node: Option<NodeId>,
    /// Optional node for cash from investing.
    pub cfi_node: Option<NodeId>,
    /// Optional node for cash from financing.
    pub cff_node: Option<NodeId>,
    /// Tolerance override; falls back to
    /// [`CheckConfig::default_tolerance`](crate::checks::CheckConfig::default_tolerance).
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
        let mut findings = Vec::new();
        let periods = &context.model.periods;

        for i in 1..periods.len() {
            let prev_pid = &periods[i - 1].id;
            let curr_pid = &periods[i].id;

            // Roll-forward identities only hold for *strictly* chronologically
            // consecutive periods. Skip any pair whose starts are not strictly
            // ascending — out-of-order or equal-start (e.g. interleaved annual +
            // quarterly) — so a misordered list cannot reconcile the current
            // period against the wrong "prior" period. Emit an Info finding so
            // the skip is visible rather than silent.
            if periods[i].start <= periods[i - 1].start {
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Info,
                    message: format!(
                        "Cash reconciliation skipped for {curr_pid}: period starts are not \
                         strictly ascending relative to prior {prev_pid}; the roll-forward \
                         identity is only evaluated for chronologically consecutive periods."
                    ),
                    period: Some(*curr_pid),
                    materiality: None,
                    nodes: vec![
                        self.cash_balance_node.clone(),
                        self.total_cash_flow_node.clone(),
                    ],
                });
                continue;
            }

            let cash_prev = get_node_value(context.results, &self.cash_balance_node, prev_pid);
            let cash_curr = get_node_value(context.results, &self.cash_balance_node, curr_pid);
            let total_cf = get_node_value(context.results, &self.total_cash_flow_node, curr_pid);

            let (cash_prev, cash_curr, total_cf) = match (cash_prev, cash_curr, total_cf) {
                (Some(p), Some(c), Some(t)) => (p, c, t),
                (p, c, t) => {
                    let mut missing: Vec<String> = Vec::new();
                    if p.is_none() {
                        missing.push(format!("{} @ {prev_pid}", self.cash_balance_node));
                    }
                    if c.is_none() {
                        missing.push(format!("{} @ {curr_pid}", self.cash_balance_node));
                    }
                    if t.is_none() {
                        missing.push(format!("{} @ {curr_pid}", self.total_cash_flow_node));
                    }
                    findings.push(CheckFinding {
                        check_id: self.id().to_string(),
                        severity: Severity::Warning,
                        message: format!(
                            "Cash reconciliation skipped for {curr_pid}: missing inputs [{}]. \
                             The identity cannot be evaluated without all three values.",
                            missing.join(", ")
                        ),
                        period: Some(*curr_pid),
                        materiality: None,
                        nodes: vec![
                            self.cash_balance_node.clone(),
                            self.total_cash_flow_node.clone(),
                        ],
                    });
                    continue;
                }
            };

            let expected_cash = cash_prev + total_cf;
            let diff = cash_curr - expected_cash;
            let reference = cash_prev.abs().max(1.0);
            let tolerance = effective_tolerance(&context.config, self.tolerance, reference);

            if diff.abs() > tolerance {
                let relative = (diff / reference).abs() * 100.0;

                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "Cash does not reconcile in {curr_pid}: \
                         actual ({cash_curr:.2}) != prior ({cash_prev:.2}) + \
                         total CF ({total_cf:.2}), difference = {diff:.2}"
                    ),
                    period: Some(*curr_pid),
                    materiality: Some(Materiality {
                        absolute: diff.abs(),
                        relative_pct: relative,
                        reference_value: cash_prev,
                        reference_label: "prior_cash_balance".to_string(),
                    }),
                    nodes: vec![
                        self.cash_balance_node.clone(),
                        self.total_cash_flow_node.clone(),
                    ],
                });
            }

            // Component check: CFO + CFI + CFF = TotalCF
            if let (Some(cfo_node), Some(cfi_node), Some(cff_node)) =
                (&self.cfo_node, &self.cfi_node, &self.cff_node)
            {
                let cfo = get_node_value(context.results, cfo_node, curr_pid);
                let cfi = get_node_value(context.results, cfi_node, curr_pid);
                let cff = get_node_value(context.results, cff_node, curr_pid);

                if let (Some(cfo_val), Some(cfi_val), Some(cff_val)) = (cfo, cfi, cff) {
                    let component_sum = cfo_val + cfi_val + cff_val;
                    let component_diff = total_cf - component_sum;
                    let reference = total_cf.abs().max(1.0);
                    let component_tolerance =
                        effective_tolerance(&context.config, self.tolerance, reference);

                    if component_diff.abs() > component_tolerance {
                        let relative = (component_diff / reference).abs() * 100.0;

                        findings.push(CheckFinding {
                            check_id: self.id().to_string(),
                            severity: Severity::Error,
                            message: format!(
                                "Cash flow components do not sum to total in {curr_pid}: \
                                 CFO ({cfo_val:.2}) + CFI ({cfi_val:.2}) + CFF ({cff_val:.2}) \
                                 = {component_sum:.2} != total CF ({total_cf:.2}), \
                                 difference = {component_diff:.2}"
                            ),
                            period: Some(*curr_pid),
                            materiality: Some(Materiality {
                                absolute: component_diff.abs(),
                                relative_pct: relative,
                                reference_value: total_cf,
                                reference_label: "total_cash_flow".to_string(),
                            }),
                            nodes: vec![
                                self.total_cash_flow_node.clone(),
                                cfo_node.clone(),
                                cfi_node.clone(),
                                cff_node.clone(),
                            ],
                        });
                    }
                }
            }
        }

        let passed = !findings.iter().any(|f| f.severity == Severity::Error);

        Ok(CheckResult {
            check_id: self.id().to_string(),
            check_name: self.name().to_string(),
            category: self.category(),
            passed,
            findings,
        })
    }
}
