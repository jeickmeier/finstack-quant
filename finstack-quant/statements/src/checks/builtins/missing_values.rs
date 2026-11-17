//! Required-observation coverage check.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::checks::identity::require_known_distinct;
use crate::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, PeriodScope, Severity,
};
use crate::evaluator::CellStatus;
use crate::types::NodeId;

/// Checks that every required node has a value in every period of the
/// selected scope.
///
/// The required nodes and the period scope (actuals, forecasts or both) are
/// stated explicitly. An observation that is missing, or unavailable for a
/// stated reason, makes the covered scope incomplete: it is a coverage
/// finding, and the check's outcome is `incomplete`. A typed zero is a valid
/// observation. A scope with no required node or no period in it examines
/// nothing and is `unevaluated`. `evaluated` counts the required cells
/// examined; the findings list the missing and unavailable ones.
///
/// A required node the model does not know, or one listed twice, is a
/// configuration error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct MissingValueCheck {
    /// Nodes that must have a value in every in-scope period.
    pub required_nodes: Vec<NodeId>,
    /// Which periods are in scope: all, actuals only or forecasts only.
    pub scope: PeriodScope,
}

impl Check for MissingValueCheck {
    fn id(&self) -> &str {
        "missing_value"
    }

    fn name(&self) -> &str {
        "Missing Value"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::DataQuality
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        require_known_distinct(
            self.id(),
            context,
            &[("required_nodes", &self.required_nodes)],
        )?;
        let mut findings = Vec::new();
        let mut evaluated = 0_usize;

        for period in &context.model.periods {
            let in_scope = match self.scope {
                PeriodScope::AllPeriods => true,
                PeriodScope::ActualsOnly => period.is_actual,
                PeriodScope::ForecastOnly => !period.is_actual,
            };
            if !in_scope {
                continue;
            }
            for node in &self.required_nodes {
                evaluated += 1;
                let cell = context.results.get_cell(node.as_str(), &period.id);
                let state = match cell.map(|cell| (cell.status, cell.reason)) {
                    Some((CellStatus::Value, _)) => continue,
                    Some((CellStatus::Unavailable, Some(reason))) => {
                        format!("unavailable ({})", reason.as_str())
                    }
                    Some((CellStatus::Unavailable, None)) => "unavailable".to_string(),
                    Some((CellStatus::Masked, _)) => "masked".to_string(),
                    Some((CellStatus::Missing, _)) | None => "missing".to_string(),
                };
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "Required observation '{node}' is {state} in period {}",
                        period.id
                    ),
                    period: Some(period.id),
                    materiality: None,
                    nodes: vec![node.clone()],
                    comparison: None,
                    coverage: true,
                });
            }
        }

        Ok(CheckResult::new(self, evaluated, findings, Vec::new()))
    }
}
