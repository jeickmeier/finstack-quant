//! Period-over-period growth rate plausibility check.

use serde::{Deserialize, Serialize};

use super::super::get_node_value;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Materiality, PeriodScope,
    Severity,
};
use finstack_quant_statements::types::NodeId;
use finstack_quant_statements::Result;

/// Flags line items whose period-over-period growth exceeds configurable
/// upper / lower bounds.
///
/// For each node, computes `(value_t − value_{t−1}) / |value_{t−1}|`.
/// Periods where the prior value is zero are skipped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowthRateConsistency {
    /// Nodes to monitor for growth-rate plausibility.
    pub nodes: Vec<NodeId>,
    /// Maximum positive growth rate as a decimal (e.g. 0.50 = 50 %).
    pub max_period_growth_pct: f64,
    /// Maximum negative growth rate as a decimal (e.g. −0.30 = −30 %).
    pub max_decline_pct: f64,
    /// Which periods to check.
    pub scope: PeriodScope,
}

impl Check for GrowthRateConsistency {
    fn id(&self) -> &str {
        "growth_rate_consistency"
    }

    fn name(&self) -> &str {
        "Growth Rate Consistency"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::InternalConsistency
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        let mut findings = Vec::new();
        let periods = &context.model.periods;

        for node in &self.nodes {
            for i in 1..periods.len() {
                let prev_spec = &periods[i - 1];
                let curr_spec = &periods[i];

                if !scope_matches(self.scope, curr_spec.is_actual) {
                    continue;
                }

                let prev_pid = &prev_spec.id;
                let curr_pid = &curr_spec.id;

                let Some(prev_val) = get_node_value(context.results, node, prev_pid) else {
                    continue;
                };
                let Some(curr_val) = get_node_value(context.results, node, curr_pid) else {
                    continue;
                };

                if prev_val.abs() < f64::EPSILON {
                    continue;
                }

                let growth = (curr_val - prev_val) / prev_val.abs();

                let flagged = growth > self.max_period_growth_pct || growth < self.max_decline_pct;

                if flagged {
                    let abs_change = (curr_val - prev_val).abs();

                    findings.push(CheckFinding {
                        check_id: self.id().to_string(),
                        severity: Severity::Warning,
                        message: format!(
                            "Node '{}' growth {:.1}% in {curr_pid} outside [{:.0}%, {:.0}%]",
                            node.as_str(),
                            growth * 100.0,
                            self.max_decline_pct * 100.0,
                            self.max_period_growth_pct * 100.0,
                        ),
                        period: Some(*curr_pid),
                        materiality: Some(Materiality {
                            absolute: abs_change,
                            relative_pct: growth.abs() * 100.0,
                            reference_value: prev_val,
                            reference_label: format!("prior_{}", node.as_str()),
                        }),
                        nodes: vec![node.clone()],
                    });
                }
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

/// Returns `true` if the period's actual/forecast status matches the scope.
fn scope_matches(scope: PeriodScope, is_actual: bool) -> bool {
    match scope {
        PeriodScope::AllPeriods => true,
        PeriodScope::ActualsOnly => is_actual,
        PeriodScope::ForecastOnly => !is_actual,
    }
}
