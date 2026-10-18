//! Instrument builders shared with the family crates that own them, included
//! by path: the IRS builder from the rates crate (deal hedge swaps) and the
//! stochastic revolver fixtures from the fixed-income crate (loan pools).

#[path = "../../../rates/tests/support/rates.rs"]
pub mod rates;
#[path = "../../../fixed-income/tests/support/revolver_draws.rs"]
pub mod revolver_draws;
