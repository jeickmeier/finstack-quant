//! Retained earnings reconciliation check.

use serde::{Deserialize, Serialize};

use super::{get_node_value, sum_nodes};
use crate::checks::types::effective_tolerance;
use crate::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
    SignConventionPolicy,
};
use crate::types::NodeId;
use crate::Result;

/// Checks that retained earnings flow correctly across periods:
/// RE(t) = RE(t−1) + NI(t) − Dividends(t) ± Adjustments(t).
///
/// # Sign convention
///
/// * `net_income_node` carries the [`SignConventionPolicy::InflowPositive`]
///   convention — positive values increase RE (profit), negative values
///   decrease it (loss). No validation is emitted for NI since both signs
///   are meaningful.
/// * `dividends_node` carries the [`SignConventionPolicy::MagnitudePositive`]
///   convention by default — the formula subtracts dividends explicitly.
/// * `other_adjustments` carry
///   [`SignConventionPolicy::InflowPositive`] — they are signed amounts
///   added directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetainedEarningsReconciliation {
    /// Node for retained earnings balance.
    pub retained_earnings_node: NodeId,
    /// Node for net income.
    pub net_income_node: NodeId,
    /// Optional node for dividends paid.
    pub dividends_node: Option<NodeId>,
    /// Additional adjustment nodes (buybacks, AOCI, etc.).
    pub other_adjustments: Vec<NodeId>,
    /// Tolerance override; falls back to
    /// [`CheckConfig::default_tolerance`](crate::checks::CheckConfig::default_tolerance).
    pub tolerance: Option<f64>,
    /// Sign convention applied to the `dividends_node` input. Defaults
    /// to [`SignConventionPolicy::MagnitudePositive`].
    /// `net_income_node` and `other_adjustments` always use
    /// `InflowPositive` by construction.
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
        let mut findings = Vec::new();
        let periods = &context.model.periods;

        for i in 1..periods.len() {
            let prev_pid = &periods[i - 1].id;
            let curr_pid = &periods[i].id;

            // Roll-forward identities only hold for *adjacent* periods (half-open
            // `[start, end)`, so `prior.end == current.start`). Skip any pair
            // that is not adjacent — non-ascending/equal-start (interleaved
            // annual + quarterly), a gap (a period is missing between them), or
            // an overlap (a year-end reconciled against a quarter it already
            // contains) — so the identity is never evaluated across the wrong
            // span. Emit an Info finding so the skip is visible rather than
            // silent.
            if periods[i - 1].end != periods[i].start {
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Info,
                    message: format!(
                        "Retained earnings reconciliation skipped for {curr_pid}: it is not \
                         adjacent to prior {prev_pid} (gap, overlap, or out-of-order); the \
                         roll-forward identity is only evaluated for consecutive periods."
                    ),
                    period: Some(*curr_pid),
                    materiality: None,
                    nodes: vec![
                        self.retained_earnings_node.clone(),
                        self.net_income_node.clone(),
                    ],
                });
                continue;
            }

            let re_prev = get_node_value(context.results, &self.retained_earnings_node, prev_pid);
            let re_curr = get_node_value(context.results, &self.retained_earnings_node, curr_pid);
            let ni = get_node_value(context.results, &self.net_income_node, curr_pid);

            let (re_prev, re_curr, ni) = match (re_prev, re_curr, ni) {
                (Some(p), Some(c), Some(n)) => (p, c, n),
                (p, c, n) => {
                    let mut missing: Vec<String> = Vec::new();
                    if p.is_none() {
                        missing.push(format!("{} @ {prev_pid}", self.retained_earnings_node));
                    }
                    if c.is_none() {
                        missing.push(format!("{} @ {curr_pid}", self.retained_earnings_node));
                    }
                    if n.is_none() {
                        missing.push(format!("{} @ {curr_pid}", self.net_income_node));
                    }
                    findings.push(CheckFinding {
                        check_id: self.id().to_string(),
                        severity: Severity::Warning,
                        message: format!(
                            "Retained earnings reconciliation skipped for {curr_pid}: \
                             missing inputs [{}]. The identity cannot be evaluated without \
                             all three core values.",
                            missing.join(", ")
                        ),
                        period: Some(*curr_pid),
                        materiality: None,
                        nodes: vec![
                            self.retained_earnings_node.clone(),
                            self.net_income_node.clone(),
                        ],
                    });
                    continue;
                }
            };

            let dividends = self
                .dividends_node
                .as_ref()
                .and_then(|n| get_node_value(context.results, n, curr_pid))
                .unwrap_or(0.0);

            // Dividends must be a non-negative magnitude under the
            // default convention; the formula subtracts it.
            if let Some(node) = self.dividends_node.as_ref() {
                if let Some(f) = self.dividends_sign_convention.validate(
                    dividends,
                    node.as_str(),
                    Some(*curr_pid),
                    self.id(),
                ) {
                    findings.push(f);
                }
            }

            let adjustments = sum_nodes(context.results, &self.other_adjustments, curr_pid);

            let expected = re_prev + ni - dividends + adjustments;
            let diff = re_curr - expected;
            let reference = re_prev.abs().max(1.0);
            let tolerance = effective_tolerance(&context.config, self.tolerance, reference);

            if diff.abs() > tolerance {
                let relative = (diff / reference).abs() * 100.0;

                let mut nodes = vec![
                    self.retained_earnings_node.clone(),
                    self.net_income_node.clone(),
                ];
                if let Some(ref d) = self.dividends_node {
                    nodes.push(d.clone());
                }
                nodes.extend(self.other_adjustments.iter().cloned());

                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "Retained earnings do not reconcile in {curr_pid}: \
                         actual RE ({re_curr:.2}) != expected ({expected:.2}), \
                         difference = {diff:.2}"
                    ),
                    period: Some(*curr_pid),
                    materiality: Some(Materiality {
                        absolute: diff.abs(),
                        relative_pct: relative,
                        reference_value: re_prev,
                        reference_label: "prior_retained_earnings".to_string(),
                    }),
                    nodes,
                });
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
