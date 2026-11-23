//! Cross-statement reconciliation checks.
//!
//! These checks verify that values reported on different statements (income
//! statement, balance sheet, cash flow statement) are mutually consistent.

pub(crate) mod capex;
pub(crate) mod depreciation;
pub(crate) mod dividends;
pub(crate) mod interest;

pub(crate) use capex::CapexReconciliation;
pub(crate) use depreciation::DepreciationReconciliation;
pub(crate) use dividends::DividendReconciliation;
pub(crate) use interest::InterestExpenseReconciliation;
