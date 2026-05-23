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

use finstack_core::math::{norm_cdf, norm_pdf};

/// Calculate d parameter for Bachelier model
///
/// d = (F - K) / (σ * √T)
///
/// # Edge Cases
/// - At expiration (t ≤ 0) or zero volatility: returns appropriate limit
#[inline]
#[must_use]
pub fn d_bachelier(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        // At expiration: d → ±∞ based on intrinsic value
        let intrinsic_sign = (forward - strike).signum();
        if intrinsic_sign > 0.0 {
            return f64::INFINITY;
        } else if intrinsic_sign < 0.0 {
            return f64::NEG_INFINITY;
        } else {
            return 0.0;
        }
    }
    (forward - strike) / (sigma * t.sqrt())
}

/// Bachelier (Normal) model price for a call/payer option
///
/// # Arguments
/// * `option_type` - Call (payer) or Put (receiver)
/// * `forward` - Forward rate
/// * `strike` - Strike rate
/// * `sigma` - Normal volatility (in rate terms, not percentage)
/// * `t` - Time to expiry in years
/// * `annuity` - Present value of 1bp running (sum of discount factors × accrual fractions)
///
/// # Returns
/// Option premium in the same units as annuity (typically currency units)
#[inline]
#[must_use]
pub fn bachelier_price(
    option_type: crate::instruments::common_impl::parameters::OptionType,
    forward: f64,
    strike: f64,
    sigma: f64,
    t: f64,
    annuity: f64,
) -> f64 {
    if t <= 0.0 {
        return match option_type {
            crate::instruments::common_impl::parameters::OptionType::Call => {
                (forward - strike).max(0.0) * annuity
            }
            crate::instruments::common_impl::parameters::OptionType::Put => {
                (strike - forward).max(0.0) * annuity
            }
        };
    }

    let d = d_bachelier(forward, strike, sigma, t);
    let disc_vol = sigma * t.sqrt();

    let term1 = (forward - strike) * norm_cdf(d);
    let term2 = disc_vol * norm_pdf(d);

    match option_type {
        crate::instruments::common_impl::parameters::OptionType::Call => annuity * (term1 + term2),
        crate::instruments::common_impl::parameters::OptionType::Put => {
            // Put-Call Parity or direct formula:
            // Put = Call - (F - K) * A
            //     = A * [(F-K)N(d) + v*n(d) - (F-K)]
            //     = A * [(F-K)(N(d)-1) + v*n(d)]
            //     = A * [(K-F)N(-d) + v*n(d)]
            // Since n(d) == n(-d)
            let term1_put = (strike - forward) * norm_cdf(-d);
            annuity * (term1_put + term2)
        }
    }
}
