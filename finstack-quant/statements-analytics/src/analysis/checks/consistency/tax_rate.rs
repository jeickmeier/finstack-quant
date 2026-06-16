//! Effective tax rate range check.

use serde::{Deserialize, Serialize};

use super::super::get_node_value;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Severity,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::Result;

/// Verifies that the effective tax rate (tax expense / pretax income) falls
/// within an expected range.
///
/// Periods with zero or negative pretax income are skipped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveTaxRateCheck {
    /// Tax expense node.
    pub tax_expense_node: NodeId,
    /// Pretax income node.
    pub pretax_income_node: NodeId,
    /// Expected (min, max) for the effective tax rate (decimals, e.g. 0.15..0.40).
    pub expected_range: (f64, f64),
}

impl Check for EffectiveTaxRateCheck {
    fn id(&self) -> &str {
        "effective_tax_rate"
    }

    fn name(&self) -> &str {
        "Effective Tax Rate"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::InternalConsistency
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let mut findings = Vec::new();
        let (lo, hi) = self.expected_range;

        for period_spec in &context.model.periods {
            let pid = &period_spec.id;

            let Some(tax) = get_node_value(context.results, &self.tax_expense_node, pid) else {
                continue;
            };
            let Some(pretax) = get_node_value(context.results, &self.pretax_income_node, pid)
            else {
                continue;
            };

            if pretax <= 0.0 {
                continue;
            }

            let etr = tax / pretax;

            if etr < lo || etr > hi {
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Info,
                    message: format!(
                        "Effective tax rate {:.1}% in {pid} outside [{:.0}%, {:.0}%]",
                        etr * 100.0,
                        lo * 100.0,
                        hi * 100.0,
                    ),
                    period: Some(*pid),
                    materiality: None,
                    nodes: vec![
                        self.tax_expense_node.clone(),
                        self.pretax_income_node.clone(),
                    ],
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
