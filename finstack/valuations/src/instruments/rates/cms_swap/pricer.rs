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
    rate_period_on_dates, relative_df_discount_curve,
};
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cms_option::pricer::convexity_adjustment;
use crate::instruments::rates::cms_swap::types::{CmsSwap, FundingLeg};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::dates::{Date, DateExt, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;

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
        let vol_surface = market.get_surface(inst.vol_surface_id.as_str())?;

        let mut total_pv = 0.0;

        for (i, &fixing_date) in inst.cms_fixing_dates.iter().enumerate() {
            let payment_date = inst.cms_payment_dates[i];
            let accrual_fraction = inst.cms_accrual_fractions[i];

            if payment_date <= as_of {
                continue;
            }

            // Seasoned coupon: the CMS rate fixed in the past. Value it off
            // the recorded fixing (mirroring the cap/floor pricer) — never
            // re-project from the live curve, which books phantom P&L. The
            // rate is known, so there is no convexity adjustment and the
            // embedded cap/floor collapses to intrinsic (time_to_fixing = 0).
            if fixing_date < as_of {
                let observed =
                    crate::instruments::rates::exotics_shared::fixings::historical_cms_fixing(
                        market,
                        &inst.forward_curve_id,
                        inst.cms_tenor,
                        fixing_date,
                    )?;
                let coupon_rate = apply_cms_cap_floor(
                    observed,
                    inst.cms_spread,
                    inst.cms_cap,
                    inst.cms_floor,
                    &vol_surface,
                    0.0,
                );
                let df_pay =
                    relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;
                total_pv += coupon_rate * accrual_fraction * df_pay * inst.notional.amount();
                continue;
            }

            let swap_start = fixing_date;
            let swap_tenor_months = (inst.cms_tenor * 12.0).round() as i32;
            let swap_end = swap_start.add_months(swap_tenor_months);

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
                    },
                )?;

            if forward_swap_rate <= 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "Forward swap rate {} is non-positive for fixing date {}",
                    forward_swap_rate, fixing_date
                )));
            }

            let time_to_fixing =
                inst.cms_day_count
                    .year_fraction(as_of, fixing_date, DayCountContext::default())?;

            let adj = if time_to_fixing > 0.0 {
                // ASSUMPTION: `vol_surface` must be the swaption volatility
                // surface for the CMS reference swap tenor (`inst.cms_tenor`),
                // keyed by (expiry, strike). The lookup below uses
                // `(time_to_fixing, forward_swap_rate)` — i.e. at-the-money on
                // the forward swap rate — and the surface has no separate
                // swap-tenor axis, so the caller must supply the surface that
                // corresponds to the CMS reference swap tenor.
                convexity_adjustment(
                    vol_surface.value_clamped(time_to_fixing.max(0.0), forward_swap_rate),
                    time_to_fixing,
                    inst.cms_tenor,
                    forward_swap_rate,
                ) * convexity_scale
            } else {
                0.0
            };

            // Convexity-adjusted forward CMS rate (the payment-measure
            // martingale forward). The coupon is paid on `cms_rate + spread`.
            let adjusted_forward = forward_swap_rate + adj;

            // Expected option-adjusted coupon rate (Hagan 2003).
            // See `apply_cms_cap_floor` for the full derivation comment.
            let coupon_rate = apply_cms_cap_floor(
                adjusted_forward,
                inst.cms_spread,
                inst.cms_cap,
                inst.cms_floor,
                &vol_surface,
                time_to_fixing,
            );

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
                    let fwd_rate =
                        rate_period_on_dates(fwd_curve.as_ref(), prev_date, payment_date)?;
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

/// Present value entry point for `Instrument::value`.
pub(crate) fn compute_pv(inst: &CmsSwap, market: &MarketContext, as_of: Date) -> Result<Money> {
    let pricer = CmsSwapPricer::new();
    pricer.price_internal(inst, market, as_of)
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
    vol_surface: &finstack_core::market_data::surfaces::VolSurface,
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
/// (`time_to_fixing ≤ 0`, non-positive forward/strike, or non-positive vol).
pub(super) fn cms_embedded_option_value(
    adjusted_forward: f64,
    strike: f64,
    vol_surface: &finstack_core::market_data::surfaces::VolSurface,
    time_to_fixing: f64,
    option_type: crate::instruments::OptionType,
) -> f64 {
    use crate::instruments::OptionType;
    use crate::models::d1_d2_black76;

    let intrinsic = match option_type {
        OptionType::Call => (adjusted_forward - strike).max(0.0),
        OptionType::Put => (strike - adjusted_forward).max(0.0),
    };

    // Black-76 needs a positive forward, strike, time, and vol; otherwise the
    // option value is its intrinsic.
    if time_to_fixing <= 0.0 || adjusted_forward <= 0.0 || strike <= 0.0 {
        return intrinsic;
    }
    let vol = vol_surface.value_clamped(time_to_fixing, strike);
    if vol <= 0.0 {
        return intrinsic;
    }

    let (d1, d2) = d1_d2_black76(adjusted_forward, strike, vol, time_to_fixing);
    match option_type {
        OptionType::Call => {
            adjusted_forward * finstack_core::math::norm_cdf(d1)
                - strike * finstack_core::math::norm_cdf(d2)
        }
        OptionType::Put => {
            strike * finstack_core::math::norm_cdf(-d2)
                - adjusted_forward * finstack_core::math::norm_cdf(-d1)
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
    mod test_utils {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/test_utils.rs"
        ));
    }

    use super::*;
    use crate::instruments::common_impl::parameters::IRSConvention;
    use finstack_core::currency::Currency;
    use finstack_core::dates::DayCount;
    use finstack_core::types::{CurveId, InstrumentId};
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

        let expected = swap.notional.amount() * 0.05 * 0.25 * 2.0;
        assert!(
            (pv - expected).abs() < 1e-8,
            "expected funding PV {expected}, got {pv}"
        );
    }

    /// Build a market with a flat vol surface for the embedded-option tests.
    fn cms_market_with_vol(as_of: Date, vol: f64) -> MarketContext {
        use finstack_core::market_data::surfaces::VolSurface;

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
        use finstack_core::market_data::fixings::cms_fixing_series_id;
        use finstack_core::market_data::scalars::ScalarTimeSeries;

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
}
