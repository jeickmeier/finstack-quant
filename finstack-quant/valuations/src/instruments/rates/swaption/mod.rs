//! Swaption instruments with Black (1976), Normal (Bachelier), and SABR volatility models.
//!
//! Swaptions are options on interest rate swaps, giving the holder the right
//! (but not obligation) to enter into a swap at a predetermined fixed rate.
//! They are key instruments for managing long-term interest rate exposure.
//!
//! # Swaption Types
//!
//! - **Payer swaption**: Right to enter payer swap (pay fixed, receive floating)
//!   - Benefits when rates rise (swap value becomes positive)
//!
//! - **Receiver swaption**: Right to enter receiver swap (receive fixed, pay floating)
//!   - Benefits when rates fall (swap value becomes positive)
//!
//! # Exercise Styles
//!
//! - **European**: Single exercise date
//! - **Bermudan**: Exercise on any coupon date in a window
//! - **American**: Exercise any time (rare in practice)
//!
//! # Settlement Types
//!
//! - **Physical**: Deliver the underlying swap upon exercise (uses Physical Annuity)
//! - **Cash**: Cash settlement based on swap present value (uses Par Yield Annuity)
//!
//! # Pricing Models
//!
//! ## Black (1976) - Lognormal
//!
//! European swaptions are priced using Black (1976) model for options on
//! forward swap rates. Requires positive rates.
//!
//! **Payer Swaption:**
//! ```text
//! V_payer = A(0,T) · [S · N(d₁) - K · N(d₂)]
//! ```
//!
//! ## Bachelier - Normal
//!
//! European swaptions priced using Normal model, suitable for negative rates.
//!
//! **Payer Swaption:**
//! ```text
//! V_payer = A(0,T) · [(S - K) · N(d) + σ√T · n(d)]
//! ```
//!
//! where:
//! ```text
//! d = (S - K) / (σ√T)
//! n(x) = standard normal PDF
//! N(x) = standard normal CDF
//! ```
//!
//! # SABR Volatility Interpolation
//!
//! Market swaption volatilities are typically quoted on a strike grid and
//! interpolated using the SABR stochastic volatility model (Hagan et al. 2002).
//!
//! # Market Conventions
//!
//! Standard swaption quoting conventions:
//!
//! - **USD**: 3M or 6M into 2Y, 5Y, 10Y, 30Y swaps
//! - **EUR**: 1Y, 2Y, 5Y, 10Y expiries into various tenors
//! - **Volatility**: Quoted as lognormal (Black) or normal (Bachelier)
//! - **Daycount**: Follow underlying swap conventions
//!
//! # References
//!
//! - Black, F. (1976). "The Pricing of Commodity Contracts." *Journal of
//!   Financial Economics*, 3(1-2), 167-179.
//!   (Black model extended to swaptions) `docs/REFERENCES.md#black-1976`
//!
//! - Hagan, P. S., Kumar, D., Lesniewski, A. S., & Woodward, D. E. (2002).
//!   "Managing Smile Risk." *Wilmott Magazine*, September, 84-108.
//!   (SABR model for volatility interpolation) `docs/REFERENCES.md#hagan-2002-sabr`
//!
//! - Rebonato, R. (2004). *Volatility and Correlation: The Perfect Hedger and
//!   the Fox* (2nd ed.). Wiley. Part II: Swaptions. `docs/REFERENCES.md#rebonato-2004-volatility-correlation`
//!
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*
//!   (2nd ed.). Springer. Chapter 13: Swaption Pricing. `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`
//!
//! # Implementation Notes
//!
//! - European swaptions use Black (1976) or Bachelier (Normal)
//! - Bermudan swaptions require tree-based or LSM pricing (stubbed)
//! - Volatility interpolation via SABR model when enabled
//! - Settlement conventions affect discount factor adjustments (Physical vs Cash Annuity)
//!
//! # Examples
//!
//! See [`Swaption`] for construction and usage examples.
//!
//! # See Also
//!
//! - [`crate::instruments::rates::swaption::Swaption`] for swaption instrument struct
//! - [`crate::instruments::ExerciseStyle`] for exercise style specification
//! - [`crate::instruments::SettlementType`] for settlement type
//! - swaption metrics module for risk metrics
//! - [`crate::instruments::rates::swaption::SimpleSwaptionBlackPricer`] for Black-76 pricing
//! - [`crate::instruments::rates::swaption::SimpleSwaptionNormalPricer`] for Bachelier pricing
//! - [`crate::instruments::VolatilityModel`] for selecting Black vs Normal

use finstack_quant_core::dates::Date;

/// Bermudan swaption pricing orchestration.
pub(crate) mod bermudan;
/// Hull-White 1-factor tree pricer for European swaptions
pub(crate) mod hw_pricer;
/// Bermudan swaption LMM structure construction and Monte Carlo pricer.
pub mod lmm_pricer;
/// Swaption risk metrics (delta, vega, theta, rho)
pub(crate) mod metrics;
/// Swaption parameters and market data extraction
pub(crate) mod parameters;
/// European swaption pricers for Black-76 and Bachelier normal models.
pub(crate) mod pricer;
/// Bermudan swaption pricing engines (tree, LSMC, LMM).
///
/// Crate-private: the tree valuator is re-exported as
/// [`BermudanSwaptionTreeValuator`]; the Monte Carlo engines
/// ([`pricing::monte_carlo_lsmc`], [`pricing::lmm_bermudan`]) are exercised by
/// no-arbitrage numéraire tests living in-crate under `#[cfg(test)]`, so the
/// engines need no public visibility.
pub(crate) mod pricing;
pub(crate) mod types;

pub use bermudan::{
    BermudanPricingMethod, BermudanSwaptionPricer, BermudanSwaptionPricerConfig,
    PreparedHullWhiteModel,
};
pub use parameters::SwaptionParams;
pub use pricer::{SimpleSwaptionBlackPricer, SimpleSwaptionNormalPricer};
pub use pricing::BermudanSwaptionTreeValuator;
pub use types::{
    BermudanSchedule, BermudanSwaption, BermudanType, CashSettlementMethod, GreekInputs, Swaption,
    SwaptionBuilder,
};

/// Convert effective swap dates to the canonical market tenor coordinate.
///
/// Swaption cubes are rectangular grids labeled by contractual month/year
/// tenors (for example, 2Y, 5Y, 10Y), not by day-count year fractions. Business
/// day adjustment and settlement lag can shorten or lengthen the actual accrual
/// interval by a few days without changing that market tenor label.
///
/// # Arguments
///
/// * `start` - Contractual effective date of the underlying swap.
/// * `end` - Contractual maturity date of the underlying swap.
///
/// # Errors
///
/// Returns a validation error when `end` is not after `start` or the dates do
/// not define a positive whole-month tenor.
pub fn contractual_swap_tenor_years(start: Date, end: Date) -> finstack_quant_core::Result<f64> {
    if end <= start {
        return Err(finstack_quant_core::Error::Validation(format!(
            "swaption underlying maturity {end} must be after effective start {start}"
        )));
    }
    let months = (end.year() - start.year()) * 12 + i32::from(end.month() as u8)
        - i32::from(start.month() as u8);
    if months <= 0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "swaption underlying dates {start} to {end} do not define a positive monthly tenor"
        )));
    }
    Ok(f64::from(months) / 12.0)
}
