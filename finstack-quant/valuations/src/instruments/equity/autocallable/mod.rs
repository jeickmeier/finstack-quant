//! Autocallable structured notes with Monte Carlo pricing.
//!
//! Autocallables (reverse convertibles, phoenix autocalls) automatically redeem
//! early if the underlying exceeds barrier levels on observation dates. Popular
//! structured products combining high coupons with downside participation.
//!
//! # Autocallable Structure
//!
//! - **Observation dates**: Regular schedule (monthly, quarterly)
//! - **Autocall barrier**: Early redemption if S > Barrier
//! - **Protection barrier**: Capital protection level at maturity
//!
//! Typical observation treatment:
//! - If the coupon barrier is met, pay the current coupon plus any remembered
//!   coupons on its contractual payment date.
//! - If the autocall barrier is met, redeem principal on that payment date and stop.
//! - Otherwise continue to the next observation.
//!
//! At maturity (if not called):
//! - If S_T ≥ Protection Barrier: Repay par
//! - Else: Lose (Protection - S_T)/S_0 (downside participation)
//!
//! # Pricing Method
//!
//! Autocallables require Monte Carlo simulation due to:
//! - Path dependency (early redemption feature)
//! - Discrete observation dates
//! - Complex conditional payoffs
//!
//! No closed-form solutions exist.
//!
//! # Market Usage
//!
//! Popular underlyings:
//! - **Single stocks**: Large-cap, liquid names
//! - **Indices**: S&P 500, Euro Stoxx 50
//!
//! # References
//!
//! - Overhaus, M., Bermudez, A., Buehler, H., Ferraris, A., Jordinson, C., &
//!   Lamnouar, A. (2007). *Equity Derivatives: Theory and Applications*. Wiley.
//!   Chapter 6: Autocallables. `docs/REFERENCES.md#overhaus-2007-equity-derivatives`
//!
//! # See Also
//!
//! - [`Autocallable`] for instrument struct
//! - [`FinalPayoffType`] for maturity payoff specification
//! - Monte Carlo pricer for path-dependent pricing

pub(crate) mod metrics;
pub(crate) mod monte_carlo;
pub(crate) mod pricer;
pub(crate) mod types;

pub use types::{Autocallable, FinalPayoffType};
