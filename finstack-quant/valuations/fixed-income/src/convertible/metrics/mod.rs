//! Convertible Bond metrics module.
//!
//! Provides metric calculators specific to `ConvertibleBond`, split into
//! focused files. The calculators compose with the shared metrics framework
//! and are registered via `register_convertible_metrics`.
//!
//! Exposed metrics:
//! - Parity
//! - Conversion premium
//! - Conversion value (absolute)
//! - Accrued interest
//! - Clean price
//! - Greeks: Delta, Gamma, Vega, Rho, Theta

mod accrued_interest;
mod conversion01;
mod cs01;
mod dividend_risk;
mod greeks;
mod implied_vol;
mod oas;
mod register;

use register::quoted_dirty_target;
pub(crate) use register::register_convertible_metrics;
