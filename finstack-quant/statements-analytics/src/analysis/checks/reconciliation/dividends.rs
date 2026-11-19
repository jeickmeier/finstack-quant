//! Dividend reconciliation check.

use serde::{Deserialize, Serialize};

use finstack_quant_statements::Result;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckComparison, CheckContext, CheckFinding, CheckResult, Materiality,
    Severity, SignConventionPolicy,
};
use finstack_quant_statements::types::NodeId;

/// Verifies that dividends on the cash flow statement equal the dividends
/// charged against equity.
///
/// # Sign convention
///
/// Both dividend nodes are expected to carry
/// [`SignConventionPolicy::MagnitudePositive`] by default.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
        let mut comparisons = Vec::new();

        for period_spec in context.periods() {
            let pid = &period_spec.id;

            // Both sides are read so a period missing either names each gap.
            let div_cf = crate::analysis::checks::required(
                context,
                self,
                &self.dividends_cf_node,
                pid,
                &mut findings,
            );
            let div_eq = crate::analysis::checks::required(
                context,
                self,
                &self.dividends_equity_node,
                pid,
                &mut findings,
            );
            let (Some(div_cf), Some(div_eq)) = (div_cf, div_eq) else {
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
            let comparison = CheckComparison::from_floats(
                "dividends_reconciliation",
                *pid,
                crate::analysis::checks::currency_of(context, &self.dividends_cf_node, pid),
                (div_cf, div_eq, tolerance),
            )?;
            comparisons.push(comparison.clone());

            if diff.abs() > tolerance {
                let reference = div_eq.abs().max(1.0);
                let relative = (diff / reference).abs();

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
                        relative: Some(relative),
                        reference_value: div_eq,
                        reference_label: "equity_dividends".to_string(),
                    }),
                    nodes: vec![
                        self.dividends_cf_node.clone(),
                        self.dividends_equity_node.clone(),
                    ],
                    comparison: Some(comparison),
                    coverage: false,
                });
            }
        }

        let evaluated = context.periods().len();
        Ok(CheckResult::new(self, evaluated, findings, comparisons))
    }
}
