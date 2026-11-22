//! Fixtures shared across this crate's integration tests. `revolver_draws` is
//! also `#[path]`-included by the structured-credit crate's tests, and `trs` is
//! `#[path]`-included by the umbrella crate's cross-family TRS tests.

pub mod revolver_draws;
pub mod trs;
pub mod trs_market;
