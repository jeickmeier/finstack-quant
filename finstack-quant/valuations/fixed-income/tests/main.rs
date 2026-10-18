//! Integration tests for `finstack-quant-valuations-fixed-income`: one target so the crate links once.

mod support;

#[path = "bond/mod.rs"]
mod bond;
#[path = "bond_future/mod.rs"]
mod bond_future;
#[path = "inflation_linked_bond/mod.rs"]
mod inflation_linked_bond;
#[path = "revolving_credit/mod.rs"]
mod revolving_credit;
#[path = "term_loan/mod.rs"]
mod term_loan;
