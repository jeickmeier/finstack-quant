//! CMS swap pricer with convexity adjustment.
//!
//! Prices a CMS swap by:
//! 1. **CMS leg**: For each period, compute the forward CMS rate (par swap rate
//!    for the reference tenor), apply the Hagan (2003) convexity adjustment, add
//!    spread, optionally apply cap/floor, discount and sum.
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
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext, PricingResult,
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

            let mut adjusted_rate = forward_swap_rate + adj + inst.cms_spread;

            if let Some(cap) = inst.cms_cap {
                adjusted_rate = adjusted_rate.min(cap);
            }
            if let Some(floor) = inst.cms_floor {
                adjusted_rate = adjusted_rate.max(floor);
            }

            let df_pay = relative_df_discount_curve(discount_curve.as_ref(), as_of, payment_date)?;

            total_pv += adjusted_rate * accrual_fraction * df_pay * inst.notional.amount();
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
    ) -> PricingResult<ValuationResult> {
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
}
