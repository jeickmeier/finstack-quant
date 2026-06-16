//! Coverage-floor check.

use serde::{Deserialize, Serialize};

use super::super::get_node_value;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::Result;

/// Flags periods where a coverage ratio (e.g. DSCR, interest coverage)
/// falls below configurable warning and error floors.
///
/// A *negative* denominator makes the coverage ratio undefined and emits a
/// high-severity finding (consistent with `LeverageRangeCheck`'s treatment
/// of non-positive EBITDA). A *zero* denominator means there is no debt
/// service / interest to cover and the period is skipped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageFloorCheck {
    /// Numerator node (e.g. EBITDA, cash flow available for debt service).
    pub numerator_node: NodeId,
    /// Denominator node (e.g. interest expense, total debt service).
    pub denominator_node: NodeId,
    /// Minimum ratio before a warning is issued.
    pub min_warning: f64,
    /// Minimum ratio before an error is issued.
    pub min_error: f64,
}

impl Check for CoverageFloorCheck {
    fn id(&self) -> &str {
        "coverage_floor"
    }

    fn name(&self) -> &str {
        "Coverage Floor"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::CreditReasonableness
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let mut findings = Vec::new();

        for period_spec in &context.model.periods {
            let pid = &period_spec.id;

            let Some(num) = get_node_value(context.results, &self.numerator_node, pid) else {
                continue;
            };
            let Some(denom) = get_node_value(context.results, &self.denominator_node, pid) else {
                continue;
            };

            if denom < 0.0 {
                // Negative denominator → ratio undefined; surface explicitly
                // rather than silently passing (matches LeverageRangeCheck).
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "Coverage ratio undefined in {pid}: denominator = {denom:.2} (< 0)"
                    ),
                    period: Some(*pid),
                    materiality: Some(Materiality {
                        absolute: denom,
                        relative_pct: 0.0,
                        reference_value: denom,
                        reference_label: "denominator".to_string(),
                    }),
                    nodes: vec![self.numerator_node.clone(), self.denominator_node.clone()],
                });
                continue;
            }
            if denom == 0.0 {
                // No debt service to cover this period — nothing to test.
                continue;
            }

            let ratio = num / denom;

            let severity = if ratio < self.min_error {
                Some(Severity::Error)
            } else if ratio < self.min_warning {
                Some(Severity::Warning)
            } else {
                None
            };

            if let Some(sev) = severity {
                let floor = if sev == Severity::Error {
                    self.min_error
                } else {
                    self.min_warning
                };

                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: sev,
                    message: format!(
                        "Coverage ratio {ratio:.2}x in {pid} below {sev:?} floor {floor:.2}x",
                    ),
                    period: Some(*pid),
                    materiality: Some(Materiality {
                        absolute: ratio,
                        relative_pct: 0.0,
                        reference_value: denom,
                        reference_label: "denominator".to_string(),
                    }),
                    nodes: vec![self.numerator_node.clone(), self.denominator_node.clone()],
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
