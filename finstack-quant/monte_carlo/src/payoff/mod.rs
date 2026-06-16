//! Payoff definitions for Monte Carlo pricing.
//!
//! Start with [`vanilla`] for European call / put, digital, and forward-style
//! payoffs. This module also includes path-dependent payoffs such as Asian,
//! barrier, basket, and lookback contracts.
//!
//! All payoffs return [`finstack_quant_core::money::Money`] for currency safety and
//! are evaluated on a mutable [`crate::traits::PathState`], which lets them
//! inspect named state variables and record path-level cashflows.

pub mod asian;
pub mod barrier;
pub mod basket;
pub mod lookback;
pub mod vanilla;

/// Read a named payoff input, failing loudly when it is missing or non-finite.
///
/// A missing state key is a process/payoff wiring bug (wrong state key, wrong
/// `num_assets`, a process that does not populate `SPOT`, or a payoff grid
/// that does not match the engine grid). Silently defaulting to `0.0` turns
/// that bug into a systematically wrong price — puts paying full strike,
/// down-barriers knocking out at step 0, worst-of baskets pinned to zero —
/// so payoffs fail at the first affected event instead.
///
/// # Panics
///
/// Panics when `value` is `None` or non-finite.
pub(crate) fn require_finite_state(value: Option<f64>, key: &str, step: usize) -> f64 {
    let v = value.unwrap_or(f64::NAN);
    assert!(
        v.is_finite(),
        "payoff input '{key}' missing or non-finite at step {step}: \
         process/payoff wiring mismatch, diverged process state, or payoff \
         grid not matching the engine time grid"
    );
    v
}

pub use basket::{margrabe_exchange_option, BasketCall, BasketPut, BasketType, ExchangeOption};
pub use vanilla::{Digital, EuropeanCall, EuropeanPut, Forward};
