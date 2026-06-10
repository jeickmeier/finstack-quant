//! SABR (Stochastic Alpha Beta Rho) volatility model implementation.
//!
//! The SABR model is widely used for pricing interest rate derivatives and FX options
//! with volatility smile. It provides closed-form approximations for implied volatility
//! that capture the smile and skew observed in market prices.
//!
//! # Accuracy Limitations
//!
//! This implementation uses the Hagan et al. (2002) expansion with the Obloj (2008)
//! correction applied to the z/χ(z) ratio. The correction replaces the difference-of-powers
//! moneyness with geometric-mean-based moneyness, reducing errors from O(ε²) to O(ε³)
//! for intermediate β values (0 < β < 1).
//!
//! Residual accuracy limitations (after Obloj correction):
//! - **T > 10Y**: Very long maturities may still show ~5-10bp error
//! - **ν (vol-of-vol) > 1.0**: Extreme vol-of-vol
//! - **Very far OTM strikes**: 3+ standard deviations from ATM
//!
//! References:
//! - Hagan, P. S., et al. (2002). "Managing Smile Risk." *Wilmott Magazine*.
//! - Obloj, J. (2008). "Fine-tune your smile: Correction to Hagan et al." arXiv:0708.0998v2
//!
//! # Conventions
//!
//! | Parameter | Convention | Units |
//! |-----------|-----------|-------|
//! | Forward (F) | Underlying forward rate/price | Decimal for rates, price units for equity |
//! | Strike (K) | Same units as forward | Decimal for rates, price units for equity |
//! | Alpha (α) | Initial stochastic vol | Same scale as F^β |
//! | Time (T) | Time to expiry | Years |
//! | Output | Implied volatility — lognormal (Black) for β>0, **normal (Bachelier)** for β≈0; see `SabrVolType` | Decimal (0.20 = 20%) / absolute rate units |

mod calibration;
mod model;
mod parameters;
mod smile;
#[cfg(test)]
mod tests;

pub(crate) use calibration::vega_weight;
pub use calibration::SABRCalibrator;
pub use model::{SABRModel, SabrVolType};
pub use parameters::SABRParameters;
pub use smile::{ArbitrageValidationResult, ButterflyViolation, MonotonicityViolation, SABRSmile};
