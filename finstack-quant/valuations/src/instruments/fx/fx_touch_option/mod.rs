//! FX touch options (American binary options).
//!
//! Touch options pay a fixed amount if the spot rate touches (or doesn't touch)
//! a barrier level at any time before expiry.
//!
//! # Instrument Types
//!
//! - **One-touch**: pays if barrier is touched before expiry
//! - **No-touch**: pays if barrier is NOT touched before expiry
//!
//! # Pricing Model
//!
//! Uses Rubinstein & Reiner (1991) closed-form for continuous monitoring:
//!
//! ```text
//! P = e^{-r_d T} × payout × [(S/H)^{-(μ+λ)} × N(η·z) + (S/H)^{-(μ-λ)} × N(η·z')]
//! ```
//!
//! where:
//! - μ = (r_d - r_f - σ²/2) / σ²
//! - λ = sqrt(μ² + 2r_d/σ²)
//! - z = ln(H/S)/(σ√T) + λσ√T
//! - z' = ln(H/S)/(σ√T) - λσ√T
//! - η = +1 for down barrier, -1 for up barrier
//!
//! # Key Properties
//!
//! - One-touch + No-touch = discounted payout (put-call parity for touch options)
//! - Higher volatility increases one-touch value (more likely to hit barrier)
//! - Closer barrier increases one-touch value
//!
//! # References
//!
//! - Rubinstein, M., & Reiner, E. (1991). "Unscrambling the Binary Code."
//!   *Risk Magazine*, 4(9), 75-83.
//! - Wystup, U. (2006). *FX Options and Structured Products*. Wiley. Chapter 4.
//!
//! # See Also
//!
//! - [`FxTouchOption`] for the instrument struct
//! - `pricer::FxTouchOptionCalculator` for pricing calculations

/// FX touch option risk metrics
pub(crate) mod metrics;
/// FX touch option pricer implementation
pub(crate) mod pricer;
mod types;

pub use types::{BarrierDirection, FxTouchOption, PayoutTiming, TouchType};
