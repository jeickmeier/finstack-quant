//! Leverage-range check.

use serde::{Deserialize, Serialize};

use super::super::get_node_value;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::Result;

/// Flags periods where Debt / EBITDA falls outside configurable warning
/// and error ranges.
///
/// Periods with non-positive EBITDA emit a high-severity "leverage
/// undefined" finding so the case surfaces explicitly rather than
/// silently passing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeverageRangeCheck {
    /// Total debt node.
    pub debt_node: NodeId,
    /// EBITDA node.
    pub ebitda_node: NodeId,
    /// `(min, max)` range that triggers a warning when exceeded.
    pub warn_range: (f64, f64),
    /// `(min, max)` range that triggers an error when exceeded.
    pub error_range: (f64, f64),
}

impl Check for LeverageRangeCheck {
    fn id(&self) -> &str {
        "leverage_range"
    }

    fn name(&self) -> &str {
        "Leverage Range"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::CreditReasonableness
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let mut findings = Vec::new();

        for period_spec in &context.model.periods {
            let pid = &period_spec.id;

            let Some(debt) = get_node_value(context.results, &self.debt_node, pid) else {
                continue;
            };
            let Some(ebitda) = get_node_value(context.results, &self.ebitda_node, pid) else {
                continue;
            };

            if ebitda <= 0.0 {
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Error,
                    message: format!("Debt/EBITDA undefined in {pid}: EBITDA = {ebitda:.2} (≤ 0)"),
                    period: Some(*pid),
                    materiality: Some(Materiality {
                        absolute: ebitda,
                        relative_pct: 0.0,
                        reference_value: ebitda,
                        reference_label: "ebitda".to_string(),
                    }),
                    nodes: vec![self.debt_node.clone(), self.ebitda_node.clone()],
                });
                continue;
            }

            let leverage = debt / ebitda;

            let severity = if leverage < self.error_range.0 || leverage > self.error_range.1 {
                Some(Severity::Error)
            } else if leverage < self.warn_range.0 || leverage > self.warn_range.1 {
                Some(Severity::Warning)
            } else {
                None
            };

            if let Some(sev) = severity {
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: sev,
                    message: format!(
                        "Debt/EBITDA {leverage:.2}x in {pid} outside \
                         {sev:?} range [{:.1}x, {:.1}x]",
                        if sev == Severity::Error {
                            self.error_range.0
                        } else {
                            self.warn_range.0
                        },
                        if sev == Severity::Error {
                            self.error_range.1
                        } else {
                            self.warn_range.1
                        },
                    ),
                    period: Some(*pid),
                    materiality: Some(Materiality {
                        absolute: leverage,
                        relative_pct: 0.0,
                        reference_value: ebitda,
                        reference_label: "ebitda".to_string(),
                    }),
                    nodes: vec![self.debt_node.clone(), self.ebitda_node.clone()],
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
