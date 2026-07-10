//! Two-clock plumbing for pricers that combine a vol-surface clock with
//! a discount-curve clock.
//!
//! # Background
//!
//! Many Monte Carlo and closed-form pricers in this crate use the
//! pattern:
//!
//! ```text
//! let t_vol = inst.day_count.year_fraction(as_of, expiry)?;
//! let df = disc_curve.df_between_dates(as_of, expiry)?;
//! let r_model = -df.ln() / t_vol;
//! ```
//!
//! The resulting rate is used as the drift in a GBM / Heston / barrier
//! simulation whose time horizon is `t_vol`. Consequently it must satisfy
//! `exp(-r_model * t_vol) = df`. Dividing by a year fraction on any other
//! clock breaks the forward/discount-factor identity when day counts differ.
//!
//! # The two-clock convention
//!
//! The fix is to thread **both** clocks through:
//!
//! * `t_vol` — year fraction on the **instrument / vol-surface** day
//!   count. Drives the time grid for MC simulation and vol-surface
//!   lookups (so that `σ²·t_vol` stays consistent with how the surface
//!   was stripped).
//! * `df` — the exact discount factor read from the curve. Applied
//!   directly to the final payoff, never back-computed from a rate.
//!
//! The model-clock drift is therefore `r_model = -ln(df) / t_vol`, while the
//! exact `df` discounts the payoff. This keeps both the simulated forward and
//! final discounting consistent with the same curve observation.
//!
//! # Migration status
//!
//! [`TwoClockParams`] centralizes this invariant for pricers that need both
//! curve-native and model-native time coordinates.
//!
//! # References
//!
//! - Hull, J.C. *Options, Futures, and Other Derivatives* (Ch. 15 on
//!   risk-neutral pricing under distinct rate and measurement
//!   conventions).

use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::term_structures::DiscountCurve;

/// Bundled two-clock pricing inputs.
///
/// See the module docs for the motivation. Construct via
/// [`TwoClockParams::from_curve_and_instrument`] whenever a pricer has
/// access to the discount curve and the instrument's day-count.
///
/// All fields are public so pricers can read them directly in the hot
/// loop without indirection.
#[derive(Debug, Clone, Copy)]
pub struct TwoClockParams {
    /// Year fraction from `as_of` to `expiry` on the **instrument's**
    /// (vol-surface) day-count convention. Drives the MC time grid and
    /// vol-surface lookups.
    pub t_vol: f64,
    /// Exact discount factor `P(as_of, expiry)` from the curve.
    pub df: f64,
}

impl TwoClockParams {
    /// Construct from the curve + instrument day-count + dates.
    ///
    /// Returns the model/volatility year fraction and the exact discount
    /// factor from the curve.
    ///
    /// # Errors
    ///
    /// Returns an error if the day-count computation or discount-factor
    /// lookup fails, or if the resulting DF is non-positive/non-finite.
    pub fn from_curve_and_instrument(
        disc_curve: &DiscountCurve,
        instrument_day_count: DayCount,
        as_of: Date,
        expiry: Date,
    ) -> finstack_quant_core::Result<Self> {
        let t_vol =
            instrument_day_count.year_fraction(as_of, expiry, DayCountContext::default())?;
        let df = disc_curve.df_between_dates(as_of, expiry)?;
        if !df.is_finite() || df <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "discount factor must be finite and positive, got {df}"
            )));
        }
        Ok(Self { t_vol, df })
    }

    /// Drift rate annualized on the stochastic model's time clock.
    ///
    /// For a positive model horizon this exactly satisfies
    /// `exp(-r_model * t_vol) = df`. A non-positive horizon represents an
    /// expired instrument and returns zero; constructors reject invalid DFs.
    #[inline]
    pub fn r_model(&self) -> f64 {
        if self.t_vol > 0.0 {
            -self.df.ln() / self.t_vol
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A non-positive model horizon is an expired contract with no drift.
    #[test]
    fn expired_model_clock_returns_zero_rate() {
        let p_zero_t = TwoClockParams {
            t_vol: 0.0,
            df: 0.95,
        };
        assert_eq!(p_zero_t.r_model(), 0.0);
    }

    #[test]
    fn model_rate_reproduces_exact_discount_factor() {
        let p = TwoClockParams {
            t_vol: 1.0,
            df: 0.951_229_424_500_714,
        };
        let recovered_df = (-p.r_model() * p.t_vol).exp();
        assert!((recovered_df - p.df).abs() < 1e-14);
    }
}
