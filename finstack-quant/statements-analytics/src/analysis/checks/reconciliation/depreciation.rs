//! PP&E / depreciation reconciliation check.

use serde::{Deserialize, Serialize};

use super::super::get_node_value;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
    SignConventionPolicy,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::Result;

/// Verifies PP&E(t) = PP&E(t−1) + Capex(t) − D&A(t) − Disposals(t).
///
/// Skips the first period because no prior balance is available.
///
/// # Sign convention
///
/// `capex_node`, `depreciation_expense_node`, and `disposals_node` are
/// expected to carry [`SignConventionPolicy::MagnitudePositive`] by
/// default. The formula subtracts D&A and disposals explicitly, so any
/// negative magnitude would double-sign the reconciliation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepreciationReconciliation {
    /// D&A expense node (income statement).
    pub depreciation_expense_node: NodeId,
    /// PP&E balance node (balance sheet).
    pub ppe_node: NodeId,
    /// Capital expenditures node (cash flow statement / investing).
    pub capex_node: NodeId,
    /// Optional asset disposals node.
    pub disposals_node: Option<NodeId>,
    /// Tolerance override; falls back to
    /// [`finstack_quant_statements::checks::CheckConfig::default_tolerance`].
    pub tolerance: Option<f64>,
    /// Sign convention applied to `capex_node`,
    /// `depreciation_expense_node`, and `disposals_node`. Defaults to
    /// [`SignConventionPolicy::MagnitudePositive`].
    #[serde(default)]
    pub sign_convention: SignConventionPolicy,
}

impl Check for DepreciationReconciliation {
    fn id(&self) -> &str {
        "depreciation_reconciliation"
    }

    fn name(&self) -> &str {
        "Depreciation Reconciliation"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::CrossStatementReconciliation
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let tolerance = self.tolerance.unwrap_or(context.config.default_tolerance);
        let mut findings = Vec::new();
        let periods = &context.model.periods;

        for i in 1..periods.len() {
            let prev_pid = &periods[i - 1].id;
            let curr_pid = &periods[i].id;

            let Some(ppe_prev) = get_node_value(context.results, &self.ppe_node, prev_pid) else {
                continue;
            };
            let Some(ppe_curr) = get_node_value(context.results, &self.ppe_node, curr_pid) else {
                continue;
            };
            let Some(capex) = get_node_value(context.results, &self.capex_node, curr_pid) else {
                continue;
            };
            let Some(da) =
                get_node_value(context.results, &self.depreciation_expense_node, curr_pid)
            else {
                continue;
            };

            let disposals = self
                .disposals_node
                .as_ref()
                .and_then(|n| get_node_value(context.results, n, curr_pid))
                .unwrap_or(0.0);

            // Flag magnitude-positive violations before the
            // reconciliation math.
            if let Some(f) = self.sign_convention.validate(
                capex,
                self.capex_node.as_str(),
                Some(*curr_pid),
                self.id(),
            ) {
                findings.push(f);
            }
            if let Some(f) = self.sign_convention.validate(
                da,
                self.depreciation_expense_node.as_str(),
                Some(*curr_pid),
                self.id(),
            ) {
                findings.push(f);
            }
            if let Some(node) = self.disposals_node.as_ref() {
                if let Some(f) = self.sign_convention.validate(
                    disposals,
                    node.as_str(),
                    Some(*curr_pid),
                    self.id(),
                ) {
                    findings.push(f);
                }
            }

            let expected = ppe_prev + capex - da - disposals;
            let diff = ppe_curr - expected;

            if diff.abs() > tolerance {
                let reference = ppe_prev.abs().max(1.0);
                let relative = (diff / reference).abs() * 100.0;

                let mut nodes = vec![
                    self.ppe_node.clone(),
                    self.capex_node.clone(),
                    self.depreciation_expense_node.clone(),
                ];
                if let Some(ref d) = self.disposals_node {
                    nodes.push(d.clone());
                }

                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "PP&E does not reconcile in {curr_pid}: \
                         actual ({ppe_curr:.2}) != prior ({ppe_prev:.2}) + capex ({capex:.2}) \
                         - D&A ({da:.2}) - disposals ({disposals:.2}) = expected ({expected:.2}), \
                         difference = {diff:.2}"
                    ),
                    period: Some(*curr_pid),
                    materiality: Some(Materiality {
                        absolute: diff.abs(),
                        relative_pct: relative,
                        reference_value: ppe_prev,
                        reference_label: "prior_ppe".to_string(),
                    }),
                    nodes,
                });
            }
        }

        let passed = !findings.iter().any(|f| f.severity >= Severity::Error);

        Ok(CheckResult {
            check_id: self.id().to_string(),
            check_name: self.name().to_string(),
            category: self.category(),
            passed,
            findings,
        })
    }
}
