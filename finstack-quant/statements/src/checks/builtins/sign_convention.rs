//! Sign convention check.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::checks::identity::require_known_distinct;
use crate::checks::{Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Severity};
use crate::types::NodeId;

/// Flags finite values with an unexpected sign (revenue below zero, an
/// expense above zero).
///
/// **Advisory only**: every finding is `Severity::Warning`, so the check
/// never fails. Zero is valid in either group, and a cell without a value is
/// not examined: mandatory sign rules belong to the operation that owns
/// them (the retained-earnings dividend convention, for example).
///
/// A node the model does not know, a node listed twice, or a node in both
/// groups is a configuration error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct SignConventionCheck {
    /// Nodes expected to be non-negative.
    #[serde(default)]
    pub positive_nodes: Vec<NodeId>,
    /// Nodes expected to be non-positive.
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
        require_known_distinct(
            self.id(),
            context,
            &[
                ("positive_nodes", &self.positive_nodes),
                ("negative_nodes", &self.negative_nodes),
            ],
        )?;
        let mut findings = Vec::new();
        let mut evaluated = 0_usize;

        for period in context.periods() {
            let pid = &period.id;
            // Positive-node findings come before negative-node findings.
            for (nodes, positive) in [(&self.positive_nodes, true), (&self.negative_nodes, false)] {
                for node in nodes {
                    let Some(value) = context
                        .results
                        .get_cell(node.as_str(), pid)
                        .and_then(|cell| cell.numeric())
                        .filter(|value| value.is_finite())
                    else {
                        continue;
                    };
                    evaluated += 1;
                    if (positive && value >= 0.0) || (!positive && value <= 0.0) {
                        continue;
                    }
                    let sign = if positive { "negative" } else { "positive" };
                    findings.push(CheckFinding {
                        check_id: self.id().to_string(),
                        severity: Severity::Warning,
                        message: format!(
                            "Node '{node}' has unexpected {sign} value ({value:.2}) in period {pid}"
                        ),
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
