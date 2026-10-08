//! Bachelier (Normal) model helpers.
//!
//! The Bachelier model assumes the underlying asset follows a normal distribution
//! (arithmetic Brownian motion), allowing for negative rates. This is the standard
//! model for interest rate options in many markets.
//!
//! # Pricing Formulas
//!
//! ```text
//! Call = A * [ (F - K) * N(d) + σ * √T * n(d) ]
//! Put  = A * [ (K - F) * N(-d) + σ * √T * n(d) ]
//!
//! where d = (F - K) / (σ * √T)
//!       A = annuity (discount factor × year fraction sum)
//! ```
//!
//! # Use Cases
//!
//! - Swaptions with normal volatility quoting
//! - Caps/floors in negative rate environments
//! - Interest rate options generally

use crate::closed_form::volatility::{bachelier_call, bachelier_put};

/// Bachelier (Normal) model price scaled by an annuity (unchecked).
///
/// The canonical checked unit-annuity price the host bindings expose is
/// [`crate::closed_form::bachelier_price`]; this variant multiplies by a
/// caller-supplied annuity and collapses degenerate inputs to intrinsic value.
///
/// # Arguments
/// * `option_type` - Call (payer) or Put (receiver)
/// * `forward` - Forward rate or price at expiry, in the same units as the
///   strike and normal volatility.
/// * `strike` - Exercise rate or price in the same units as `forward`.
/// * `sigma` - Normal volatility (in rate terms, not percentage)
/// * `t` - Time to expiry in years
/// * `annuity` - Present value of 1bp running (sum of discount factors × accrual fractions)
///
/// # Returns
/// Option premium in the same units as annuity (typically currency units)
#[inline]
#[must_use]
pub fn bachelier_price_with_annuity(
    option_type: crate::types::OptionType,
    forward: f64,
    strike: f64,
    sigma: f64,
    t: f64,
    annuity: f64,
) -> f64 {
    // Degenerate inputs (`t <= 0` or `sigma <= 0`) collapse to intrinsic value
    // inside the unit-annuity kernels.
    let unit_price = match option_type {
        crate::types::OptionType::Call => bachelier_call(forward, strike, sigma, t),
        crate::types::OptionType::Put => bachelier_put(forward, strike, sigma, t),
    };
    annuity * unit_price
}
