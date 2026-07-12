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
//!   Wilmott Magazine, March, 38-44.
//! - Hull, J. (2018). "Options, Futures, and Other Derivatives."

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cms_option::types::CmsOption;
use crate::models::d1_d2_black76;
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

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
                let observed =
                    crate::instruments::rates::exotics_shared::fixings::historical_cms_fixing(
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
            let swap_start = inst.reference_swap_start(fixing_date)?;
            let swap_tenor_months = (inst.cms_tenor * 12.0).round() as i32;
            let swap_end = swap_start.add_months(swap_tenor_months);

            // Calculate annuity and forward rate
            let (forward_swap_rate, _) =
                self.calculate_forward_swap_rate(inst, curves, as_of, swap_start, swap_end)?;

            // Negative-rate regimes (EUR/JPY/CHF): Black-76 and the Hagan
            // lognormal convexity adjustment are undefined for F ≤ 0. Fall
            // back to the Bachelier (normal) model — matching the swaption
            // and cap/floor pricers — with the surface's lognormal vol
            // converted to a normal vol. The lognormal convexity adjustment
            // scales with F²σ²T and vanishes as F → 0, so it is omitted on
            // this path.
            if forward_swap_rate <= 0.0 {
                let time_to_fixing = DayCount::Act365F.year_fraction(
                    as_of,
                    fixing_date,
                    DayCountContext::default(),
                )?;
                let option_val = if time_to_fixing <= 0.0 {
                    match inst.option_type {
                        crate::instruments::OptionType::Call => {
                            (forward_swap_rate - strike).max(0.0)
                        }
                        crate::instruments::OptionType::Put => {
                            (strike - forward_swap_rate).max(0.0)
                        }
                    }
                } else {
                    let strike_vol = vol_surface.value_clamped(time_to_fixing, strike);
                    let normal_vol =
                        crate::instruments::rates::swaption::types::lognormal_to_normal_vol(
                            strike_vol,
                            forward_swap_rate,
                            strike,
                            time_to_fixing,
                            None,
                        );
                    crate::models::volatility::normal::bachelier_price(
                        inst.option_type,
                        forward_swap_rate,
                        strike,
                        normal_vol,
                        time_to_fixing,
                        1.0,
                    )
                };
                let df_pay =
                    relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;
                total_pv += option_val * accrual_fraction * df_pay;
                continue;
            }

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
            let strike_vol = vol_surface.value_clamped(time_to_fixing.max(0.0), strike);
            let atm_vol = vol_surface.value_clamped(time_to_fixing.max(0.0), forward_swap_rate);

            // Convexity adjustment using Hagan (2003) formula with the ATM vol.
            let raw_convexity_adj = if time_to_fixing > 0.0 {
                convexity_adjustment_with_frequency(
                    atm_vol,
                    time_to_fixing,
                    inst.cms_tenor,
                    forward_swap_rate,
                    1.0 / inst.resolved_swap_fixed_freq().to_years_simple(),
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
                self.black_price(
                    adjusted_rate,
                    strike,
                    strike_vol,
                    time_to_fixing,
                    inst.option_type,
                )
            };

            // 4. Discount to present using curve-consistent relative DF
            let df_pay = relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;

            let period_pv = option_val * accrual_fraction * df_pay;
            total_pv += period_pv;
        }

        Ok(Money::new(
            total_pv * inst.notional.amount(),
            inst.notional.currency(),
        ))
    }

    fn price_internal(
        &self,
        inst: &CmsOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        self.price_internal_with_convexity(inst, curves, as_of, 1.0)
    }

    /// Calculate forward swap rate and annuity.
    ///
    /// # Time Basis
    ///
    /// Uses curve-consistent time mapping:
    /// - Discount factors use `relative_df_discount_curve` (curve's own day_count/base_date)
    /// - Forward rates use `rate_between_on_dates` (forward curve's own day_count/base_date)
    /// - Accrual fractions use `swap_day_count` for the fixed leg and
    ///   `swap_float_day_count` (if provided) for the floating leg.
    pub(crate) fn calculate_forward_swap_rate(
        &self,
        inst: &CmsOption,
        market: &MarketContext,
        as_of: Date,
        start: Date,
        end: Date,
    ) -> Result<(f64, f64)> {
        let convention = crate::instruments::rates::exotics_shared::forward_swap_rate::resolve_reference_swap_convention(
            inst.swap_convention,
            inst.notional.currency(),
        )?;
        let calendar_id = convention.calendar_id().ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "CMS reference-swap convention has no calendar".to_string(),
            )
        })?;
        crate::instruments::rates::exotics_shared::forward_swap_rate::calculate_forward_swap_rate(
            crate::instruments::rates::exotics_shared::forward_swap_rate::ForwardSwapRateInputs {
                market,
                discount_curve_id: &inst.discount_curve_id,
                forward_curve_id: &inst.forward_curve_id,
                as_of,
                start,
                end,
                fixed_freq: inst.resolved_swap_fixed_freq(),
                fixed_day_count: inst.resolved_swap_day_count(),
                float_freq: inst.resolved_swap_float_freq(),
                float_day_count: inst.resolved_swap_float_day_count(),
                calendar_id: &calendar_id,
                business_day_convention: convention.business_day_convention(),
                stub: finstack_quant_core::dates::StubKind::ShortFront,
                end_of_month: start.end_of_month() == start && end.end_of_month() == end,
                payment_lag_days: convention.payment_lag_days(),
                enforce_forward_tenor: !convention.uses_daily_compounding(),
            },
        )
    }

    fn black_price(
        &self,
        forward: f64,
        strike: f64,
        vol: f64,
        t: f64,
        option_type: crate::instruments::OptionType,
    ) -> f64 {
        if t <= 0.0 {
            return match option_type {
                crate::instruments::OptionType::Call => (forward - strike).max(0.0),
                crate::instruments::OptionType::Put => (strike - forward).max(0.0),
            };
        }

        // Use combined d1_d2_black76 for efficiency (computes shared intermediates once)
        let (d1, d2) = d1_d2_black76(forward, strike, vol, t);

        match option_type {
            crate::instruments::OptionType::Call => {
                forward * finstack_quant_core::math::norm_cdf(d1)
                    - strike * finstack_quant_core::math::norm_cdf(d2)
            }
            crate::instruments::OptionType::Put => {
                strike * finstack_quant_core::math::norm_cdf(-d2)
                    - forward * finstack_quant_core::math::norm_cdf(-d1)
            }
        }
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
        let cms = instrument
            .as_any()
            .downcast_ref::<CmsOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CmsOption, instrument.key())
            })?;

        let pv = self.price_internal(cms, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

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
///   Wilmott Magazine, March, 38-44.
/// - Andersen, L. B., & Piterbarg, V. V. (2010). *Interest Rate Modeling*, Vol. 3, §16.2.
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

    let a_par = |k: f64| par_annuity_proxy(k, swap_tenor, payments_per_year);
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

/// Closed-form par annuity for a fixed-rate swap.
///
/// `A_par(k) = (1 − (1 + k/m)^(−n·m)) / k` for `k > 0`, with the L'Hôpital
/// limit `A_par(0) = n`. This is the same closed form used by the CMS
/// static-replication pricer.
fn par_annuity_proxy(rate: f64, tenor_years: f64, m: f64) -> f64 {
    if rate.abs() < 1e-12 {
        return tenor_years;
    }
    let nm = tenor_years * m;
    let discount = (1.0 + rate / m).powf(-nm);
    (1.0 - discount) / rate
}
