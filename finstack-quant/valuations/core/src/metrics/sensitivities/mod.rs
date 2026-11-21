//! Family-facing sensitivity calculators — DV01, CS01, finite-difference Greeks,
//! theta, carry and breakeven — and their bump configuration.
pub mod breakeven;
pub mod carry_decomposition;
pub mod config;
pub mod cross_factor;
pub mod cs01;
pub mod dv01;
pub mod fd_greeks;
pub mod fx01;
pub mod option_greeks;
pub mod recovery01;
pub mod theta;
