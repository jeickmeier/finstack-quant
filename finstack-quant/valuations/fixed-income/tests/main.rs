//! Integration tests for `finstack-quant-valuations-fixed-income`: one target so the crate links once.

mod support;

#[path = "bond/mod.rs"]
mod bond;
#[path = "bond_future/mod.rs"]
mod bond_future;
mod bond_portfolio;
mod contractual_accrual;
#[path = "convertible/mod.rs"]
mod convertible;
mod cross_factor_metrics_tests;
mod default_risk;
#[path = "fi_trs/mod.rs"]
mod fi_trs;
mod graceful_metrics_test;
#[path = "inflation_linked_bond/mod.rs"]
mod inflation_linked_bond;
mod principal_postings;
mod return_floor_example;
#[path = "revolving_credit/mod.rs"]
mod revolving_credit;
#[path = "term_loan/mod.rs"]
mod term_loan;

#[path = "performance/cashflow_accrual.rs"]
mod cashflow_accrual;
