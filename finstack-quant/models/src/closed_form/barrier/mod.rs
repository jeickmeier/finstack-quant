//! Analytical formulas for barrier options with continuous monitoring.
//!
//! Provides closed-form pricing formulas for European barrier options that
//! knock in or out when the underlying asset price crosses a barrier level.
//! These formulas assume continuous barrier monitoring and serve as:
//! 1. **Validation benchmarks** for discrete barrier adjustments
//! 2. **Production pricing** when continuous monitoring is appropriate
//!
//! # Barrier Option Types
//!
//! - **Up-and-In**: Activated when spot rises above barrier (S > H)
//! - **Up-and-Out**: Deactivated when spot rises above barrier
//! - **Down-and-In**: Activated when spot falls below barrier (S < H)
//! - **Down-and-Out**: Deactivated when spot falls below barrier
//!
//! # Mathematical Foundation
//!
//! Barrier options are priced using the **reflection principle** applied to
//! geometric Brownian motion. The key insight is that barrier crossing
//! probabilities can be computed analytically using mirror image arguments.
//!
//! ## General Formula (Reiner-Rubinstein 1991)
//!
//! The price decomposes into combinations of vanilla options and
//! barrier-adjusted terms involving powers of (H/S) where H is the barrier.
//!
//! For a down-and-out call:
//! ```text
//! C_do = C_vanilla - C_knock_in
//!      = S·e^(-qT)·N(x) - K·e^(-rT)·N(x - σ√T)
//!        - (H/S)^(2λ) · [H·e^(-qT)·N(y) - K·e^(-rT)·N(y - σ√T)]
//!   ```
//!
//! where λ = (r - q + σ²/2) / σ² and x, y are appropriately defined d-parameters.
//!
//! # Conventions
//!
//! | Parameter | Convention | Units |
//! |-----------|-----------|-------|
//! | Rates (r, q) | Continuously compounded | Decimal (0.05 = 5%) |
//! | Volatility (σ) | Annualized | Decimal (0.20 = 20%) |
//! | Time (T) | ACT/365-style | Years (1.0 = 1 year) |
//! | Prices / Barrier (H) | Per unit of underlying | Currency units |
//! | Greeks (vega, rho) | Per 1% move | Scaled by 0.01 |
//!
//! # Discrete Monitoring Corrections
//!
//! Real-world barriers are monitored discretely (e.g., daily closes), not continuously.
//! Continuous barrier formulas **underestimate** discrete barrier option values.
//!
//! Common corrections:
//! - **Broadie-Glasserman-Kou (1997)**: Adjust barrier by factor exp(±0.5826σ√Δt)
//! - **Gobet (2000)**: Higher-order correction using Brownian bridge
//! - **Rule of thumb**: H_adj = H · exp(±0.5826σ√Δt) where Δt is monitoring frequency
//!
//! # Academic References
//!
//! ## Primary Sources
//!
//! - Reiner, E., & Rubinstein, M. (1991). "Breaking Down the Barriers."
//!   *Risk Magazine*, 4(8), 28-35.
//!   (Canonical formulas for all 8 barrier option types) `docs/REFERENCES.md#reiner-rubinstein-1991`
//!
//! - Merton, R. C. (1973). "Theory of Rational Option Pricing."
//!   *Bell Journal of Economics and Management Science*, 4(1), 141-183.
//!   (Foundational work including barrier option theory) `docs/REFERENCES.md#merton-1973`
//!
//! ## Discrete Monitoring Corrections
//!
//! - Broadie, M., Glasserman, P., & Kou, S. G. (1997). "A Continuity Correction
//!   for Discrete Barrier Options." *Mathematical Finance*, 7(4), 325-349. `docs/REFERENCES.md#glasserman-2004-monte-carlo` `docs/REFERENCES.md#broadie-glasserman-kou-1997`
//!
//! - Gobet, E. (2000). "Weak Approximation of Killed Diffusion Using Euler Schemes."
//!   *Stochastic Processes and their Applications*, 87(2), 167-197.
//!
//! - Fusai, G., & Recchioni, M. C. (2007). "Analysis of Quadrature Methods for
//!   Pricing Discrete Barrier Options." *Journal of Economic Dynamics and Control*,
//!   31(3), 826-860.
//!
//! ## Reference Texts
//!
//! - Haug, E. G. (2007). *The Complete Guide to Option Pricing Formulas* (2nd ed.).
//!   McGraw-Hill. Chapter 4: Barrier Options. `docs/REFERENCES.md#haug-2007-option-formulas`
//!
//! - Wilmott, P., Howison, S., & Dewynne, J. (1995). *The Mathematics of Financial
//!   Derivatives*. Cambridge University Press. Chapter 8.
//!
//! # Implementation Notes
//!
//! - Formulas are numerically stable for typical parameter ranges
//! - Public wrappers return finite values for `time <= 0` and `vol <= 0`
//! - Zero-vol and near-zero-vol touch probability paths fall back to the deterministic limit
//! - `time <= 0` helper behavior uses the current terminal spot as a convenience convention;
//!   realized expired barrier state still requires observed path history at the instrument layer
//! - Knock-out rebates support explicit [`PayoutTiming::AtHit`] and [`PayoutTiming::AtExpiry`]; knock-in (no-hit) rebates support only `AtExpiry`
//! - For discrete monitoring in production, apply Broadie-Glasserman-Kou correction
//!
//! # Examples
//!
//! ## Down-and-Out and Down-and-In Calls
//!
//! ```
//! use finstack_quant_core::types::BarrierType;
//! use finstack_quant_models::closed_form::{barrier_price, BarrierParams};
//! use finstack_quant_models::types::OptionType;
//!
//! let spot = 100.0;
//! let strike = 100.0;
//! let barrier = 90.0;    // Barrier below current spot
//! let time = 1.0;
//! let rate = 0.05;
//! let div_yield = 0.02;
//! let vol = 0.20;
//! let params = BarrierParams::new(spot, strike, barrier, time, rate, div_yield, vol);
//!
//! // Price should be less than vanilla call (knockout feature reduces value)
//! let knock_out = barrier_price(&params, BarrierType::DownAndOut, OptionType::Call);
//! assert!(knock_out >= 0.0);
//!
//! // The knock-in only activates if spot falls to 90
//! let knock_in = barrier_price(&params, BarrierType::DownAndIn, OptionType::Call);
//! assert!(knock_in >= 0.0);
//! ```
//!
//! # See Also
//!
//! - [`BarrierType`] for barrier option classification
//! - [`BarrierParams`] for parameter grouping
//! - Monte Carlo barrier pricing for discrete monitoring and exotic payoffs

pub(crate) mod pricing;
#[cfg(test)]
mod tests;

pub(crate) use pricing::{BarrierParams, barrier_price};
