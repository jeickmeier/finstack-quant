//! Quanto option instruments with cross-currency adjustments.
//!
//! Quanto options provide exposure to a foreign asset with payoff settled
//! in domestic currency at a **fixed** exchange rate. The quanto adjustment
//! accounts for the correlation between asset and FX rate.
//!
//! # Quanto Feature
//!
//! For a quanto call on foreign asset with strike K:
//! - Underlying: Foreign asset with price S (in foreign currency)
//! - Payoff: Notional_DOM × max(S_T - K, 0) settled in domestic currency
//! - Fixed FX: **No FX risk** — the conversion rate is predetermined at trade inception
//!
//! This is distinct from a cross-currency option where payoff is converted at
//! the prevailing FX rate at expiry.
//!
//! # Quanto Adjustment
//!
//! The quanto drift adjustment modifies the forward price of the asset:
//!
//! ```text
//! Adjusted drift = r_foreign - q - ρ · σ_asset · σ_FX
//! ```
//!
//! where:
//! - r_foreign = foreign risk-free rate
//! - q = dividend yield
//! - ρ = correlation between asset and FX rate
//! - σ_asset = asset volatility
//! - σ_FX = FX rate volatility
//!
//! **Effect of correlation on call values:**
//! - Negative correlation (asset up ↔ FX down) **increases** the drift and call value
//! - Positive correlation (asset up ↔ FX up) **decreases** the drift and call value
//!
//! # Pricing Model
//!
//! Analytical Black-Scholes with quanto drift adjustment:
//! - Discount at domestic rate r_domestic
//! - Use quanto-adjusted forward: S × exp((r_foreign - q - ρ·σ_S·σ_FX) × T)
//! - Use asset volatility σ_asset
//!
//! **Note:** Only analytical pricing is supported. Monte Carlo pricing is
//! intentionally disabled because the payoff and drift parameterization
//! required for MC would differ materially from the analytical quanto model.
//!
//! # References
//!
//! - Garman, M. B., & Kohlhagen, S. W. (1983). "Foreign Currency Option Values."
//!   *Journal of International Money and Finance*, 2(3), 231-237.
//!
//! - Derman, E., Karasinski, P., & Wecker, J. (1990). "Understanding Guaranteed
//!   Exchange-Rate Contracts in Foreign Stock Investments." Goldman Sachs
//!   Quantitative Strategies Research Notes.
//!
//! # See Also
//!
//! - [`QuantoOption`] for instrument struct
//! - [`models::closed_form::quanto`](crate::models::closed_form::quanto) for formulas

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod types;

pub use types::QuantoOption;
