//! Liquidity-runway check.

use serde::{Deserialize, Serialize};

use super::super::get_node_value;
use finstack_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
};
use finstack_statements::types::NodeId;
use finstack_statements::Result;

/// Estimates the liquidity runway in months and flags periods that fall
/// below configurable warning and error thresholds.
///
/// The burn node is interpreted as cash burn **per model period**; the
/// runway is converted to months using the model's period cadence:
/// `months = (cash / burn_per_period) * (12 / periods_per_year)` — e.g. a
/// quarterly model with `cash / burn = 2` periods of runway reports 6
/// months.
///
/// Periods with non-positive cash burn are skipped (the company is not
/// burning cash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityRunwayCheck {
    /// Cash balance node.
    pub cash_node: NodeId,
    /// Cash burn rate node (positive = burning cash).
    pub cash_burn_node: NodeId,
    /// Minimum runway (in months) before a warning.
    pub min_months_warning: f64,
    /// Minimum runway (in months) before an error.
    pub min_months_error: f64,
}

impl Check for LiquidityRunwayCheck {
    fn id(&self) -> &str {
        "liquidity_runway"
    }

    fn name(&self) -> &str {
        "Liquidity Runway"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::CreditReasonableness
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let mut findings = Vec::new();

        for period_spec in &context.model.periods {
            let pid = &period_spec.id;

            let Some(cash) = get_node_value(context.results, &self.cash_node, pid) else {
                continue;
            };
            let Some(burn) = get_node_value(context.results, &self.cash_burn_node, pid) else {
                continue;
            };

            if burn <= 0.0 {
                continue;
            }

            // `cash / burn` is the runway in *model periods*; convert to
            // months using the period cadence (12 / periods_per_year).
            let periods_per_year = f64::from(pid.kind().periods_per_year());
            let months = (cash / burn) * (12.0 / periods_per_year);

            let severity = if months < self.min_months_error {
                Some(Severity::Error)
            } else if months < self.min_months_warning {
                Some(Severity::Warning)
            } else {
                None
            };

            if let Some(sev) = severity {
                let floor = if sev == Severity::Error {
                    self.min_months_error
                } else {
                    self.min_months_warning
                };

                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: sev,
                    message: format!(
                        "Liquidity runway {months:.1} months in {pid} below \
                         {sev:?} threshold {floor:.1} months",
                    ),
                    period: Some(*pid),
                    materiality: Some(Materiality {
                        absolute: months,
                        relative_pct: 0.0,
                        reference_value: burn,
                        reference_label: "cash_burn".to_string(),
                    }),
                    nodes: vec![self.cash_node.clone(), self.cash_burn_node.clone()],
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
