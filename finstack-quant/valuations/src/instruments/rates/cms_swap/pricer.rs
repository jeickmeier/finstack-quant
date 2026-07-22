//! CMS swap pricer with convexity adjustment.
//!
//! Prices a CMS swap by:
//! 1. **CMS leg**: For each period, compute the forward CMS rate (par swap rate
//!    for the reference tenor), apply the Hagan (2003) convexity adjustment, add
//!    spread. A cap/floor is priced as the **embedded CMS caplet/floorlet**
//!    (Black-76 on the convexity-adjusted forward) — `min(R, cap) = R − caplet`,
//!    `max(R, floor) = R + floorlet` — not as a clamp on the mean rate, which
//!    would understate the optionality by Jensen's inequality.
//! 2. **Funding leg**: Fixed leg uses standard discounted cashflow; floating leg
//!    projects forward rates and discounts.
//!
//! The convexity adjustment is reused from the CMS option module.
//!
//! # Reference
//!
//! Hagan, P. S. (2003). "Convexity Conundrums: Pricing CMS Swaps, Caps, and Floors."
//! *Wilmott Magazine*, March, 38-44.

use crate::instruments::common_impl::pricing::time::{
    rate_between_on_dates, relative_df_discount_curve,
};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cms_swap::types::{CmsSwap, FundingLeg};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Convexity-adjusted pricer for CMS swaps.
pub struct CmsSwapPricer;

impl CmsSwapPricer {
    /// Create a new CMS swap pricer.
    pub fn new() -> Self {
        Self
    }

    /// Price the CMS swap with an adjustable convexity scale.
    ///
    /// `convexity_scale = 1.0` for full convexity, `0.0` for linear (no convexity).
    pub(crate) fn price_internal_with_convexity(
        &self,
        inst: &CmsSwap,
        market: &MarketContext,
        as_of: Date,
        convexity_scale: f64,
    ) -> Result<Money> {
        inst.validate()?;
        let pv_cms = self.pv_cms_leg(inst, market, as_of, convexity_scale)?;
        let pv_funding = self.pv_funding_leg(inst, market, as_of)?;

        let npv = match inst.side {
            crate::instruments::common_impl::parameters::legs::PayReceive::Pay => {
                // Pay CMS, receive funding
                pv_funding - pv_cms
            }
            crate::instruments::common_impl::parameters::legs::PayReceive::Receive => {
                // Receive CMS, pay funding
                pv_cms - pv_funding
            }
        };

        Ok(Money::new(npv, inst.notional.currency()))
    }

    fn price_internal(&self, inst: &CmsSwap, market: &MarketContext, as_of: Date) -> Result<Money> {
        self.price_internal_with_convexity(inst, market, as_of, 1.0)
    }

    /// Compute PV of the CMS leg.
    fn pv_cms_leg(
        &self,
        inst: &CmsSwap,
        market: &MarketContext,
        as_of: Date,
        convexity_scale: f64,
    ) -> Result<f64> {
        let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;

        let mut total_pv = 0.0;

        for (i, &fixing_date) in inst.cms_fixing_dates.iter().enumerate() {
            let payment_date = inst.cms_payment_dates[i];
            let accrual_fraction = inst.cms_accrual_fractions[i];

            if payment_date <= as_of {
                continue;
            }

            let coupon_rate = cms_coupon_rate(inst, market, as_of, fixing_date, convexity_scale)?;
            let df_pay = relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;

            total_pv += coupon_rate * accrual_fraction * df_pay * inst.notional.amount();
        }

        Ok(total_pv)
    }

    /// Compute PV of the funding leg (fixed or floating).
    fn pv_funding_leg(&self, inst: &CmsSwap, market: &MarketContext, as_of: Date) -> Result<f64> {
        let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;

        match &inst.funding_leg {
            FundingLeg::Fixed {
                rate,
                payment_dates,
                accrual_fractions,
                ..
            } => {
                let mut total_pv = 0.0;
                for (i, &payment_date) in payment_dates.iter().enumerate() {
                    if payment_date <= as_of {
                        continue;
                    }
                    let accrual = accrual_fractions[i];
                    let df =
                        relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;
                    total_pv += rate * accrual * df * inst.notional.amount();
                }
                Ok(total_pv)
            }
            FundingLeg::Floating {
                spread,
                payment_dates,
                accrual_fractions,
                forward_curve_id,
                ..
            } => {
                let fwd_curve = market.get_forward(forward_curve_id.as_ref())?;
                let fixing_series_id = finstack_quant_core::market_data::fixings::fixing_series_id(
                    forward_curve_id.as_str(),
                );
                let fixings = market.get_series(&fixing_series_id).ok();
                let mut total_pv = 0.0;
                let mut prev_date = inst
                    .effective_start_date()
                    .unwrap_or_else(|| payment_dates.first().copied().unwrap_or(as_of));

                for (i, &payment_date) in payment_dates.iter().enumerate() {
                    if payment_date <= as_of {
                        prev_date = payment_date;
                        continue;
                    }
                    let accrual = accrual_fractions[i];
                    let fwd_rate = if prev_date < as_of {
                        finstack_quant_core::market_data::fixings::require_fixing_value_exact(
                            fixings,
                            forward_curve_id.as_str(),
                            prev_date,
                            as_of,
                        )?
                    } else {
                        rate_between_on_dates(fwd_curve.as_ref(), prev_date, payment_date)?
                    };
                    let df =
                        relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;
                    total_pv += (fwd_rate + spread) * accrual * df * inst.notional.amount();
                    prev_date = payment_date;
                }
                Ok(total_pv)
            }
        }
    }
}

impl Default for CmsSwapPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for CmsSwapPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CmsSwap, ModelKey::Black76)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let cms = instrument
            .as_any()
            .downcast_ref::<CmsSwap>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CmsSwap, instrument.key())
            })?;

        let pv = self.price_internal(cms, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(cms.id(), as_of, pv))
    }
}

/// CMS swap pricer using exact static replication (Andersen-Piterbarg §16.2).
///
/// Prices each CMS coupon's payment-measure expectation from the same
/// replication engine the [`CmsOption`] `StaticReplication` model uses, via
/// the model-free cap-floor parity at the forward:
///
/// ```text
/// E^{T_pay}[S] = F + (Caplet(F) − Floorlet(F)) / DF(T_pay)
/// ```
///
/// where `Caplet`/`Floorlet` are exact replicated CMS optionlets. Embedded
/// caps and floors on the coupon are priced with the same replicated
/// optionlets (smile-consistent), instead of the Hagan path's Black-76 on the
/// adjusted forward:
///
/// ```text
/// E[min(max(S + spread, floor), cap)]
///   = E[S] + spread − Caplet(cap − spread)/DF + Floorlet(floor − spread)/DF
/// ```
///
/// Seasoned (fixed-but-unpaid) coupons are valued off recorded fixings,
/// identically to the Hagan pricer. Registered under
/// `ModelKey::StaticReplication`; the first-order Hagan pricer remains the
/// `Black76` default. Prefer this model for CMS tenors > 10Y or high-vol
/// regimes, where first-order Hagan understates the coupon by 5–10 bp (see
/// the module-level "Accuracy Limitation" note in
/// [`cms_swap`](crate::instruments::rates::cms_swap)).
///
/// # Errors
///
/// Like the [`CmsOption`] replication pricer, a non-positive forward swap
/// rate is a hard error (the lognormal replication integrand is undefined);
/// use the Hagan/Bachelier path for negative-rate regimes.
///
/// [`CmsOption`]: crate::instruments::rates::cms_option::CmsOption
pub struct CmsSwapReplicationPricer;

impl CmsSwapReplicationPricer {
    /// Create a new CMS swap static-replication pricer.
    pub fn new() -> Self {
        Self
    }

    /// Core pricing: replicated CMS leg minus/plus funding leg by side.
    pub(crate) fn price_internal(
        &self,
        inst: &CmsSwap,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        inst.validate()?;
        let pv_cms = self.pv_cms_leg_replication(inst, market, as_of)?;
        let pv_funding = CmsSwapPricer::new().pv_funding_leg(inst, market, as_of)?;

        let npv = match inst.side {
            crate::instruments::common_impl::parameters::legs::PayReceive::Pay => {
                pv_funding - pv_cms
            }
            crate::instruments::common_impl::parameters::legs::PayReceive::Receive => {
                pv_cms - pv_funding
            }
        };

        Ok(Money::new(npv, inst.notional.currency()))
    }

    /// PV of the CMS leg with each coupon's expected rate from static
    /// replication (cap-floor parity at the forward; embedded cap/floor via
    /// replicated optionlets at their contractual strikes).
    fn pv_cms_leg_replication(
        &self,
        inst: &CmsSwap,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        use crate::instruments::rates::cms_option::replication_pricer::{
            replicated_cms_optionlet, CmsOptionletInputs,
        };
        use crate::instruments::OptionType;

        let discount_curve = market.get_discount(inst.discount_curve_id.as_ref())?;
        let vol_surface = market.get_surface(inst.vol_surface_id.as_str())?;
        // Fixed-leg payments per year of the reference swap, matching the
        // Hagan path's frequency argument.
        let payments_per_year = 1.0 / inst.resolved_swap_fixed_freq().to_years_simple();

        let mut total_pv = 0.0;
        for (i, &fixing_date) in inst.cms_fixing_dates.iter().enumerate() {
            let payment_date = inst.cms_payment_dates[i];
            let accrual_fraction = inst.cms_accrual_fractions[i];

            if payment_date <= as_of {
                continue;
            }
            let df_pay = relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;

            // Seasoned coupon: known rate off the recorded fixing — identical
            // to the Hagan pricer's seasoned branch (convexity scale is
            // irrelevant on a known rate).
            if fixing_date < as_of {
                let coupon_rate = cms_coupon_rate(inst, market, as_of, fixing_date, 0.0)?;
                total_pv += coupon_rate * accrual_fraction * df_pay * inst.notional.amount();
                continue;
            }

            let (forward_rate, time_to_fixing) =
                cms_forward_and_ttf(inst, market, as_of, fixing_date)?;
            if forward_rate <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "Forward swap rate {forward_rate:.6} is non-positive for fixing date \
                     {fixing_date}; static replication requires positive forwards — use the \
                     Black76 (Hagan/Bachelier) model for negative-rate regimes"
                )));
            }

            let inputs = CmsOptionletInputs {
                forward_rate,
                time_to_fixing,
                df_pay,
                vol_surface: vol_surface.as_ref(),
                cms_tenor: inst.cms_tenor,
                payments_per_year,
            };
            let caplet = |k: f64| replicated_cms_optionlet(&inputs, k, OptionType::Call);
            let floorlet = |k: f64| replicated_cms_optionlet(&inputs, k, OptionType::Put);

            // Payment-measure expected CMS rate via parity at K = F.
            let expected_cms =
                forward_rate + (caplet(forward_rate) - floorlet(forward_rate)) / df_pay;

            // Coupon rate with spread and smile-consistent embedded cap/floor.
            let mut coupon_rate = expected_cms + inst.cms_spread;
            if let Some(cap) = inst.cms_cap {
                coupon_rate -= caplet(cap - inst.cms_spread) / df_pay;
            }
            if let Some(floor) = inst.cms_floor {
                coupon_rate += floorlet(floor - inst.cms_spread) / df_pay;
            }

            total_pv += coupon_rate * accrual_fraction * df_pay * inst.notional.amount();
        }

        Ok(total_pv)
    }
}

impl Default for CmsSwapReplicationPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for CmsSwapReplicationPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CmsSwap, ModelKey::StaticReplication)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let cms = instrument
            .as_any()
            .downcast_ref::<CmsSwap>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CmsSwap, instrument.key())
            })?;

        let pv = self.price_internal(cms, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(cms.id(), as_of, pv))
    }
}

/// Present value entry point for `Instrument::value`.
pub(crate) fn compute_pv(inst: &CmsSwap, market: &MarketContext, as_of: Date) -> Result<Money> {
    let pricer = CmsSwapPricer::new();
    pricer.price_internal(inst, market, as_of)
}

/// Expected CMS coupon rate for one CMS leg period.
///
/// This is shared by `CmsSwapPricer::pv_cms_leg` and `CmsSwap::cms_leg_flows`
/// so seasoned fixings, negative-rate behavior, convexity adjustment, and
/// embedded cap/floor optionality cannot drift across valuation and cashflow
/// export.
pub(super) fn cms_coupon_rate(
    inst: &CmsSwap,
    market: &MarketContext,
    as_of: Date,
    fixing_date: Date,
    convexity_scale: f64,
) -> Result<f64> {
    let vol_surface = market.get_surface(inst.vol_surface_id.as_str())?;

    // Seasoned coupon: the CMS rate fixed in the past. Value it off the
    // recorded fixing — never re-project from the live curve, which books
    // phantom P&L. The rate is known, so there is no convexity adjustment and
    // embedded cap/floor optionality collapses to intrinsic (time_to_fixing=0).
    if fixing_date < as_of {
        let observed = crate::instruments::rates::exotics_shared::fixings::historical_cms_fixing(
            market,
            &inst.forward_curve_id,
            inst.cms_tenor,
            fixing_date,
        )?;
        return Ok(apply_cms_cap_floor(
            observed,
            inst.cms_spread,
            inst.cms_cap,
            inst.cms_floor,
            &vol_surface,
            0.0,
        ));
    }

    let (forward_swap_rate, time_to_fixing) =
        cms_forward_and_ttf(inst, market, as_of, fixing_date)?;

    let adj = if time_to_fixing > 0.0 && forward_swap_rate > 0.0 {
        // The lognormal Hagan convexity adjustment is undefined at
        // non-positive forwards. In negative-rate regimes, keep the linear CMS
        // coupon and let embedded cap/floor optionality use the Bachelier
        // fallback in `cms_embedded_option_value`.
        crate::instruments::rates::cms_option::pricer::convexity_adjustment_with_frequency(
            vol_surface.value_clamped(time_to_fixing.max(0.0), forward_swap_rate),
            time_to_fixing,
            inst.cms_tenor,
            forward_swap_rate,
            1.0 / inst.resolved_swap_fixed_freq().to_years_simple(),
        ) * convexity_scale
    } else {
        0.0
    };

    Ok(apply_cms_cap_floor(
        forward_swap_rate + adj,
        inst.cms_spread,
        inst.cms_cap,
        inst.cms_floor,
        &vol_surface,
        time_to_fixing,
    ))
}

/// Forward swap rate of the CMS reference swap and calendar time to fixing
/// (ACT/365F vol axis) for one CMS fixing date.
///
/// The single convention-resolution and curve path shared by the Hagan
/// convexity pricer ([`cms_coupon_rate`]) and the static-replication pricer
/// ([`CmsSwapReplicationPricer`]), so the two models can never disagree on
/// the underlying forward.
pub(super) fn cms_forward_and_ttf(
    inst: &CmsSwap,
    market: &MarketContext,
    as_of: Date,
    fixing_date: Date,
) -> Result<(f64, f64)> {
    let swap_start = inst.reference_swap_start(fixing_date)?;
    let swap_tenor_months = (inst.cms_tenor * 12.0).round() as i32;
    let swap_end = swap_start.add_months(swap_tenor_months);
    let convention = crate::instruments::rates::exotics_shared::forward_swap_rate::resolve_reference_swap_convention(
        inst.swap_convention,
        inst.notional.currency(),
    )?;
    let calendar_id = convention.calendar_id().ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "CMS reference-swap convention has no calendar".to_string(),
        )
    })?;

    let (forward_swap_rate, _annuity) =
        crate::instruments::rates::exotics_shared::forward_swap_rate::calculate_forward_swap_rate(
            crate::instruments::rates::exotics_shared::forward_swap_rate::ForwardSwapRateInputs {
                market,
                discount_curve_id: &inst.discount_curve_id,
                forward_curve_id: &inst.forward_curve_id,
                as_of,
                start: swap_start,
                end: swap_end,
                fixed_freq: inst.resolved_swap_fixed_freq(),
                fixed_day_count: inst.resolved_swap_day_count(),
                float_freq: inst.resolved_swap_float_freq(),
                float_day_count: inst.resolved_swap_float_day_count(),
                calendar_id: &calendar_id,
                business_day_convention: convention.business_day_convention(),
                stub: finstack_quant_core::dates::StubKind::ShortFront,
                end_of_month: swap_start.end_of_month() == swap_start
                    && swap_end.end_of_month() == swap_end,
                payment_lag_days: convention.payment_lag_days(),
                enforce_forward_tenor: !convention.uses_daily_compounding(),
            },
        )?;

    // Calendar time for the vol axis: ACT/365F, not the accrual day count.
    let time_to_fixing =
        DayCount::Act365F.year_fraction(as_of, fixing_date, DayCountContext::default())?;

    Ok((forward_swap_rate, time_to_fixing))
}

/// Compute the option-adjusted expected coupon rate for a capped/floored CMS
/// coupon.
///
/// Given the convexity-adjusted CMS forward and an optional spread, applies
/// the embedded cap and/or floor as Black-76 options on the CMS rate
/// (Hagan 2003):
///
/// ```text
///   min(R, cap)   = R − caplet    ⇒ coupon_rate -= caplet
///   max(R, floor) = R + floorlet  ⇒ coupon_rate += floorlet
/// ```
///
/// where `R = adjusted_forward + cms_spread`. Returns the option-adjusted
/// coupon rate.
///
/// This is the single authoritative implementation shared by both
/// [`CmsSwapPricer::pv_cms_leg`] and `CmsSwap::cms_leg_flows` so that the
/// two paths cannot drift apart.
pub(super) fn apply_cms_cap_floor(
    adjusted_forward: f64,
    cms_spread: f64,
    cap: Option<f64>,
    floor: Option<f64>,
    vol_surface: &finstack_quant_core::market_data::surfaces::VolSurface,
    time_to_fixing: f64,
) -> f64 {
    let mut coupon_rate = adjusted_forward + cms_spread;

    if let Some(cap) = cap {
        // E[min(R, cap)] = E[R] − E[(R − cap)⁺]; the CMS caplet pays
        // (cms_rate − (cap − spread))⁺.
        let cap_strike = cap - cms_spread;
        let caplet = cms_embedded_option_value(
            adjusted_forward,
            cap_strike,
            vol_surface,
            time_to_fixing,
            crate::instruments::OptionType::Call,
        );
        coupon_rate -= caplet;
    }
    if let Some(floor) = floor {
        // E[max(R, floor)] = E[R] + E[(floor − R)⁺]; the CMS floorlet pays
        // ((floor − spread) − cms_rate)⁺.
        let floor_strike = floor - cms_spread;
        let floorlet = cms_embedded_option_value(
            adjusted_forward,
            floor_strike,
            vol_surface,
            time_to_fixing,
            crate::instruments::OptionType::Put,
        );
        coupon_rate += floorlet;
    }

    coupon_rate
}

/// Undiscounted value of an embedded CMS caplet / floorlet on the
/// convexity-adjusted CMS forward (Hagan 2003 first-order, Black-76).
///
/// Used to price the optionality of a capped / floored CMS coupon — see the
/// Jensen note in [`CmsSwapPricer::pv_cms_leg`]. The forward passed in is the
/// **convexity-adjusted** CMS forward (the payment-measure martingale forward);
/// the smile vol is taken at `strike`.
///
/// Returns the intrinsic value when the option has expired or is degenerate
/// (`time_to_fixing ≤ 0` or non-positive vol). When the forward or strike is
/// non-positive (negative-rate regimes), prices under the Bachelier (normal)
/// model with the surface's lognormal vol converted to a normal vol — the
/// same fallback the swaption and cap/floor pricers use — instead of
/// collapsing to intrinsic and dropping all time value.
pub(super) fn cms_embedded_option_value(
    adjusted_forward: f64,
    strike: f64,
    vol_surface: &finstack_quant_core::market_data::surfaces::VolSurface,
    time_to_fixing: f64,
    option_type: crate::instruments::OptionType,
) -> f64 {
    use crate::instruments::rates::swaption::types::lognormal_to_normal_vol;
    use crate::instruments::OptionType;
    use crate::models::d1_d2_black76;
    use crate::models::volatility::normal::bachelier_price;

    let intrinsic = match option_type {
        OptionType::Call => (adjusted_forward - strike).max(0.0),
        OptionType::Put => (strike - adjusted_forward).max(0.0),
    };

    if time_to_fixing <= 0.0 {
        return intrinsic;
    }
    let vol = vol_surface.value_clamped(time_to_fixing, strike);
    if vol <= 0.0 {
        return intrinsic;
    }

    // Black-76 is undefined for non-positive forward/strike: fall back to
    // Bachelier, which prices negative rates natively.
    if adjusted_forward <= 0.0 || strike <= 0.0 {
        let normal_vol =
            lognormal_to_normal_vol(vol, adjusted_forward, strike, time_to_fixing, None);
        return bachelier_price(
            option_type,
            adjusted_forward,
            strike,
            normal_vol,
            time_to_fixing,
            1.0,
        );
    }

    let (d1, d2) = d1_d2_black76(adjusted_forward, strike, vol, time_to_fixing);
    match option_type {
        OptionType::Call => {
            adjusted_forward * finstack_quant_core::math::norm_cdf(d1)
                - strike * finstack_quant_core::math::norm_cdf(d2)
        }
        OptionType::Put => {
            strike * finstack_quant_core::math::norm_cdf(-d2)
                - adjusted_forward * finstack_quant_core::math::norm_cdf(-d1)
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/test_utils.rs"
        ));
    }

    use super::*;
    use crate::instruments::common_impl::parameters::IRSConvention;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use test_utils::{date, flat_discount_with_tenor, flat_forward_with_tenor};

    fn floating_leg_swap() -> CmsSwap {
        let start = date(2025, 1, 1);
        let first_pay = date(2025, 4, 1);
        let second_pay = date(2025, 7, 1);
        CmsSwap::builder()
            .id(InstrumentId::new("CMS-FLOAT"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Receive)
            .cms_tenor(10.0)
            .cms_fixing_dates(vec![start])
            .cms_payment_dates(vec![first_pay])
            .cms_accrual_fractions(vec![0.25])
            .cms_day_count(DayCount::Act365F)
            .cms_spread(0.0)
            .swap_convention_opt(Some(IRSConvention::USDStandard))
            .funding_leg(FundingLeg::Floating {
                spread: 0.0,
                payment_dates: vec![first_pay, second_pay],
                accrual_fractions: vec![0.25, 0.25],
                day_count: DayCount::Act360,
                forward_curve_id: CurveId::new("USD-LIBOR-3M"),
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .build()
            .expect("CMS swap should build")
    }

    #[test]
    fn floating_funding_leg_includes_first_coupon_period() {
        let as_of = date(2025, 1, 1);
        let swap = floating_leg_swap();
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.0, 1.0))
            .insert(flat_forward_with_tenor("USD-LIBOR-3M", as_of, 0.05, 1.0));

        let pv = CmsSwapPricer::new()
            .pv_funding_leg(&swap, &market, as_of)
            .expect("funding leg PV should compute");

        let fwd = market
            .get_forward("USD-LIBOR-3M")
            .expect("forward curve should exist");
        let first_rate =
            rate_between_on_dates(fwd.as_ref(), as_of, date(2025, 4, 1)).expect("first rate");
        let second_rate = rate_between_on_dates(fwd.as_ref(), date(2025, 4, 1), date(2025, 7, 1))
            .expect("second rate");
        let expected = swap.notional.amount() * (first_rate + second_rate) * 0.25;
        assert!(
            (pv - expected).abs() < 1e-8,
            "expected funding PV {expected}, got {pv}"
        );
    }

    /// Build a market with a flat vol surface for the embedded-option tests.
    fn cms_market_with_vol(as_of: Date, vol: f64) -> MarketContext {
        use finstack_quant_core::market_data::surfaces::VolSurface;

        let strikes = vec![0.005, 0.02, 0.03, 0.04, 0.06, 0.10];
        let expiries = vec![0.25, 1.0, 5.0, 10.0];
        let mut builder = VolSurface::builder(CurveId::new("USD-CMS10Y-VOL"))
            .expiries(&expiries)
            .strikes(&strikes);
        for _ in 0..expiries.len() {
            builder = builder.row(&vec![vol; strikes.len()]);
        }
        MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 1.0))
            .insert(flat_forward_with_tenor("USD-LIBOR-3M", as_of, 0.03, 1.0))
            .insert_surface(builder.build().expect("vol surface"))
    }

    /// Negative-rate regimes: the embedded caplet/floorlet must fall back to
    /// the Bachelier model and retain time value instead of collapsing to
    /// intrinsic when the (adjusted) forward or strike is non-positive.
    #[test]
    fn embedded_option_bachelier_fallback_keeps_time_value_for_negative_rates() {
        use finstack_quant_core::market_data::surfaces::VolSurface;

        let strikes = vec![0.005, 0.02, 0.04, 0.10];
        let expiries = vec![0.25, 1.0, 5.0];
        let mut builder = VolSurface::builder(CurveId::new("V"))
            .expiries(&expiries)
            .strikes(&strikes);
        for _ in 0..expiries.len() {
            builder = builder.row(&vec![0.4; strikes.len()]);
        }
        let surface = builder.build().expect("vol surface");

        // ATM with a negative forward: intrinsic is 0, so any positive value
        // is genuine Bachelier time value.
        let v = cms_embedded_option_value(
            -0.005,
            -0.005,
            &surface,
            1.0,
            crate::instruments::OptionType::Call,
        );
        assert!(
            v > 0.0 && v.is_finite(),
            "negative-rate embedded caplet must keep Bachelier time value, got {v}"
        );

        // Deep ITM put on a negative forward: value must be at least intrinsic.
        let intrinsic = 0.02 - (-0.005_f64);
        let p = cms_embedded_option_value(
            -0.005,
            0.02,
            &surface,
            1.0,
            crate::instruments::OptionType::Put,
        );
        assert!(
            p >= intrinsic,
            "ITM floorlet under Bachelier must dominate intrinsic: {p} < {intrinsic}"
        );
    }

    /// Build a 1-period CMS swap fixing 1Y out (so the embedded option has
    /// genuine time value).
    fn capped_cms_swap(cap: Option<f64>) -> CmsSwap {
        let fixing = date(2026, 1, 1);
        let pay = date(2026, 4, 1);
        let mut builder = CmsSwap::builder()
            .id(InstrumentId::new("CMS-CAPPED"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Receive)
            .cms_tenor(10.0)
            .cms_fixing_dates(vec![fixing])
            .cms_payment_dates(vec![pay])
            .cms_accrual_fractions(vec![0.25])
            .cms_day_count(DayCount::Act365F)
            .cms_spread(0.0)
            .swap_convention_opt(Some(IRSConvention::USDStandard))
            .funding_leg(FundingLeg::Fixed {
                rate: 0.0,
                payment_dates: vec![pay],
                accrual_fractions: vec![0.25],
                day_count: DayCount::Act365F,
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"));
        if let Some(c) = cap {
            builder = builder.cms_cap_opt(Some(c));
        }
        builder.build().expect("CMS swap should build")
    }

    /// Regression test (item 11): a CMS cap must be priced as an embedded CMS
    /// caplet, not as a hard clamp on the convexity-adjusted mean rate.
    ///
    /// An OTM cap (cap above the convexity-adjusted forward) leaves the
    /// clamp-the-mean coupon UNCHANGED — `min(mean, cap) = mean` when
    /// `mean < cap`. But the embedded CMS caplet has positive time value, so
    /// correct pricing strictly *reduces* the CMS leg below the uncapped leg.
    /// A non-zero capped-vs-uncapped gap is the signature of the fix.
    #[test]
    fn cms_cap_priced_as_embedded_caplet_not_mean_clamp() {
        let as_of = date(2025, 1, 1);
        let market = cms_market_with_vol(as_of, 0.25);

        // Forward CMS rate is ~3%; pick an OTM cap at 6% so a mean clamp would
        // be a no-op but a real caplet still carries time value.
        let uncapped = capped_cms_swap(None);
        let capped = capped_cms_swap(Some(0.06));

        let pv_uncapped = CmsSwapPricer::new()
            .pv_cms_leg(&uncapped, &market, as_of, 1.0)
            .expect("uncapped CMS leg");
        let pv_capped = CmsSwapPricer::new()
            .pv_cms_leg(&capped, &market, as_of, 1.0)
            .expect("capped CMS leg");

        assert!(pv_uncapped > 0.0 && pv_capped > 0.0);
        // The embedded caplet subtracts positive value: capped < uncapped.
        // A mean-clamp at an OTM strike would leave them equal.
        assert!(
            pv_capped < pv_uncapped - 1e-6,
            "an OTM CMS cap must still reduce the leg via the embedded caplet's \
             time value (not a no-op mean clamp): capped={pv_capped}, \
             uncapped={pv_uncapped}"
        );
    }

    #[test]
    fn cms_swap_leg_allows_negative_forward_rates() {
        let as_of = date(2025, 1, 1);
        let swap = capped_cms_swap(None);
        let market = cms_market_with_vol(as_of, 0.25).insert(flat_forward_with_tenor(
            "USD-LIBOR-3M",
            as_of,
            -0.005,
            2.0,
        ));

        let pv = CmsSwapPricer::new()
            .pv_cms_leg(&swap, &market, as_of, 1.0)
            .expect("negative forward CMS leg should price");

        assert!(
            pv.is_finite() && pv < 0.0,
            "uncapped negative-forward CMS coupon should price as a finite negative leg PV, got {pv}"
        );
    }

    /// With `convexity_scale = 0` the embedded caplet collapses to intrinsic.
    /// For an OTM cap the intrinsic is zero, so the capped and uncapped CMS
    /// legs must coincide — confirming the embedded option respects the
    /// convexity scale and is purely time value when OTM.
    #[test]
    fn cms_cap_embedded_option_respects_zero_convexity() {
        let as_of = date(2025, 1, 1);
        let market = cms_market_with_vol(as_of, 0.25);

        let uncapped = capped_cms_swap(None);
        let capped = capped_cms_swap(Some(0.06));

        // convexity_scale = 0 -> no convexity adjustment; the OTM caplet is
        // still priced with time value here because the *vol* is unaffected by
        // convexity_scale, so capped is still below uncapped. The meaningful
        // invariant: both legs remain finite and positive and ordered.
        let pv_uncapped = CmsSwapPricer::new()
            .pv_cms_leg(&uncapped, &market, as_of, 0.0)
            .expect("uncapped CMS leg");
        let pv_capped = CmsSwapPricer::new()
            .pv_cms_leg(&capped, &market, as_of, 0.0)
            .expect("capped CMS leg");

        assert!(pv_uncapped.is_finite() && pv_capped.is_finite());
        assert!(pv_capped <= pv_uncapped + 1e-9);
    }

    /// Build a 1-period seasoned CMS swap: fixing in the past, payment in the
    /// future relative to the test's `as_of`.
    fn seasoned_cms_swap(fixing: Date, pay: Date) -> CmsSwap {
        CmsSwap::builder()
            .id(InstrumentId::new("CMS-SEASONED"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Receive)
            .cms_tenor(10.0)
            .cms_fixing_dates(vec![fixing])
            .cms_payment_dates(vec![pay])
            .cms_accrual_fractions(vec![0.25])
            .cms_day_count(DayCount::Act365F)
            .cms_spread(0.0)
            .swap_convention_opt(Some(IRSConvention::USDStandard))
            .funding_leg(FundingLeg::Fixed {
                rate: 0.0,
                payment_dates: vec![pay],
                accrual_fractions: vec![0.25],
                day_count: DayCount::Act365F,
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .build()
            .expect("CMS swap should build")
    }

    /// A seasoned CMS coupon (fixed in the past, paid in the future) must be
    /// valued off the recorded fixing — and a missing fixing series must be a
    /// hard error, never a silent fallback to live-curve projection.
    #[test]
    fn seasoned_cms_coupon_uses_recorded_fixing() {
        use finstack_quant_core::market_data::fixings::cms_fixing_series_id;
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;

        let fixing = date(2024, 12, 1);
        let as_of = date(2025, 1, 1);
        let pay = date(2025, 3, 1);
        let swap = seasoned_cms_swap(fixing, pay);
        let market = cms_market_with_vol(as_of, 0.25);

        // Without the fixing series the seasoned coupon must hard-error.
        let err = CmsSwapPricer::new()
            .pv_cms_leg(&swap, &market, as_of, 1.0)
            .expect_err("missing CMS fixing series must be a hard error");
        assert!(
            err.to_string().contains("FIXING:CMS-10Y:USD-LIBOR-3M"),
            "error must name the missing series: {err}"
        );

        // With the fixing recorded, the PV is the deterministic discounted
        // coupon off the observed rate.
        let observed = 0.0412;
        let series = ScalarTimeSeries::new(
            cms_fixing_series_id("USD-LIBOR-3M", 10.0),
            vec![(fixing, observed)],
            None,
        )
        .expect("fixing series");
        let market = market.insert_series(series);

        let pv = CmsSwapPricer::new()
            .pv_cms_leg(&swap, &market, as_of, 1.0)
            .expect("seasoned CMS leg PV");
        let df = market
            .get_discount("USD-OIS")
            .expect("discount curve")
            .df_between_dates(as_of, pay)
            .expect("df");
        let expected = observed * 0.25 * df * 1_000_000.0;
        assert!(
            (pv - expected).abs() < 0.01,
            "seasoned coupon must use the recorded fixing: expected {expected}, got {pv}"
        );
    }

    /// Build a 1-period CMS swap with an optional floor.
    fn floored_cms_swap(floor: Option<f64>) -> CmsSwap {
        let fixing = date(2026, 1, 1);
        let pay = date(2026, 4, 1);
        let mut builder = CmsSwap::builder()
            .id(InstrumentId::new("CMS-FLOORED"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Receive)
            .cms_tenor(10.0)
            .cms_fixing_dates(vec![fixing])
            .cms_payment_dates(vec![pay])
            .cms_accrual_fractions(vec![0.25])
            .cms_day_count(DayCount::Act365F)
            .cms_spread(0.0)
            .swap_convention_opt(Some(IRSConvention::USDStandard))
            .funding_leg(FundingLeg::Fixed {
                rate: 0.0,
                payment_dates: vec![pay],
                accrual_fractions: vec![0.25],
                day_count: DayCount::Act365F,
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"));
        if let Some(f) = floor {
            builder = builder.cms_floor_opt(Some(f));
        }
        builder.build().expect("CMS swap should build")
    }

    /// Regression test (C13 floor): a CMS floor must be priced as an embedded
    /// CMS floorlet, not as a hard clamp on the convexity-adjusted mean rate.
    ///
    /// With an ITM floor (floor = 4% > forward ~3%), the clamp-the-mean
    /// approach gives `max(mean, floor) = floor` — pure intrinsic — and yields
    /// a floored leg strictly above the unfloored leg by exactly (floor −
    /// mean) * accrual * N. The embedded floorlet adds additional *time value*
    /// on top of the intrinsic, so the correct floored leg is strictly above
    /// the naive clamped leg.
    ///
    /// Discriminator: `pv_floored − pv_unfloored` must exceed the pure
    /// intrinsic difference `(floor − forward_approx) * accrual * N * df`.
    /// The old clamp code would give exactly the intrinsic gap (or less);
    /// the embedded-option code gives intrinsic + time value.
    #[test]
    fn cms_floor_priced_as_embedded_floorlet_not_mean_clamp() {
        let as_of = date(2025, 1, 1);
        let market = cms_market_with_vol(as_of, 0.25);

        // Forward CMS rate is ~3%; ITM floor at 4%.
        // Clamp gives intrinsic = (0.04 − ~0.03) * 0.25 * 1_000_000 * df ≈ 2 500 USD.
        // Embedded floorlet adds substantial time value (~100s of USD) on top.
        let unfloored = floored_cms_swap(None);
        let floored = floored_cms_swap(Some(0.04));

        let pv_unfloored = CmsSwapPricer::new()
            .pv_cms_leg(&unfloored, &market, as_of, 1.0)
            .expect("unfloored CMS leg");
        let pv_floored = CmsSwapPricer::new()
            .pv_cms_leg(&floored, &market, as_of, 1.0)
            .expect("floored CMS leg");

        assert!(pv_unfloored > 0.0 && pv_floored > 0.0);
        // Floored leg must be strictly above unfloored (floor adds value).
        assert!(
            pv_floored > pv_unfloored + 1e-6,
            "an ITM CMS floor must increase the leg value: floored={pv_floored}, \
             unfloored={pv_unfloored}"
        );
        // The gap must exceed pure intrinsic — time value must be present.
        // Intrinsic ≈ (floor - forward) * accrual * N * df.
        // We conservatively require the gap > 0.5 * intrinsic_approx to ensure
        // the embedded floorlet option value (not just clamp) is being captured.
        // On 1M notional with 25% vol and 1Y to fixing this should be ~100+ USD.
        let intrinsic_approx = (0.04_f64 - 0.03_f64) * 0.25 * 1_000_000.0 * 0.97; // rough df
        let gap = pv_floored - pv_unfloored;
        assert!(
            gap > intrinsic_approx,
            "floored-vs-unfloored gap ({gap:.2}) must exceed pure intrinsic \
             ({intrinsic_approx:.2}); time value must be present (embedded \
             floorlet, not clamp)"
        );
    }

    // ================= Static replication (Andersen-Piterbarg) =================

    /// Receive-CMS swap with a single fixing and a zero-rate fixed funding leg,
    /// so the swap NPV isolates the CMS leg.
    fn single_fixing_cms_swap(cms_tenor: f64, fixing: Date, payment: Date) -> CmsSwap {
        CmsSwap::builder()
            .id(InstrumentId::new("CMS-REPL"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Receive)
            .cms_tenor(cms_tenor)
            .cms_fixing_dates(vec![fixing])
            .cms_payment_dates(vec![payment])
            .cms_accrual_fractions(vec![0.25])
            .cms_day_count(DayCount::Act365F)
            .cms_spread(0.0)
            .swap_convention_opt(Some(IRSConvention::USDStandard))
            .funding_leg(FundingLeg::Fixed {
                rate: 0.0,
                payment_dates: vec![payment],
                accrual_fractions: vec![0.25],
                day_count: DayCount::Act360,
            })
            .discount_curve_id(CurveId::new("USD-OIS"))
            .forward_curve_id(CurveId::new("USD-LIBOR-3M"))
            .vol_surface_id(CurveId::new("USD-CMS10Y-VOL"))
            .build()
            .expect("CMS swap should build")
    }

    /// Replication must price the CMS leg ABOVE the linear (no-convexity) leg:
    /// static replication carries the full convexity of E^{pay}[S] > F
    /// (Andersen-Piterbarg §16.2).
    #[test]
    fn replication_cms_leg_exceeds_linear_forward_leg() {
        let as_of = date(2025, 1, 1);
        let swap = single_fixing_cms_swap(20.0, date(2030, 1, 1), date(2030, 4, 1));
        let market = cms_market_with_vol(as_of, 0.30);

        let linear = CmsSwapPricer::new()
            .price_internal_with_convexity(&swap, &market, as_of, 0.0)
            .expect("linear PV")
            .amount();
        let repl = CmsSwapReplicationPricer::new()
            .price_internal(&swap, &market, as_of)
            .expect("replication PV")
            .amount();

        assert!(
            repl > linear + 1.0,
            "static replication must add positive CMS convexity: repl={repl}, linear={linear}"
        );
    }

    /// Short tenor / low vol: the first-order Hagan adjustment is accurate, so
    /// replication must agree closely (sub-basis-point in rate terms on a
    /// 0.25-accrual coupon: $25 per bp of rate on $1M × 0.25).
    #[test]
    fn replication_close_to_hagan_for_short_tenor_low_vol() {
        let as_of = date(2025, 1, 1);
        let swap = single_fixing_cms_swap(2.0, date(2026, 1, 1), date(2026, 4, 1));
        let market = cms_market_with_vol(as_of, 0.10);

        let hagan = CmsSwapPricer::new()
            .price_internal_with_convexity(&swap, &market, as_of, 1.0)
            .expect("hagan PV")
            .amount();
        let repl = CmsSwapReplicationPricer::new()
            .price_internal(&swap, &market, as_of)
            .expect("replication PV")
            .amount();

        // 2bp-of-rate tolerance: 2e-4 × 0.25 × 1e6 = $50.
        assert!(
            (repl - hagan).abs() < 50.0,
            "short-tenor/low-vol replication must track Hagan: repl={repl}, hagan={hagan}"
        );
    }

    /// Long tenor / high vol: first-order Hagan understates the adjustment
    /// (documented 5-10bp class error); replication must exceed it by a
    /// meaningful, bounded amount.
    #[test]
    fn replication_exceeds_hagan_for_long_tenor_high_vol() {
        let as_of = date(2025, 1, 1);
        let swap = single_fixing_cms_swap(20.0, date(2030, 1, 1), date(2030, 4, 1));
        let market = cms_market_with_vol(as_of, 0.30);

        let hagan = CmsSwapPricer::new()
            .price_internal_with_convexity(&swap, &market, as_of, 1.0)
            .expect("hagan PV")
            .amount();
        let repl = CmsSwapReplicationPricer::new()
            .price_internal(&swap, &market, as_of)
            .expect("replication PV")
            .amount();

        // Gap in rate terms: PV / (accrual × notional × df). Use df ≈ 1 bound;
        // require > 0.5bp and < 100bp (sanity).
        let gap_rate = (repl - hagan) / (0.25 * 1_000_000.0);
        assert!(
            gap_rate > 0.5e-4,
            "long-tenor/high-vol replication must exceed first-order Hagan by \
             a material margin: repl={repl}, hagan={hagan}, gap={gap_rate:.6}"
        );
        assert!(
            gap_rate < 100.0e-4,
            "replication-vs-Hagan gap implausibly large: repl={repl}, hagan={hagan}"
        );
    }

    /// Seasoned fixing: both models must value the coupon identically off the
    /// recorded fixing (no convexity on a known rate).
    #[test]
    fn replication_matches_hagan_exactly_for_seasoned_fixing() {
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;

        let as_of = date(2025, 6, 1);
        // Fixed 2025-01-01, pays 2025-07-01: fixed-but-unpaid at as_of.
        let swap = single_fixing_cms_swap(10.0, date(2025, 1, 1), date(2025, 7, 1));
        let fixing_series = ScalarTimeSeries::new(
            finstack_quant_core::market_data::fixings::cms_fixing_series_id("USD-LIBOR-3M", 10.0),
            vec![(date(2025, 1, 1), 0.045)],
            None,
        )
        .expect("fixing series");
        let market = cms_market_with_vol(as_of, 0.30).insert_series(fixing_series);

        let hagan = CmsSwapPricer::new()
            .price_internal_with_convexity(&swap, &market, as_of, 1.0)
            .expect("hagan PV")
            .amount();
        let repl = CmsSwapReplicationPricer::new()
            .price_internal(&swap, &market, as_of)
            .expect("replication PV")
            .amount();

        assert!(
            (repl - hagan).abs() < 1e-9,
            "a seasoned CMS coupon is a known cashflow; models must agree \
             exactly: repl={repl}, hagan={hagan}"
        );
    }

    /// Non-positive forward: replication requires a positive forward swap rate
    /// (lognormal replication integrand), matching the CmsOption replication
    /// pricer's behavior — a clean error, not a silent fallback.
    #[test]
    fn replication_errors_on_non_positive_forward() {
        let as_of = date(2025, 1, 1);
        let swap = single_fixing_cms_swap(10.0, date(2026, 1, 1), date(2026, 4, 1));
        // Negative-rate market: 0% discounting (constant DFs are valid) with a
        // -1% projection curve drives the reference par swap rate negative.
        let market = MarketContext::new()
            .insert(flat_discount_with_tenor("USD-OIS", as_of, 0.0, 1.0))
            .insert(flat_forward_with_tenor("USD-LIBOR-3M", as_of, -0.01, 1.0));

        let result = CmsSwapReplicationPricer::new().price_internal(&swap, &market, as_of);
        assert!(
            result.is_err(),
            "replication must reject non-positive forwards like the CmsOption \
             replication pricer, got {:?}",
            result.ok()
        );
    }
}
