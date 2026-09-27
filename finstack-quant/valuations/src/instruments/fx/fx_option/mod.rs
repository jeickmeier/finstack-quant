//! FX options using Garman-Kohlhagen (1983) model.
//!
//! Foreign exchange options provide the right to exchange one currency for
//! another at a predetermined exchange rate. The Garman-Kohlhagen model is
//! the market-standard adaptation of Black-Scholes for FX options.
//!
//! # FX Option Structure
//!
//! - **Call on FOR/DOM**: Right to buy foreign currency at strike K
//!   - Example: EUR call / USD put at strike 1.10
//!   - Payoff (in DOM): Notional_FOR × max(S_T - K, 0)
//!
//! - **Put on FOR/DOM**: Right to sell foreign currency at strike K
//!   - Example: EUR put / USD call at strike 1.10
//!   - Payoff (in DOM): Notional_FOR × max(K - S_T, 0)
//!
//! # Garman-Kohlhagen Model (1983)
//!
//! FX options are priced using a Black-Scholes variant with two interest rates:
//!
//! **FX Call (on foreign currency):**
//! ```text
//! C = S·e^(-r_f·T)·N(d₁) - K·e^(-r_d·T)·N(d₂)
//! ```
//!
//! **FX Put (on foreign currency):**
//! ```text
//! P = K·e^(-r_d·T)·N(-d₂) - S·e^(-r_f·T)·N(-d₁)
//! ```
//!
//! where:
//! ```text
//! d₁ = [ln(S/K) + (r_d - r_f + σ²/2)T] / (σ√T)
//! d₂ = d₁ - σ√T
//! S = spot FX rate (domestic per foreign)
//! K = strike FX rate
//! r_d = domestic interest rate
//! r_f = foreign interest rate
//! σ = FX volatility
//! T = time to expiration
//! ```
//!
//! # Key Insight
//!
//! Foreign currency acts like a "stock" that pays a continuous "dividend"
//! equal to the foreign interest rate r_f. This maps directly to the
//! Merton (1973) model with q = r_f.
//!
//! # Delta Conventions
//!
//! `FxOption::delta_convention` records the quoted venue convention and premium
//! currency. The metric surface keeps each hedge coordinate distinct:
//!
//! - `delta`: unadjusted spot delta
//! - `delta_forward`: unadjusted forward delta
//! - `delta_premium_adjusted_spot`: premium-adjusted spot delta
//! - `delta_premium_adjusted_forward`: premium-adjusted forward delta
//!
//! Base-currency premium changes both premium-adjusted metrics. Quote-currency
//! premium leaves them equal to their unadjusted spot/forward counterparts.
//!
//! # Academic References
//!
//! ## Primary Source
//!
//! - Garman, M. B., & Kohlhagen, S. W. (1983). "Foreign Currency Option Values."
//!   *Journal of International Money and Finance*, 2(3), 231-237.
//!   (Canonical FX option pricing model) `docs/REFERENCES.md#garman-kohlhagen-1983`
//!
//! ## Related Work
//!
//! - Biger, N., & Hull, J. (1983). "The Valuation of Currency Options."
//!   *Financial Management*, 12(1), 24-28.
//!   (Independent derivation of same model)
//!
//! - Reiner, E., & Rubinstein, M. (1991). "Unscrambling the Binary Code."
//!   *Risk Magazine*, 4(9), 75-83.
//!   (FX digital options) `docs/REFERENCES.md#reiner-rubinstein-1991`
//!
//! ## Market Practice
//!
//! - Wystup, U. (2006). *FX Options and Structured Products*. Wiley.
//!   (Comprehensive guide to FX option markets) `docs/REFERENCES.md#wystup-fx-options`
//!
//! - Clark, I. J. (2011). *Foreign Exchange Option Pricing: A Practitioner's Guide*.
//!   Wiley.
//!   (Delta conventions and smile interpolation) `docs/REFERENCES.md#clark-fx-options`
//!
//! # Implementation Notes
//!
//! - **European options only**: Uses analytical Garman-Kohlhagen formula
//! - American and Bermudan exercise styles are **not supported** and will return an error
//! - Spot, forward, and premium-adjusted delta conventions are exposed separately
//! - Volatility surface interpolation via SABR when available
//!
//! # Examples
//!
//! See [`FxOption`] for construction and usage examples.
//!
//! # See Also
//!
//! - [`FxOption`] for FX option struct
//! - `pricer` for Garman-Kohlhagen pricing calculations
//! - `FxOptionGreeks` for Greeks computation
//! - FX option metrics module for risk metrics

/// FX option risk metrics (delta, gamma, vega, theta, rho)
pub(crate) mod metrics;
/// FX option pricer implementation using Black-Scholes FX model
pub(crate) mod pricer;
mod types;

pub use crate::instruments::common_impl::parameters::FxUnderlyingParams;
pub use types::{FxDeltaConvention, FxDeltaConventionKind, FxOption, FxOptionBuilder};
