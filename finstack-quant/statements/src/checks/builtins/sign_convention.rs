//! Sign convention check.

use serde::{Deserialize, Serialize};

use super::get_node_value;
use crate::checks::{Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Severity};
use crate::types::NodeId;
use crate::Result;

/// Flags values with unexpected signs (e.g., revenue < 0 or expense > 0).
///
/// **Advisory-only**: every finding is `Severity::Warning`, so this check's
/// `CheckResult::passed` is always `true` and it never fails a pipeline gate.
/// Treat its findings as review prompts, not assertions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct SignConventionCheck {
    /// Nodes expected to carry positive values.
    #[serde(default)]
    pub positive_nodes: Vec<NodeId>,
    /// Nodes expected to carry negative values.
    #[serde(default)]
    pub negative_nodes: Vec<NodeId>,
}

impl Check for SignConventionCheck {
    fn id(&self) -> &str {
        "sign_convention"
    }

    fn name(&self) -> &str {
        "Sign Convention"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::DataQuality
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let mut findings = Vec::new();

        // NaN compares false against both `< 0.0` and `> 0.0`, so a non-finite
        // value would silently satisfy either sign expectation. Surface it as
        // its own finding instead of letting it pass.
        let non_finite_finding = |node: &NodeId, pid, val: f64| CheckFinding {
            check_id: "sign_convention".to_string(),
            severity: Severity::Warning,
            message: format!(
                "Node '{}' has non-finite value ({val}) in period {pid}; \
                 its sign convention cannot be verified",
                node.as_str()
            ),
            period: Some(pid),
            materiality: None,
            nodes: vec![node.clone()],
        };

        for period in &context.model.periods {
            let pid = &period.id;

            for node in &self.positive_nodes {
                if let Some(val) = get_node_value(context.results, node, pid) {
                    if !val.is_finite() {
                        findings.push(non_finite_finding(node, *pid, val));
                    } else if val < 0.0 {
                        findings.push(CheckFinding {
                            check_id: self.id().to_string(),
                            severity: Severity::Warning,
                            message: format!(
                                "Node '{}' has unexpected negative value ({val:.2}) \
                                 in period {pid}",
                                node.as_str()
                            ),
                            period: Some(*pid),
                            materiality: None,
                            nodes: vec![node.clone()],
                        });
                    }
                }
            }

            for node in &self.negative_nodes {
                if let Some(val) = get_node_value(context.results, node, pid) {
                    if !val.is_finite() {
                        findings.push(non_finite_finding(node, *pid, val));
                    } else if val > 0.0 {
                        findings.push(CheckFinding {
                            check_id: self.id().to_string(),
                            severity: Severity::Warning,
                            message: format!(
                                "Node '{}' has unexpected positive value ({val:.2}) \
                                 in period {pid}",
                                node.as_str()
                            ),
                            period: Some(*pid),
                            materiality: None,
                            nodes: vec![node.clone()],
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
