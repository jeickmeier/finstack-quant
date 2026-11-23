//! Agency MBS risk metrics.
//!
//! This module provides MBS-specific risk metrics including:
//!
//! - **OAS (Option-Adjusted Spread)**: Spread over risk-free that equates
//!   model price to market price
//! - **Effective Duration**: Duration accounting for prepayment sensitivity
//! - **Effective Convexity**: Convexity accounting for prepayment sensitivity
//! - **Bucketed DV01**: Key-rate bucketed interest-rate sensitivities via
//!   the generic `UnifiedDv01Calculator` with triangular key-rate config.

#[doc(hidden)]
pub mod duration;
pub(crate) mod mc_oas;
mod register;
#[cfg(test)]
mod tests;

pub(crate) use mc_oas::{McOasConfig, calculate_mc_oas};
#[cfg(test)]
use register::OasCalculator;
pub(crate) use register::{PREPAY_RATE_SENSITIVITY, register_mbs_passthrough_metrics};
