//! Balance sheet articulation check.

use serde::{Deserialize, Serialize};

use crate::Result;
use crate::checks::identity::{
    Comparison, Term, coverage_finding, read_cells, require_known_distinct,
};
use crate::checks::{Check, CheckCategory, CheckContext, CheckResult};
use crate::error::Error;
use crate::types::NodeId;

/// Verifies that Assets = Liabilities + Equity at every model period close.
///
/// Each group names known nodes, with no identifier repeated within a group
/// or shared between groups: the caller includes each account once, through
/// its subtotal or its components. All operands have compatible units and
/// one currency; the check performs no FX conversion. Signed balances and
/// typed zeros are used as they are.
///
/// Monetary balances are summed exactly and scalar balances with compensated
/// sums. The signed difference is `assets - (liabilities + equity)`; the
/// relative reference of the tolerance is `|assets|`. A period with a
/// missing balance is incomplete and names the nodes without a value; the
/// check passes only when every period is complete and within tolerance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct BalanceSheetArticulation {
    /// Node IDs whose values represent total assets.
    pub assets_nodes: Vec<NodeId>,
    /// Node IDs whose values represent total liabilities.
    pub liabilities_nodes: Vec<NodeId>,
    /// Node IDs whose values represent total equity.
    pub equity_nodes: Vec<NodeId>,
    /// Absolute tolerance override in the balances' units; replaces both
    /// terms of the suite's
    /// [`CheckConfig`](crate::checks::CheckConfig) allowance.
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
        let groups: [(&str, &[NodeId]); 3] = [
            ("assets_nodes", &self.assets_nodes),
            ("liabilities_nodes", &self.liabilities_nodes),
            ("equity_nodes", &self.equity_nodes),
        ];
        // An empty group would sum to zero and make the identity vacuous.
        for (label, group) in groups {
            if group.is_empty() {
                return Err(Error::invalid_input(format!(
                    "balance_sheet_articulation: node group '{label}' is empty; the identity \
                     Assets = Liabilities + Equity cannot be evaluated against an empty side"
                )));
            }
        }
        require_known_distinct(self.id(), context, &groups)?;
        crate::checks::helpers::validate_identity_inputs(
            self.id(),
            context,
            self.tolerance,
            self.assets_nodes
                .iter()
                .chain(&self.liabilities_nodes)
                .chain(&self.equity_nodes),
        )?;

        let all_nodes = || -> Vec<NodeId> {
            self.assets_nodes
                .iter()
                .chain(&self.liabilities_nodes)
                .chain(&self.equity_nodes)
                .cloned()
                .collect()
        };
        let mut findings = Vec::new();
        let mut comparisons = Vec::new();

        for period in context.periods() {
            let pid = period.id;
            let operands: Vec<&NodeId> = self
                .assets_nodes
                .iter()
                .chain(&self.liabilities_nodes)
                .chain(&self.equity_nodes)
                .collect();
            let values = match read_cells(self.id(), context, &operands, &pid)? {
                Ok(values) => values,
                Err(missing) => {
                    findings.push(coverage_finding(
                        self.id(),
                        "Balance sheet articulation",
                        pid,
                        missing,
                    ));
                    continue;
                }
            };
            let (assets, claims) = values.split_at(self.assets_nodes.len());
            let assets: Vec<Term> = assets.iter().copied().map(Term::plus).collect();
            let claims: Vec<Term> = claims.iter().copied().map(Term::plus).collect();
            let reference = crate::checks::identity::signed_sum(self.id(), &assets)?.abs();
            let (comparison, finding) = Comparison {
                check_id: self.id(),
                identity: "balance_sheet_articulation",
                period: pid,
                actual: &assets,
                expected: &claims,
                reference,
                reference_label: "total_assets",
                tolerance: self.tolerance,
                nodes: all_nodes(),
            }
            .evaluate(context, |assets, claims, difference| {
                format!(
                    "Balance sheet does not articulate in {pid}: assets ({assets:.2}) != \
                     liabilities + equity ({claims:.2}), difference = {difference:.2}"
                )
            })?;
            comparisons.push(comparison);
            findings.extend(finding);
        }

        let evaluated = context.periods().len();
        Ok(CheckResult::new(self, evaluated, findings, comparisons))
    }
}
