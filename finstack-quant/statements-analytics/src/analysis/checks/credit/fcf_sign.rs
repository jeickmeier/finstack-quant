//! Free-cash-flow sign check.

use serde::{Deserialize, Serialize};

use super::super::get_node_value;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Severity,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::Result;

/// Tracks consecutive periods of negative free cash flow and flags at
/// configurable thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FcfSignCheck {
    /// Free cash flow node.
    pub fcf_node: NodeId,
    /// Number of consecutive negative periods before a warning.
    pub consecutive_negative_warning: usize,
    /// Number of consecutive negative periods before an error.
    pub consecutive_negative_error: usize,
}

impl Check for FcfSignCheck {
    fn id(&self) -> &str {
        "fcf_sign"
    }

    fn name(&self) -> &str {
        "FCF Sign"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::CreditReasonableness
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        if self.consecutive_negative_warning == 0 || self.consecutive_negative_error == 0 {
            return Err(finstack_quant_statements::error::Error::eval(
                "FcfSignCheck thresholds must be positive".to_string(),
            ));
        }

        let mut findings = Vec::new();
        let mut consecutive_negative: usize = 0;

        for period_spec in &context.model.periods {
            let pid = &period_spec.id;

            let Some(fcf) = get_node_value(context.results, &self.fcf_node, pid) else {
                consecutive_negative = 0;
                continue;
            };

            if fcf < 0.0 {
                consecutive_negative += 1;
            } else {
                consecutive_negative = 0;
                continue;
            }

            let severity = if consecutive_negative >= self.consecutive_negative_error {
                Some(Severity::Error)
            } else if consecutive_negative >= self.consecutive_negative_warning {
                Some(Severity::Warning)
            } else {
                None
            };

            if let Some(sev) = severity {
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: sev,
                    message: format!(
                        "FCF negative for {consecutive_negative} consecutive periods \
                         as of {pid} (current = {fcf:.2})"
                    ),
                    period: Some(*pid),
                    materiality: None,
                    nodes: vec![self.fcf_node.clone()],
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
