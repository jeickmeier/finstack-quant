//! Sign convention check.

use serde::{Deserialize, Serialize};

use crate::checks::helpers::get_node_value;
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

        for period in &context.model.periods {
            let pid = &period.id;
            // Preserve positive-node findings before negative-node findings.
            for (nodes, positive) in [(&self.positive_nodes, true), (&self.negative_nodes, false)] {
                for node in nodes {
                    let Some(val) = get_node_value(context.results, node, pid) else {
                        continue;
                    };
                    let message = if !val.is_finite() {
                        format!(
                            "Node '{}' has non-finite value ({val}) in period {pid}; \
                             its sign convention cannot be verified",
                            node.as_str()
                        )
                    } else if (positive && val < 0.0) || (!positive && val > 0.0) {
                        let sign = if positive { "negative" } else { "positive" };
                        format!(
                            "Node '{}' has unexpected {sign} value ({val:.2}) in period {pid}",
                            node.as_str()
                        )
                    } else {
                        continue;
                    };
                    findings.push(CheckFinding {
                        check_id: self.id().to_string(),
                        severity: Severity::Warning,
                        message,
                        period: Some(*pid),
                        materiality: None,
                        nodes: vec![node.clone()],
                        comparison: None,
                    });
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
            comparisons: Vec::new(),
        })
    }
}
