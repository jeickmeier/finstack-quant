//! Leverage-range check.

use serde::{Deserialize, Serialize};

use super::super::get_finite_node_value;
use crate::analysis::reports::trailing_sum_at;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::Result;

/// Flags periods where Debt / TTM EBITDA falls outside configurable warning
/// and error ranges.
///
/// Leverage is `debt[t] / TTM(ebitda)` ending at `t`, using the same
/// periods-per-year window as credit-assessment reporting. Annual models
/// have window size 1. Incomplete TTM windows (for example Q1 of a
/// quarterly model) are skipped rather than using a single-period EBITDA.
///
/// Periods with a full window and non-positive TTM EBITDA emit a
/// high-severity "leverage undefined" finding so the case surfaces
/// explicitly rather than silently passing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

            let Some(debt) = get_finite_node_value(context.results, &self.debt_node, pid) else {
                continue;
            };
            let Some(ebitda) = trailing_sum_at(context.results, self.ebitda_node.as_str(), pid)
            else {
                continue;
            };

            if ebitda <= 0.0 {
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "Debt/EBITDA undefined in {pid}: TTM EBITDA = {ebitda:.2} (≤ 0)"
                    ),
                    period: Some(*pid),
                    materiality: Some(Materiality {
                        absolute: ebitda,
                        relative_pct: 0.0,
                        reference_value: ebitda,
                        reference_label: "ttm_ebitda".to_string(),
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
                        reference_label: "ttm_ebitda".to_string(),
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
