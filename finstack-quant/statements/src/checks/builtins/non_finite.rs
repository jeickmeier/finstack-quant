//! Value validity check.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::checks::identity::require_known_distinct;
use crate::checks::{Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Severity};
use crate::types::NodeId;

/// The nodes a validity check covers: every modeled node, selected
/// explicitly, or a named list.
///
/// The JSON form is the string `"all"` or an array of node identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum NodeSelection {
    /// Every node of the evaluated results.
    All(AllNodes),
    /// The listed nodes, each a known node named once.
    Named(Vec<NodeId>),
}

/// The explicit selection of every modeled node (`"all"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AllNodes {
    /// Every modeled node.
    All,
}

impl NodeSelection {
    /// Select every modeled node.
    #[must_use]
    pub const fn all() -> Self {
        Self::All(AllNodes::All)
    }

    /// Select the listed nodes.
    ///
    /// # Arguments
    ///
    /// * `nodes` - Identifiers of known nodes, each listed once.
    #[must_use]
    pub fn named(nodes: Vec<NodeId>) -> Self {
        Self::Named(nodes)
    }
}

/// Detects values that are not valid numbers: a NaN or an infinity.
///
/// **Gating**: findings are `Severity::Error`, so one invalid value fails
/// the check, whether or not any other check requires the cell. A typed
/// zero is valid, and a cell that is missing, masked or unavailable for a
/// stated reason is not a value and is not examined: coverage belongs to
/// [`MissingValueCheck`](super::MissingValueCheck).
///
/// The covered nodes are selected explicitly: all modeled nodes, or a named
/// list in which an unknown or repeated identifier is a configuration
/// error. A cell in a period the model's plan does not have, and a node
/// whose result units contradict the model's, are errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct NonFiniteCheck {
    /// The nodes to check: `"all"` or a list of node identifiers.
    pub nodes: NodeSelection,
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
        let selected: Vec<NodeId> = match &self.nodes {
            NodeSelection::All(_) => context.results.node_ids().map(NodeId::new).collect(),
            NodeSelection::Named(nodes) => {
                require_known_distinct(self.id(), context, &[("nodes", nodes)])?;
                nodes.clone()
            }
        };
        let mut findings = Vec::new();
        let mut evaluated = 0_usize;

        for node in &selected {
            // Units must be resolved and agree between model and results.
            crate::checks::lookup::validated_input_value_type(self.id(), context, node.as_str())?;
            let Some(cells) = context.results.cells.get(node.as_str()) else {
                continue;
            };
            for (period_id, cell) in cells {
                if !context
                    .model
                    .periods
                    .iter()
                    .any(|period| period.id == *period_id)
                {
                    return Err(crate::Error::invalid_input(format!(
                        "{}: node '{node}' has a cell in {period_id}, which is not a period of \
                         the model",
                        self.id()
                    )));
                }
                let Some(value) = cell.numeric() else {
                    continue;
                };
                evaluated += 1;
                if !value.is_finite() {
                    findings.push(CheckFinding {
                        check_id: self.id().to_string(),
                        severity: Severity::Error,
                        message: format!(
                            "Non-finite value ({value}) for node '{node}' in period {period_id}"
                        ),
                        period: Some(*period_id),
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
