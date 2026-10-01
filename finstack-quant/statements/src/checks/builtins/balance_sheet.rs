//! Balance sheet articulation check.

use serde::{Deserialize, Serialize};

use super::{get_finite_node_value, sum_nodes};
use crate::checks::types::effective_tolerance;
use crate::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
};
use crate::error::Error;
use crate::types::NodeId;
use crate::Result;

/// Verifies that Assets = Liabilities + Equity for every period.
///
/// All configured operands must have compatible scalar/monetary units and one
/// currency. Execution rejects incompatible units or invalid numeric tolerances;
/// it performs no FX conversion.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
        crate::checks::helpers::validate_identity_inputs(
            self.id(),
            context,
            self.tolerance,
            self.assets_nodes
                .iter()
                .chain(&self.liabilities_nodes)
                .chain(&self.equity_nodes),
        )?;
        // An empty node group would sum to zero and make the identity vacuous
        // (0 == 0 always "articulates"): the check would pass while checking
        // nothing. Fail loudly instead.
        for (label, group) in [
            ("assets_nodes", &self.assets_nodes),
            ("liabilities_nodes", &self.liabilities_nodes),
            ("equity_nodes", &self.equity_nodes),
        ] {
            if group.is_empty() {
                return Err(Error::invalid_input(format!(
                    "balance_sheet_articulation: node group '{label}' is empty; the identity \
                     Assets = Liabilities + Equity cannot be evaluated against an empty side"
                )));
            }
        }

        let mut findings = Vec::new();

        for period in &context.model.periods {
            let pid = &period.id;

            // Unresolvable operands must not flow into the identity: a
            // misspelled node silently understates one side (missing sums to
            // zero), and a NaN/Inf value makes the imbalance NaN, where
            // `NaN > tolerance` is false — either way a real imbalance can
            // flip to a false pass. Skip the period with a visible warning
            // instead, mirroring the missing-input handling in cash
            // reconciliation and retained earnings.
            let missing: Vec<String> = self
                .assets_nodes
                .iter()
                .chain(&self.liabilities_nodes)
                .chain(&self.equity_nodes)
                .filter(|n| get_finite_node_value(context.results, n, pid).is_none())
                .map(|n| n.to_string())
                .collect();
            if !missing.is_empty() {
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "Balance sheet articulation skipped for {pid}: missing or non-finite \
                         inputs [{}]. The identity cannot be evaluated with unresolved operands.",
                        missing.join(", ")
                    ),
                    period: Some(*pid),
                    materiality: None,
                    nodes: self
                        .assets_nodes
                        .iter()
                        .chain(&self.liabilities_nodes)
                        .chain(&self.equity_nodes)
                        .cloned()
                        .collect(),
                });
                continue;
            }

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
