//! Trend-deterioration check.

use serde::{Deserialize, Serialize};

use finstack_quant_statements::Result;
use finstack_quant_statements::checks::get_finite_node_value;
use finstack_quant_statements::checks::{
    Check, CheckCategory, CheckContext, CheckFinding, CheckResult, Severity,
};
use finstack_quant_statements::types::NodeId;

/// Direction in which a metric should ideally move.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TrendDirection {
    /// Higher values are better (e.g. EBITDA, coverage).
    IncreasingIsGood,
    /// Lower values are better (e.g. leverage, cost ratios).
    DecreasingIsGood,
}

/// Flags a metric that has been deteriorating for `lookback_periods`
/// consecutive periods.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct TrendCheck {
    /// Identifier of this check, unique within its suite (a suite may watch
    /// several nodes, one trend check each).
    pub id: String,
    /// Node to monitor.
    pub node: NodeId,
    /// Which direction is "good".
    pub direction: TrendDirection,
    /// Number of consecutive deteriorating periods before flagging.
    pub lookback_periods: usize,
    /// Severity to assign to the finding.
    pub severity: Severity,
}

impl Check for TrendCheck {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        "Trend"
    }

    fn category(&self) -> CheckCategory {
        CheckCategory::CreditReasonableness
    }

    fn execute(&self, context: &CheckContext) -> Result<CheckResult> {
        if self.lookback_periods == 0 {
            return Err(finstack_quant_statements::Error::eval(
                "TrendCheck lookback_periods must be positive".to_string(),
            ));
        }

        let mut findings = Vec::new();
        let periods = context.periods();
        let mut consecutive_bad: usize = 0;

        for i in 1..periods.len() {
            let prev_pid = &periods[i - 1].id;
            let curr_pid = &periods[i].id;

            // A period without a value breaks the streak and is reported as
            // a coverage gap; the gap is reported once, in its own period.
            let Some(curr_val) = crate::analysis::checks::required(
                context,
                self,
                &self.node,
                curr_pid,
                &mut findings,
            ) else {
                consecutive_bad = 0;
                continue;
            };
            let Some(prev_val) = get_finite_node_value(context.results, &self.node, prev_pid)
            else {
                consecutive_bad = 0;
                continue;
            };

            let deteriorating = match self.direction {
                TrendDirection::IncreasingIsGood => curr_val < prev_val,
                TrendDirection::DecreasingIsGood => curr_val > prev_val,
            };

            if deteriorating {
                consecutive_bad += 1;
            } else {
                consecutive_bad = 0;
            }

            if consecutive_bad >= self.lookback_periods {
                findings.push(CheckFinding {
                    check_id: self.id().to_string(),
                    severity: self.severity,
                    message: format!(
                        "'{}' deteriorating for {consecutive_bad} consecutive periods \
                         as of {curr_pid} (current = {curr_val:.2})",
                        self.node.as_str(),
                    ),
                    period: Some(*curr_pid),
                    materiality: None,
                    nodes: vec![self.node.clone()],
                    comparison: None,
                    coverage: false,
                });
            }
        }

        let evaluated = context.periods().len();
        Ok(CheckResult::new(self, evaluated, findings, Vec::new()))
    }
}
