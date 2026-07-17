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
//!   (Black model extended to swaptions)
//!
//! - Hagan, P. S., Kumar, D., Lesniewski, A. S., & Woodward, D. E. (2002).
//!   "Managing Smile Risk." *Wilmott Magazine*, September, 84-108.
//!   (SABR model for volatility interpolation)
//!
//! - Rebonato, R. (2004). *Volatility and Correlation: The Perfect Hedger and
//!   the Fox* (2nd ed.). Wiley. Part II: Swaptions.
//!
//! - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models - Theory and Practice*
//!   (2nd ed.). Springer. Chapter 13: Swaption Pricing.
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
//! - [`crate::instruments::rates::swaption::SwaptionExercise`] for exercise style specification
//! - [`crate::instruments::rates::swaption::SwaptionSettlement`] for settlement type
//! - swaption metrics module for risk metrics
//! - [`crate::instruments::rates::swaption::SimpleSwaptionBlackPricer`] for Black model pricer
//! - [`crate::instruments::rates::swaption::VolatilityModel`] for selecting Black vs Normal

/// Bermudan swaption pricing orchestration.
pub(crate) mod bermudan;
/// Bermudan swaption pricer using Cheyette + rough stochastic volatility
pub(crate) mod cheyette_rough_pricer;
/// Hull-White 1-factor tree pricer for European swaptions
pub(crate) mod hw_pricer;
/// Bermudan swaption pricer using LMM/BGM Monte Carlo
pub(crate) mod lmm_pricer;
/// Swaption risk metrics (delta, vega, theta, rho)
pub(crate) mod metrics;
/// Swaption parameters and market data extraction
pub(crate) mod parameters;
/// Swaption pricer implementation using Black (1976) model
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

pub use crate::calibration::hull_white::HullWhiteParams;
pub use bermudan::{
    BermudanPricingMethod, BermudanSwaptionPricer, BermudanSwaptionPricerConfig,
    CalibratedHullWhiteModel,
};
pub use parameters::SwaptionParams;
pub use pricer::SimpleSwaptionBlackPricer;
pub use pricing::BermudanSwaptionTreeValuator;
pub use types::{
    BermudanSchedule, BermudanSwaption, BermudanType, CashSettlementMethod, GreekInputs,
    SABRParameters, Swaption, SwaptionExercise, SwaptionSettlement, VolatilityModel,
};

/// Build the HW1F surface-calibration input from the normalized fixed-leg tenor.
///
/// The calibration engine supports annual, semiannual, and quarterly swap
/// schedules. Month- and year-based spellings that represent the same period
/// are normalized; other tenors fail instead of silently calibrating as 6M.
pub(crate) fn hw1f_swaption_surface_calibration(
    surface_id: &str,
    max_expiry: Option<f64>,
    fixed_frequency: finstack_quant_core::dates::Tenor,
) -> finstack_quant_core::Result<
    crate::instruments::rates::exotics_shared::Hw1fSurfaceCalibration<'_>,
> {
    use crate::calibration::hull_white::SwapFrequency;

    let frequency = match fixed_frequency.months() {
        Some(12) => SwapFrequency::Annual,
        Some(6) => SwapFrequency::SemiAnnual,
        Some(3) => SwapFrequency::Quarterly,
        _ => {
            return Err(finstack_quant_core::Error::Validation(format!(
                "HW1F swaption surface calibration supports fixed-leg frequencies of 1Y, 6M, \
                 or 3M; got {fixed_frequency}"
            )))
        }
    };

    Ok(
        crate::instruments::rates::exotics_shared::Hw1fSurfaceCalibration::Swaption {
            surface_id,
            max_expiry,
            frequency,
        },
    )
}

#[cfg(test)]
mod calibration_frequency_tests {
    use super::*;
    use crate::calibration::hull_white::SwapFrequency;
    use crate::instruments::rates::exotics_shared::Hw1fSurfaceCalibration;
    use finstack_quant_core::dates::{Tenor, TenorUnit};

    fn frequency_for(tenor: Tenor) -> SwapFrequency {
        match hw1f_swaption_surface_calibration("VOL", Some(10.0), tenor)
            .expect("supported fixed-leg tenor")
        {
            Hw1fSurfaceCalibration::Swaption { frequency, .. } => frequency,
            Hw1fSurfaceCalibration::CapFloor { .. } => unreachable!("swaption helper variant"),
        }
    }

    #[test]
    fn hw1f_surface_calibration_uses_annual_fixed_leg_frequency() {
        assert_eq!(frequency_for(Tenor::annual()), SwapFrequency::Annual);
        assert_eq!(
            frequency_for(Tenor::new(12, TenorUnit::Months)),
            SwapFrequency::Annual
        );
    }

    #[test]
    fn hw1f_surface_calibration_uses_quarterly_fixed_leg_frequency() {
        assert_eq!(frequency_for(Tenor::quarterly()), SwapFrequency::Quarterly);
    }

    #[test]
    fn hw1f_surface_calibration_rejects_unsupported_fixed_leg_frequency() {
        let error = match hw1f_swaption_surface_calibration("VOL", None, Tenor::monthly()) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("monthly fixed-leg tenor must be unsupported"),
        };
        assert!(error.contains("got 1M"), "unexpected error: {error}");
    }
}
