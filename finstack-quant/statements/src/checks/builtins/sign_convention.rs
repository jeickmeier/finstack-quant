//! Sign convention check.

use serde::{Deserialize, Serialize};

use super::get_node_value;
use crate::checks::{Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Severity};
use crate::types::NodeId;
use crate::Result;

/// Flags values with unexpected signs (e.g., revenue < 0 or expense > 0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignConventionCheck {
    /// Nodes expected to carry positive values.
    pub positive_nodes: Vec<NodeId>,
    /// Nodes expected to carry negative values.
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

        for period in &context.model.periods {
            let pid = &period.id;

            for node in &self.positive_nodes {
                if let Some(val) = get_node_value(context.results, node, pid) {
                    if val < 0.0 {
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
                    if val > 0.0 {
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
