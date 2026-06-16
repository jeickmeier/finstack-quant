//! Typed node-id mappings for common financial model structures.
//!
//! These mappings describe which node IDs correspond to which line items,
//! allowing the pre-built check suites in [`super::suites`] to wire up
//! structural and credit checks without hard-coding node names.

use finstack_quant_statements::types::NodeId;
use serde::{Deserialize, Serialize};

/// Maps node IDs for a three-statement financial model (income statement,
/// balance sheet, cash flow statement).
///
/// Required nodes must always be populated. Optional nodes (`Option<NodeId>`)
/// enable additional checks when present; the suite factory will skip the
/// corresponding check when the node is `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreeStatementMapping {
    /// Total-assets nodes (balance sheet).
    pub assets_nodes: Vec<NodeId>,
    /// Total-liabilities nodes (balance sheet).
    pub liabilities_nodes: Vec<NodeId>,
    /// Total-equity nodes (balance sheet).
    pub equity_nodes: Vec<NodeId>,
    /// Cash / cash-equivalents node (balance sheet).
    pub cash_node: NodeId,
    /// Retained earnings node (balance sheet).
    pub retained_earnings_node: NodeId,
    /// PP&E balance node (balance sheet).
    pub ppe_node: Option<NodeId>,
    /// Net income node (income statement).
    pub net_income_node: NodeId,
    /// Depreciation / amortization expense node (income statement).
    pub depreciation_node: Option<NodeId>,
    /// Interest expense node (income statement).
    pub interest_expense_node: Option<NodeId>,
    /// Income tax expense node (income statement).
    pub tax_expense_node: Option<NodeId>,
    /// Pre-tax income node (income statement).
    pub pretax_income_node: Option<NodeId>,
    /// Cash from operations node (cash flow statement).
    pub cfo_node: Option<NodeId>,
    /// Cash from investing node (cash flow statement).
    pub cfi_node: Option<NodeId>,
    /// Cash from financing node (cash flow statement).
    pub cff_node: Option<NodeId>,
    /// Total cash flow node (cash flow statement).
    pub total_cf_node: Option<NodeId>,
    /// Capital expenditures node (cash flow statement).
    pub capex_node: Option<NodeId>,
    /// Dividends paid node (cash flow statement / financing).
    pub dividends_node: Option<NodeId>,
}

impl ThreeStatementMapping {
    /// Collect all populated node IDs (required + optional) into a flat list.
    pub fn all_nodes(&self) -> Vec<NodeId> {
        let mut out: Vec<NodeId> = Vec::new();
        out.extend(self.assets_nodes.iter().cloned());
        out.extend(self.liabilities_nodes.iter().cloned());
        out.extend(self.equity_nodes.iter().cloned());
        out.push(self.cash_node.clone());
        out.push(self.retained_earnings_node.clone());
        out.push(self.net_income_node.clone());
        for n in [
            &self.ppe_node,
            &self.depreciation_node,
            &self.interest_expense_node,
            &self.tax_expense_node,
            &self.pretax_income_node,
            &self.cfo_node,
            &self.cfi_node,
            &self.cff_node,
            &self.total_cf_node,
            &self.capex_node,
            &self.dividends_node,
        ]
        .into_iter()
        .flatten()
        {
            out.push(n.clone());
        }
        out
    }
}

/// Maps node IDs for credit underwriting analysis.
///
/// Used by [`super::suites::credit_underwriting_checks`] to build leverage,
/// coverage, cash-flow, and trend checks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditMapping {
    /// Total debt node.
    pub debt_node: NodeId,
    /// EBITDA node.
    pub ebitda_node: NodeId,
    /// Interest expense node.
    pub interest_expense_node: NodeId,
    /// Free cash flow node.
    pub fcf_node: Option<NodeId>,
    /// Cash / liquidity node.
    pub cash_node: Option<NodeId>,
    /// Monthly cash burn node (for liquidity runway).
    pub cash_burn_node: Option<NodeId>,
    /// `(min, max)` leverage warning range; defaults to `(0.0, 6.0)`.
    pub leverage_warn: Option<(f64, f64)>,
    /// Minimum coverage ratio that triggers a warning; defaults to `1.5`.
    pub coverage_min_warn: Option<f64>,
}
