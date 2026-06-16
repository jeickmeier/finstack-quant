//! Cross-statement reconciliation checks.
//!
//! These checks verify that values reported on different statements (income
//! statement, balance sheet, cash flow statement) are mutually consistent.

mod capex;
mod depreciation;
mod dividends;
mod interest;

pub use capex::CapexReconciliation;
pub use depreciation::DepreciationReconciliation;
pub use dividends::DividendReconciliation;
pub use interest::InterestExpenseReconciliation;
