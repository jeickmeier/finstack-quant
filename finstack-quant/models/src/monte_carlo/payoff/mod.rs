//! Payoff definitions for Monte Carlo pricing.
//!
//! Start with [`vanilla`] for European call / put, digital, and forward-style
//! payoffs. This module also includes path-dependent payoffs such as Asian,
//! barrier, and lookback contracts.
//!
//! All payoffs return [`finstack_quant_core::money::Money`] for currency safety and
//! are evaluated on a mutable [`crate::monte_carlo::traits::PathState`], which lets them
//! inspect named state variables and record path-level cashflows.

pub mod asian;
pub mod barrier;
pub mod lookback;
pub mod vanilla;

/// Read a named payoff input, returning an error when it is missing or non-finite.
///
/// A missing state key is a process/payoff wiring bug (wrong state key, wrong
/// `num_assets`, a process that does not populate `SPOT`, or a payoff grid
/// that does not match the engine grid). Silently defaulting to `0.0` turns
/// that bug into a systematically wrong price, so the simulation fails at the
/// first affected event.
pub(crate) fn require_finite_state(
    value: Option<f64>,
    key: &str,
    step: usize,
) -> finstack_quant_core::Result<f64> {
    let value = value.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "payoff input '{key}' missing at step {step}: process/payoff wiring mismatch"
        ))
    })?;
    if !value.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "payoff input '{key}' non-finite at step {step}: diverged process state"
        )));
    }
    Ok(value)
}

pub use vanilla::EuropeanCall;
