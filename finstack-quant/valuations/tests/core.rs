//! Framework tests that exercise the generic pricing and metric machinery
//! over built-in instruments. The machinery lives in the valuations core;
//! these tests need the instrument families and therefore run here.

#[path = "core/breakeven.rs"]
mod breakeven;
#[path = "core/cross_factor.rs"]
mod cross_factor;
#[path = "core/dv01.rs"]
mod dv01;
#[path = "core/enrichment.rs"]
mod enrichment;
#[path = "core/fd_greeks.rs"]
mod fd_greeks;
#[path = "core/generic_pricer.rs"]
mod generic_pricer;
#[path = "core/hvar.rs"]
mod hvar;
#[path = "core/registry.rs"]
mod registry;
#[path = "core/risk_factors.rs"]
mod risk_factors;
#[path = "core/theta.rs"]
mod theta;
#[path = "core/var_calculator.rs"]
mod var_calculator;
