//! Dividend reconciliation check.

use serde::{Deserialize, Serialize};

use super::super::get_node_value;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
    SignConventionPolicy,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::Result;

/// Verifies that dividends on the cash flow statement equal the dividends
/// charged against equity.
///
/// # Sign convention
///
/// Both dividend nodes are expected to carry
/// [`SignConventionPolicy::MagnitudePositive`] by default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DividendReconciliation {
    /// Dividends paid node (cash flow statement, financing).
    pub dividends_cf_node: NodeId,
    /// Dividends node (equity / retained earnings schedule).
    pub dividends_equity_node: NodeId,
    /// Tolerance override; falls back to
    /// [`finstack_quant_statements::checks::CheckConfig::default_tolerance`].
    pub tolerance: Option<f64>,
    /// Sign convention applied to both dividend inputs. Defaults to
    /// [`SignConventionPolicy::MagnitudePositive`].
    #[serde(default)]
    pub sign_convention: SignConventionPolicy,
}

impl Check for DividendReconciliation {
    fn id(&self) -> &str {
        "dividend_reconciliation"
    }

    fn name(&self) -> &str {
        "Dividend Reconciliation"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::CrossStatementReconciliation
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let tolerance = self.tolerance.unwrap_or(context.config.default_tolerance);
        let mut findings = Vec::new();

        for period_spec in &context.model.periods {
            let pid = &period_spec.id;

            let Some(div_cf) = get_node_value(context.results, &self.dividends_cf_node, pid) else {
                continue;
            };
            let Some(div_eq) = get_node_value(context.results, &self.dividends_equity_node, pid)
            else {
                continue;
            };

            // Flag magnitude-positive violations.
            if let Some(f) = self.sign_convention.validate(
                div_cf,
                self.dividends_cf_node.as_str(),
                Some(*pid),
                self.id(),
            ) {
                findings.push(f);
            }
            if let Some(f) = self.sign_convention.validate(
                div_eq,
                self.dividends_equity_node.as_str(),
                Some(*pid),
                self.id(),
            ) {
                findings.push(f);
            }

            let diff = div_cf - div_eq;

            if diff.abs() > tolerance {
                let reference = div_eq.abs().max(1.0);
                let relative = (diff / reference).abs() * 100.0;

                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "Dividends do not reconcile in {pid}: CF ({div_cf:.2}) != \
                         equity ({div_eq:.2}), difference = {diff:.2}"
                    ),
                    period: Some(*pid),
                    materiality: Some(Materiality {
                        absolute: diff.abs(),
                        relative_pct: relative,
                        reference_value: div_eq,
                        reference_label: "equity_dividends".to_string(),
                    }),
                    nodes: vec![
                        self.dividends_cf_node.clone(),
                        self.dividends_equity_node.clone(),
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
