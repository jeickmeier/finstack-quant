//! Missing value check.

use serde::{Deserialize, Serialize};

use super::get_node_value;
use crate::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, PeriodScope, Severity,
};
use crate::types::NodeId;
use crate::Result;

/// Flags required nodes that lack values in applicable periods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingValueCheck {
    /// Nodes that must have values in every in-scope period.
    pub required_nodes: Vec<NodeId>,
    /// Which periods to inspect.
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
        let mut findings = Vec::new();

        for period in &context.model.periods {
            let in_scope = match self.scope {
                PeriodScope::AllPeriods => true,
                PeriodScope::ActualsOnly => period.is_actual,
                PeriodScope::ForecastOnly => !period.is_actual,
            };
            if !in_scope {
                continue;
            }

            let severity = if period.is_actual {
                Severity::Error
            } else {
                Severity::Warning
            };

            for node in &self.required_nodes {
                if get_node_value(context.results, node, &period.id).is_none() {
                    findings.push(CheckFinding {
                        check_id: self.id().to_string(),
                        severity,
                        message: format!(
                            "Missing value for node '{}' in period {}",
                            node.as_str(),
                            period.id
                        ),
                        period: Some(period.id),
                        materiality: None,
                        nodes: vec![node.clone()],
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
        })
    }
}
