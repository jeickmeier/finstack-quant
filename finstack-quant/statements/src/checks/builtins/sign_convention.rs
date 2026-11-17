//! Sign convention check.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::checks::{Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Severity};
use crate::types::NodeId;

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
        let mut evaluated = 0_usize;

        for period in &context.model.periods {
            let pid = &period.id;
            // Preserve positive-node findings before negative-node findings.
            for (nodes, positive) in [(&self.positive_nodes, true), (&self.negative_nodes, false)] {
                for node in nodes {
                    let Some(cell) = context.results.get_cell(node.as_str(), pid) else {
                        continue;
                    };
                    evaluated += 1;
                    // A cell without a value cannot have its sign verified.
                    let val = cell.numeric().unwrap_or(f64::NAN);
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
                        coverage: false,
                    });
                }
            }
        }

        Ok(CheckResult::new(self, evaluated, findings, Vec::new()))
    }
}
