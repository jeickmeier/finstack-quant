//! Family-facing sensitivity calculators — DV01, CS01, finite-difference Greeks,
//! vega, theta, carry and breakeven — and their bump configuration.
pub mod breakeven;
pub mod carry_decomposition;
pub mod config;
pub mod cross_factor;
pub mod cs01;
pub mod cs01_direct_hazard;
pub(crate) mod cs01_z_spread;
pub mod dv01;
pub mod fd_greeks;
pub mod fx01;
pub mod option_greeks;
pub(crate) mod rf_component_dv01;
pub mod theta;
pub(crate) mod vega;
