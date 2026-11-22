//! Generic finite difference greek calculators for equity instruments.
//!
//! Provides reusable implementations of Delta, Gamma, and Vega calculators
//! that work with any instrument implementing the required traits.
//!
//! This eliminates code duplication across exotic options (AsianOption, Autocallable,
//! BarrierOption, LookbackOption, etc.) that all use the same finite difference pattern.
//!
//! # Numerical Stability
//!
//! These calculators implement guards against numerical instability:
//! - **Minimum bump size**: Absolute bump sizes are floored at [`MIN_ABSOLUTE_BUMP`] to
//!   prevent division by zero or explosive greeks at low spot levels.
//! - **Common random numbers (CRN)**: For Monte Carlo priced instruments, all bump
//!   scenarios use the same seed ("greeks_crn") to ensure variance reduction and
//!   stable finite differences.
//! - **Non-positive spot validation**: Returns an error if spot price is non-positive.

mod bump;
mod first_order;
mod second_order;
#[cfg(test)]
mod tests;

pub use bump::{MIN_ABSOLUTE_BUMP, clone_with_crn_seed_if_supported};
pub use first_order::{GenericFdDelta, GenericFdVega};
pub use second_order::{GenericFdGamma, GenericFdVanna, GenericFdVolga};

#[cfg(test)]
use bump::{CRN_SEED_SCENARIO, clone_with_crn_seed, effective_spot_bump, validate_spot};
