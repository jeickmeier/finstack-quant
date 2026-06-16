//! Working-capital consistency check.

use serde::{Deserialize, Serialize};

use super::super::{get_node_value, sum_nodes};
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, Severity,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::Result;

/// Verifies that the change in working capital on the cash flow statement
/// equals the negative delta of net working capital on the balance sheet.
///
/// NWC = Σ current_assets − Σ current_liabilities.
/// Expected WC change on CFS = −(NWC_t − NWC_{t−1}).
///
/// Skips the first period because no prior balance is available.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingCapitalConsistency {
    /// Working-capital change node from the cash flow statement.
    pub wc_change_cf_node: NodeId,
    /// Current-asset nodes from the balance sheet.
    pub current_assets_nodes: Vec<NodeId>,
    /// Current-liability nodes from the balance sheet.
    pub current_liabilities_nodes: Vec<NodeId>,
    /// Tolerance override; falls back to
    /// [`finstack_quant_statements::checks::CheckConfig::default_tolerance`].
    pub tolerance: Option<f64>,
}

impl Check for WorkingCapitalConsistency {
    fn id(&self) -> &str {
        "working_capital_consistency"
    }

    fn name(&self) -> &str {
        "Working Capital Consistency"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::InternalConsistency
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let tolerance = self.tolerance.unwrap_or(context.config.default_tolerance);
        let mut findings = Vec::new();
        let periods = &context.model.periods;

        for i in 1..periods.len() {
            let prev_pid = &periods[i - 1].id;
            let curr_pid = &periods[i].id;

            let Some(wc_cf) = get_node_value(context.results, &self.wc_change_cf_node, curr_pid)
            else {
                continue;
            };

            let ca_prev = sum_nodes(context.results, &self.current_assets_nodes, prev_pid);
            let cl_prev = sum_nodes(context.results, &self.current_liabilities_nodes, prev_pid);
            let nwc_prev = ca_prev - cl_prev;

            let ca_curr = sum_nodes(context.results, &self.current_assets_nodes, curr_pid);
            let cl_curr = sum_nodes(context.results, &self.current_liabilities_nodes, curr_pid);
            let nwc_curr = ca_curr - cl_curr;

            let expected_wc_change = -(nwc_curr - nwc_prev);
            let diff = wc_cf - expected_wc_change;

            if diff.abs() > tolerance {
                let reference = expected_wc_change.abs().max(1.0);
                let relative = (diff / reference).abs() * 100.0;

                let mut nodes = vec![self.wc_change_cf_node.clone()];
                nodes.extend(self.current_assets_nodes.iter().cloned());
                nodes.extend(self.current_liabilities_nodes.iter().cloned());

                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "WC change on CFS ({wc_cf:.2}) != −ΔNWC ({expected_wc_change:.2}) \
                         in {curr_pid}, difference = {diff:.2}"
                    ),
                    period: Some(*curr_pid),
                    materiality: Some(Materiality {
                        absolute: diff.abs(),
                        relative_pct: relative,
                        reference_value: expected_wc_change,
                        reference_label: "expected_wc_change".to_string(),
                    }),
                    nodes,
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
