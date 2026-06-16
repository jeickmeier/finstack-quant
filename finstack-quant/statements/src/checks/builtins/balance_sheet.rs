//! Balance sheet articulation check.

use serde::{Deserialize, Serialize};

use super::sum_nodes;
use crate::checks::types::effective_tolerance;
use crate::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
};
use crate::types::NodeId;
use crate::Result;

/// Verifies that Assets = Liabilities + Equity for every period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetArticulation {
    /// Node IDs whose values represent total assets.
    pub assets_nodes: Vec<NodeId>,
    /// Node IDs whose values represent total liabilities.
    pub liabilities_nodes: Vec<NodeId>,
    /// Node IDs whose values represent total equity.
    pub equity_nodes: Vec<NodeId>,
    /// Tolerance override; falls back to
    /// [`CheckConfig::default_tolerance`](crate::checks::CheckConfig::default_tolerance).
    pub tolerance: Option<f64>,
}

impl Check for BalanceSheetArticulation {
    fn id(&self) -> &str {
        "balance_sheet_articulation"
    }

    fn name(&self) -> &str {
        "Balance Sheet Articulation"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::AccountingIdentity
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let mut findings = Vec::new();

        for period in &context.model.periods {
            let pid = &period.id;
            let assets = sum_nodes(context.results, &self.assets_nodes, pid);
            let liabilities = sum_nodes(context.results, &self.liabilities_nodes, pid);
            let equity = sum_nodes(context.results, &self.equity_nodes, pid);
            let imbalance = assets - (liabilities + equity);
            let tolerance = effective_tolerance(&context.config, self.tolerance, assets);

            if imbalance.abs() > tolerance {
                let relative = if assets.abs() > f64::EPSILON {
                    (imbalance / assets).abs() * 100.0
                } else {
                    0.0
                };

                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "Balance sheet does not articulate in {pid}: \
                         assets ({assets:.2}) != liabilities ({liabilities:.2}) + \
                         equity ({equity:.2}), imbalance = {imbalance:.2}"
                    ),
                    period: Some(*pid),
                    materiality: Some(Materiality {
                        absolute: imbalance.abs(),
                        relative_pct: relative,
                        reference_value: assets,
                        reference_label: "total_assets".to_string(),
                    }),
                    nodes: self
                        .assets_nodes
                        .iter()
                        .chain(&self.liabilities_nodes)
                        .chain(&self.equity_nodes)
                        .cloned()
                        .collect(),
                });
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
