//! Corkscrew analysis extension.
//!
//! Roll-forward validation for balance sheet accounts: opening + changes =
//! closing across periods.
//!
//! # Configuration Schema
//!
//! ```json
//! {
//!   "accounts": [
//!     {
//!       "node_id": "inventory_end",
//!       "account_type": "asset",
//!       "beginning_balance_node": "inventory_beg",
//!       "changes": ["additions"],
//!       "decreases": ["disposals"]
//!     },
//!     {
//!       "node_id": "debt",
//!       "account_type": "liability",
//!       "changes": ["debt_issuance"],
//!       "decreases": ["debt_repayment"]
//!     }
//!   ],
//!   "tolerance": 0.01
//! }
//! ```
//!
//! Identity: `expected = prev + Σ changes − Σ decreases` (or
//! `beginning + Σ changes − Σ decreases` when `beginning_balance_node` is
//! set). Decrease nodes stay **positive** outflows. Pair a roll-forward
//! template `{name}` as `node_id = "{name}_end"`,
//! `beginning_balance_node = "{name}_beg"`, `changes = increases`,
//! `decreases = disposals`.
//!
//! # Example Usage
//!
//! ```no_run
//! use finstack_quant_statements_analytics::extensions::{
//!     CorkscrewExtension, CorkscrewConfig, CorkscrewAccount, AccountType,
//! };
//! use finstack_quant_statements::evaluator::{Evaluator, StatementResult};
//! use finstack_quant_statements::types::FinancialModelSpec;
//!
//! # fn main() -> finstack_quant_statements::Result<()> {
//! # let model: FinancialModelSpec = unimplemented!("build a model");
//! let mut evaluator = Evaluator::new();
//! let results = evaluator.evaluate(&model)?;
//!
//! let config = CorkscrewConfig {
//!     accounts: vec![CorkscrewAccount {
//!         node_id: "cash".into(),
//!         account_type: AccountType::Asset,
//!         changes: vec!["cash_inflows".into()],
//!         decreases: vec!["cash_outflows".into()],
//!         beginning_balance_node: None,
//!     }],
//!     tolerance: 0.01,
//!     fail_on_error: false,
//! };
//!
//! let extension = CorkscrewExtension::new(config);
//! let report = extension.execute(&model, &results)?;
//! assert_eq!(report.status, finstack_quant_statements_analytics::extensions::CorkscrewStatus::Success);
//! # Ok(())
//! # }
//! ```

pub(crate) mod extension;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) use extension::{
    AccountType, CorkscrewAccount, CorkscrewConfig, CorkscrewExtension, CorkscrewStatus,
};
