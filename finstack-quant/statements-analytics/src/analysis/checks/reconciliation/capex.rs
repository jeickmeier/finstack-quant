//! Capital-expenditure reconciliation check.

use serde::{Deserialize, Serialize};

use super::super::get_node_value;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
    SignConventionPolicy,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::Result;

/// Verifies that cash-flow-statement capex equals the sum of PP&E additions
/// and intangible additions.
///
/// # Sign convention
///
/// All three capex-related nodes (`capex_cf_node`, `ppe_additions_node`,
/// `intangible_additions_node`) are expected to carry the
/// [`SignConventionPolicy::MagnitudePositive`] convention by default:
/// they are non-negative magnitudes, and the reconciliation matches
/// them directly (the CFS convention of reporting capex as a negative
/// outflow should be normalized at ingest to a magnitude — the
/// `sign_convention` field below flags violations at runtime).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapexReconciliation {
    /// Capex from the cash flow statement (investing section).
    pub capex_cf_node: NodeId,
    /// Optional PP&E additions node.
    pub ppe_additions_node: Option<NodeId>,
    /// Optional intangible-asset additions node.
    pub intangible_additions_node: Option<NodeId>,
    /// Tolerance override; falls back to
    /// [`finstack_quant_statements::checks::CheckConfig::default_tolerance`].
    pub tolerance: Option<f64>,
    /// Sign convention applied to `capex_cf_node`, `ppe_additions_node`,
    /// and `intangible_additions_node`. Defaults to
    /// [`SignConventionPolicy::MagnitudePositive`].
    #[serde(default)]
    pub sign_convention: SignConventionPolicy,
}

impl Check for CapexReconciliation {
    fn id(&self) -> &str {
        "capex_reconciliation"
    }

    fn name(&self) -> &str {
        "Capex Reconciliation"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::CrossStatementReconciliation
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let tolerance = self.tolerance.unwrap_or(context.config.default_tolerance);
        let mut findings = Vec::new();

        let has_components =
            self.ppe_additions_node.is_some() || self.intangible_additions_node.is_some();
        if !has_components {
            return Ok(CheckResult {
                check_id: self.id().to_string(),
                check_name: self.name().to_string(),
                category: self.category(),
                passed: true,
                findings,
            });
        }

        for period_spec in &context.model.periods {
            let pid = &period_spec.id;

            let Some(capex_cf) = get_node_value(context.results, &self.capex_cf_node, pid) else {
                continue;
            };

            // Flag sign-convention violations before the reconciliation
            // math — a negative `capex_cf` under the default
            // MagnitudePositive convention would produce a misleading
            // reconciliation-diff finding.
            if let Some(f) = self.sign_convention.validate(
                capex_cf,
                self.capex_cf_node.as_str(),
                Some(*pid),
                self.id(),
            ) {
                findings.push(f);
            }

            let ppe_add = self
                .ppe_additions_node
                .as_ref()
                .and_then(|n| get_node_value(context.results, n, pid))
                .unwrap_or(0.0);

            if let Some(node) = self.ppe_additions_node.as_ref() {
                if let Some(f) =
                    self.sign_convention
                        .validate(ppe_add, node.as_str(), Some(*pid), self.id())
                {
                    findings.push(f);
                }
            }

            let intangible_add = self
                .intangible_additions_node
                .as_ref()
                .and_then(|n| get_node_value(context.results, n, pid))
                .unwrap_or(0.0);

            if let Some(node) = self.intangible_additions_node.as_ref() {
                if let Some(f) = self.sign_convention.validate(
                    intangible_add,
                    node.as_str(),
                    Some(*pid),
                    self.id(),
                ) {
                    findings.push(f);
                }
            }

            let expected = ppe_add + intangible_add;
            let diff = capex_cf - expected;

            if diff.abs() > tolerance {
                let reference = expected.abs().max(1.0);
                let relative = (diff / reference).abs() * 100.0;

                let mut nodes = vec![self.capex_cf_node.clone()];
                if let Some(ref n) = self.ppe_additions_node {
                    nodes.push(n.clone());
                }
                if let Some(ref n) = self.intangible_additions_node {
                    nodes.push(n.clone());
                }

                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "CF capex ({capex_cf:.2}) != PPE additions ({ppe_add:.2}) + \
                         intangible additions ({intangible_add:.2}) = {expected:.2} \
                         in {pid}, difference = {diff:.2}"
                    ),
                    period: Some(*pid),
                    materiality: Some(Materiality {
                        absolute: diff.abs(),
                        relative_pct: relative,
                        reference_value: expected,
                        reference_label: "balance_sheet_additions".to_string(),
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
