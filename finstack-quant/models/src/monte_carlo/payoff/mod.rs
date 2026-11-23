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
pub mod autocallable;
pub mod barrier;
pub mod cliquet;
pub mod lookback;
pub mod range_accrual;
mod state_check;
pub mod vanilla;

pub(crate) use state_check::require_finite_state;
