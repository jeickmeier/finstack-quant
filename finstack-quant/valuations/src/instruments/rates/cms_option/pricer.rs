//! Convexity-adjusted Black pricer for CMS options.
//!
//! Implements the standard market model for CMS caps/floors:
//! 1. Calculate forward swap rate for each fixing.
//! 2. Apply convexity adjustment using Hagan (2003) methodology.
//! 3. Price the option on the adjusted rate using Black-76.
//!
//! # Convexity Adjustment
//!
//! The convexity adjustment accounts for the difference between the CMS rate
//! (which is a martingale under the payment measure) and the forward swap rate
//! (martingale under the annuity measure). Per Hagan (2003), the adjustment
//! depends on the annuity sensitivity to rate changes:
//!
//! ```text
//! CMS_Rate ≈ Forward_Swap_Rate + Convexity_Adjustment
//! Convexity_Adjustment = 0.5 * σ² * T * G(S)
//! where G(S) ≈ swap_tenor / (1 + S * swap_tenor)²
//! ```
//!
//! # Accuracy Limitations
//!
//! This pricer uses the simplified Hagan (2003) first-order convexity adjustment. It is
//! accurate for short-to-medium tenors (< 10Y) and moderate volatility. For long-dated
//! CMS (> 10Y) or high-volatility environments, a replication-based pricer is the
//! market-standard approach; this implementation does not provide that fallback and
//! should be treated as an approximation in those regimes.
//!
//! # Reference
//!
//! - Hagan, P. (2003). "Convexity Conundrums: Pricing CMS Swaps, Caps, and Floors."
//!   Wilmott Magazine, March, 38-44. `docs/REFERENCES.md#hagan-2003-cms-convexity`
//! - Hull, J. (2018). "Options, Futures, and Other Derivatives." `docs/REFERENCES.md#hull-options-futures`

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cms_common::par_annuity;
use crate::instruments::rates::cms_option::types::CmsOption;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_models::closed_form::{black_call, black_put};

/// Convexity-adjusted Black pricer for CMS options.
pub struct CmsOptionPricer;

impl CmsOptionPricer {
    /// Create a new CMS option pricer.
    pub fn new() -> Self {
        Self
    }

    /// Internal pricing logic
    ///
    /// # Time Basis
    ///
    /// - Vol surface lookups use the instrument's day_count for time_to_fixing
    ///   (market convention for vol surfaces).
    /// - Discount factors use curve-consistent relative DFs via `relative_df_discount_curve`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Vol surface is not provided (required for CMS option pricing)
    /// - Forward swap rate is non-positive (would cause NaN in Black-76)
    pub(crate) fn price_internal_with_convexity(
        &self,
        inst: &CmsOption,
        curves: &MarketContext,
        as_of: Date,
        convexity_scale: f64,
    ) -> Result<Money> {
        use crate::instruments::common_impl::pricing::time::relative_df_discount_curve;

        inst.validate()?;
        let mut total_pv = 0.0;
        let strike = inst.strike_f64()?;
        let discount_curve = curves.get_discount(inst.discount_curve_id.as_ref())?;

        let vol_surface = curves.get_surface(inst.vol_surface_id.as_str())?;

        for (i, &fixing_date) in inst.fixing_dates.iter().enumerate() {
            let payment_date = inst.payment_dates[i];
            let accrual_fraction = inst.accrual_fractions[i];

            if payment_date <= as_of {
                continue; // Period expired
            }

            // Seasoned period: the CMS rate fixed in the past, so the option
            // payoff is pure intrinsic on the *recorded* fixing (mirroring the
            // cap/floor pricer) — never on a rate re-projected from the live
            // curve, which books phantom P&L.
            if fixing_date < as_of {
                let observed = crate::instruments::rates::hw1f::fixings::historical_cms_fixing(
                    curves,
                    &inst.forward_curve_id,
                    inst.cms_tenor,
                    fixing_date,
                )?;
                let option_val = match inst.option_type {
                    crate::instruments::OptionType::Call => (observed - strike).max(0.0),
                    crate::instruments::OptionType::Put => (strike - observed).max(0.0),
                };
                let df_pay =
                    relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;
                total_pv += option_val * accrual_fraction * df_pay;
                continue;
            }

            // 1. Calculate Forward Swap Rate
            let reference_swap = inst.reference_swap();
            let swap_start = reference_swap.reference_swap_start(fixing_date)?;
            let swap_tenor_months = (inst.cms_tenor * 12.0).round() as i32;
            let swap_end = swap_start.add_months(swap_tenor_months);

            // Calculate annuity and forward rate
            let (forward_swap_rate, _) =
                reference_swap.forward_rate_and_annuity(curves, as_of, swap_start, swap_end)?;

            if forward_swap_rate <= 0.0 || strike <= 0.0 {
                return Err(finstack_quant_core::Error::Validation("Black CMS option requires positive forward and strike; no normal-volatility conversion is inferred".to_owned()));
            }
            vol_surface.require_quote_type(
                finstack_quant_core::market_data::surfaces::VolQuoteType::BlackLognormal,
            )?;

            // 2. Calculate Convexity Adjustment
            // Time to fixing is calendar time for the vol-surface axis: ACT/365F.
            let time_to_fixing =
                DayCount::Act365F.year_fraction(as_of, fixing_date, DayCountContext::default())?;

            // Get volatility from surface.
            //
            // ASSUMPTION: `vol_surface` must be the swaption volatility surface
            // for the CMS reference swap tenor (`inst.cms_tenor`), keyed by
            // (expiry, strike). The surface has no separate swap-tenor axis, so
            // the caller is responsible for supplying the surface that
            // corresponds to the CMS reference swap tenor.
            //
            // Two distinct volatilities are needed:
            //  - `strike_vol` σ(K) prices the Black-76 option payoff (captures
            //    the smile at the option strike).
            //  - `atm_vol` σ(F) drives the convexity adjustment. The CMS
            //    convexity adjustment is a property of the swap-rate
            //    *distribution* under the annuity measure (it is `g'(F)/g(F)`
            //    times the swap-rate variance `Var^A[S] ≈ F²σ(F)²T`), so it
            //    must be evaluated with the at-the-money vol, NOT the strike
            //    vol. Using σ(K) makes the same forward inconsistently
            //    convexity-adjusted across strikes — and disagrees with the
            //    static-replication pricer, which already uses σ(F). See
            //    Hagan (2003) and `replication_pricer.rs`.
            let strike_vol = finstack_quant_models::volatility::get_surface_vol_clamped(
                &vol_surface,
                time_to_fixing.max(0.0),
                strike,
            );
            let atm_vol = finstack_quant_models::volatility::get_surface_vol_clamped(
                &vol_surface,
                time_to_fixing.max(0.0),
                forward_swap_rate,
            );

            // Convexity adjustment using Hagan (2003) formula with the ATM vol.
            let raw_convexity_adj = if time_to_fixing > 0.0 {
                convexity_adjustment_with_frequency(
                    atm_vol,
                    time_to_fixing,
                    inst.cms_tenor,
                    forward_swap_rate,
                    reference_swap.payments_per_year()?,
                )
            } else {
                0.0
            };

            let convexity_adj = raw_convexity_adj * convexity_scale;
            let adjusted_rate = forward_swap_rate + convexity_adj;

            // 3. Black Price — the option payoff uses the strike vol σ(K) so
            //    the smile is captured at the option strike.
            let option_val = if time_to_fixing <= 0.0 {
                match inst.option_type {
                    crate::instruments::OptionType::Call => (forward_swap_rate - strike).max(0.0),
                    crate::instruments::OptionType::Put => (strike - forward_swap_rate).max(0.0),
                }
            } else {
                match inst.option_type {
                    crate::instruments::OptionType::Call => {
                        black_call(adjusted_rate, strike, strike_vol, time_to_fixing)
                    }
                    crate::instruments::OptionType::Put => {
                        black_put(adjusted_rate, strike, strike_vol, time_to_fixing)
                    }
                }
            };

            // 4. Discount to present using curve-consistent relative DF
            let df_pay = relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;

            let period_pv = option_val * accrual_fraction * df_pay;
            total_pv += period_pv;
        }

        Money::new(total_pv * inst.notional.amount(), inst.notional.currency())
    }

    fn price_internal(
        &self,
        inst: &CmsOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        self.price_internal_with_convexity(inst, curves, as_of, 1.0)
    }
}

impl crate::instruments::rates::cms_common::CmsConvexityPricing for CmsOption {
    fn pv_with_convexity_scale(
        &self,
        market: &MarketContext,
        as_of: Date,
        convexity_scale: f64,
    ) -> Result<Money> {
        CmsOptionPricer::new().price_internal_with_convexity(self, market, as_of, convexity_scale)
    }
}

impl Default for CmsOptionPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for CmsOptionPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CmsOption, ModelKey::Black76)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let cms = crate::pricer::expect_inst::<CmsOption>(instrument, InstrumentType::CmsOption)?;

        let pv = self
            .price_internal(cms, market, as_of)
            .map_err(|e| PricingError::from_core(e, PricingErrorContext::default()))?;

        Ok(ValuationResult::stamped(cms.id(), as_of, pv))
    }
}

/// Present value using Convexity Adjusted Black.
pub(crate) fn compute_pv(inst: &CmsOption, curves: &MarketContext, as_of: Date) -> Result<Money> {
    let pricer = CmsOptionPricer::new();
    pricer.price_internal(inst, curves, as_of)
}

/// Compute convexity adjustment for CMS rate using Hagan (2003) methodology.
///
/// The convexity adjustment accounts for the measure change from the annuity
/// measure (where the forward swap rate is a martingale) to the payment measure
/// (where the CMS rate is a martingale).
///
/// # Formula
///
/// The CMS-adjusted forward is `E^{T_pay}[S] = F + CA`. To first order in the
/// swap-rate variance, the Hagan (2003) standard-model adjustment is:
///
/// ```text
/// CA ≈ (g'(F) / g(F)) · Var^A[S] ≈ (g'(F) / g(F)) · F² · σ² · T
/// ```
///
/// where `g(k) = DF_pay / A_par(k)` is the Radon-Nikodym derivative between the
/// payment measure and the annuity measure. Because `DF_pay` is independent of
/// `k`, `g'(F)/g(F) = −A_par'(F)/A_par(F)`. The bracket `g'/g` has units of
/// `1/rate`, so `CA = (g'/g)·F²·σ²T` has units of a rate — dimensionally
/// consistent.
///
/// The earlier `0.5·σ²T·G(S)` form with `G(S) = swap_tenor/(1+S·tenor)²` was
/// dimensionally wrong: `G(S)` carries units of *years*, so the result was not
/// a rate and was oversized by one-to-two orders of magnitude.
///
/// The fixed-leg payment frequency is assumed semi-annual (`m = 2`), the
/// dominant market convention. Callers needing the exact schedule should use
/// the static-replication pricer (`replication_pricer`,
/// `ModelKey::StaticReplication`), which captures convexity to all orders.
///
/// # Arguments
///
/// * `volatility` - Swap rate volatility (annualized, decimal form e.g. 0.20 for 20%)
/// * `time_to_fixing` - Time to fixing date in years
/// * `swap_tenor` - Tenor of the underlying CMS swap in years (e.g., 10.0 for 10Y)
/// * `forward_rate` - Current forward swap rate (decimal form e.g. 0.03 for 3%)
///
/// # Returns
///
/// Convexity adjustment to add to the forward swap rate (decimal form).
///
/// # References
///
/// - Hagan, P. S. (2003). "Convexity Conundrums: Pricing CMS Swaps, Caps, and Floors."
///   Wilmott Magazine, March, 38-44. `docs/REFERENCES.md#hagan-2003-cms-convexity`
/// - Andersen, L. B., & Piterbarg, V. V. (2010). *Interest Rate Modeling*, Vol. 3, §16.2. `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
pub fn convexity_adjustment(
    volatility: f64,
    time_to_fixing: f64,
    swap_tenor: f64,
    forward_rate: f64,
) -> f64 {
    convexity_adjustment_with_frequency(volatility, time_to_fixing, swap_tenor, forward_rate, 2.0)
}

/// First-order CMS convexity adjustment using the actual reference-swap fixed
/// payment frequency.
///
/// # Arguments
///
/// * `volatility` - Annualized swap-rate volatility as a decimal.
/// * `time_to_fixing` - Remaining time to CMS fixing in years.
/// * `swap_tenor` - Underlying reference-swap tenor in years.
/// * `forward_rate` - Forward par swap rate before convexity adjustment.
/// * `payments_per_year` - Fixed-leg payment frequency, such as `2.0` for
///   semiannual coupons, used by the annuity proxy.
pub fn convexity_adjustment_with_frequency(
    volatility: f64,
    time_to_fixing: f64,
    swap_tenor: f64,
    forward_rate: f64,
    payments_per_year: f64,
) -> f64 {
    if forward_rate <= 0.0
        || time_to_fixing <= 0.0
        || swap_tenor <= 0.0
        || !payments_per_year.is_finite()
        || payments_per_year <= 0.0
    {
        return 0.0;
    }

    let a_par = |k: f64| par_annuity(k, swap_tenor, payments_per_year);
    let a0 = a_par(forward_rate);
    if a0.abs() < 1e-12 {
        return 0.0;
    }

    // g'(F)/g(F) = −A_par'(F)/A_par(F), with A_par' via a central difference.
    let h = (forward_rate * 1e-4).max(1e-7);
    let a_prime = (a_par(forward_rate + h) - a_par(forward_rate - h)) / (2.0 * h);
    let g_log_deriv = -a_prime / a0;

    g_log_deriv * forward_rate * forward_rate * volatility * volatility * time_to_fixing
}
