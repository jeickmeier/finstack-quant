//! Non-finite value detection check.

use serde::{Deserialize, Serialize};

use crate::checks::{Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Severity};
use crate::types::NodeId;
use crate::Result;

/// Detects NaN or infinite values in node results.
///
/// **Gating**: findings are `Severity::Error`, so any non-finite cell sets
/// `CheckResult::passed` to `false` and fails a pipeline gate. Non-finite
/// values poison every downstream calculation, so they are never advisory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct NonFiniteCheck {
    /// Specific nodes to check; if empty, all nodes in results are inspected.
    #[serde(default)]
    pub nodes: Vec<NodeId>,
}

impl Check for NonFiniteCheck {
    fn id(&self) -> &str {
        "non_finite"
    }

    fn name(&self) -> &str {
        "Non-Finite Value"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::DataQuality
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let mut findings = Vec::new();

        if self.nodes.is_empty() {
            for (node_id, period_map) in &context.results.nodes {
                for (period_id, &value) in period_map {
                    if !value.is_finite() {
                        findings.push(CheckFinding {
                            check_id: self.id().to_string(),
                            severity: Severity::Error,
                            message: format!(
                                "Non-finite value ({value}) for node '{node_id}' \
                                 in period {period_id}"
                            ),
                            period: Some(*period_id),
                            materiality: None,
                            nodes: vec![NodeId::new(node_id)],
                        });
                    }
                }
            }
        } else {
            for node in &self.nodes {
                if let Some(period_map) = context.results.nodes.get(node.as_str()) {
                    for (period_id, &value) in period_map {
                        if !value.is_finite() {
                            findings.push(CheckFinding {
                                check_id: self.id().to_string(),
                                severity: Severity::Error,
                                message: format!(
                                    "Non-finite value ({value}) for node '{}' \
                                     in period {period_id}",
                                    node.as_str()
                                ),
                                period: Some(*period_id),
                                materiality: None,
                                nodes: vec![node.clone()],
                            });
                        }
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
