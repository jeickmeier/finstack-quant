//! Finite-difference sensitivities to a parallel inflation-curve shift.
//!
//! Shared by the zero-coupon and YoY inflation swaps and the inflation
//! cap/floor. Every calculator bumps the instrument's inflation curve by ±1bp
//! with [`BumpSpec::inflation_shift_pct`] and reprices.
//!
//! - [`Inflation01Calculator`]: `(PV(+1bp) − PV(−1bp)) / 2`, the PV change per
//!   1bp move (the DV01/CS01 convention; see `sensitivity_central_diff`).
//! - [`InflationGammaCalculator`]: `(PV(+h) − 2·PV + PV(−h)) / h²` with
//!   `h = 1bp = 0.0001`, the raw second derivative `d²PV/dπ²` per unit² of
//!   inflation rate (divide by 1e8 for a per-bp² view). All three PVs are
//!   unrounded so Money quantization is not amplified by the `h² = 1e-8`
//!   divisor. Convexity is non-zero even for a par swap (PV = 0): it measures
//!   curvature, not level.
//!
//! The bumped-curve PVs carry the instrument's leg signs, so a pay-fixed
//! inflation swap has a positive Inflation01 without an explicit branch.

use std::marker::PhantomData;

use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// A 1bp inflation-curve shift in `BumpSpec::inflation_shift_pct` units
/// (percent: 0.01% = 1bp).
pub(crate) const INFLATION_BUMP_PCT: f64 = 0.01;

/// The same 1bp shift as a decimal rate: the step `h` of the second difference.
const INFLATION_BUMP_DECIMAL: f64 = 0.0001;

/// An instrument priced off one inflation curve.
pub(crate) trait InflationIndexed: Instrument {
    /// Identifier of the inflation curve the instrument projects index
    /// levels from; the finite-difference calculators bump this curve.
    fn inflation_index_id(&self) -> &CurveId;
}

/// `context.curves` with the inflation curve `id` shifted by `shift_pct`
/// percent (`INFLATION_BUMP_PCT` = 1bp).
pub(crate) fn bumped_inflation_market(
    context: &MetricContext,
    id: &CurveId,
    shift_pct: f64,
) -> Result<MarketContext> {
    context.curves.as_ref().bump([MarketBump::Curve {
        id: id.clone(),
        spec: BumpSpec::inflation_shift_pct(shift_pct),
    }])
}

/// Inflation01 of an inflation swap: the PV change for a 1bp parallel
/// inflation-curve move, by central difference of the instrument's `value`.
pub(crate) struct Inflation01Calculator<I>(PhantomData<fn() -> I>);

impl<I> Inflation01Calculator<I> {
    /// Calculator for instruments of type `I`.
    pub(crate) const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<I: InflationIndexed + 'static> MetricCalculator for Inflation01Calculator<I> {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &I = context.instrument_as()?;
        let as_of = context.as_of;
        let id = instrument.inflation_index_id();
        let curves_up = bumped_inflation_market(context, id, INFLATION_BUMP_PCT)?;
        let pv_up = instrument.value(&curves_up, as_of)?.amount();
        let curves_down = bumped_inflation_market(context, id, -INFLATION_BUMP_PCT)?;
        let pv_down = instrument.value(&curves_down, as_of)?.amount();
        Ok((pv_up - pv_down) / 2.0)
    }
}

/// Second derivative of PV to a parallel inflation-curve shift (inflation
/// swap convexity, inflation cap/floor gamma), from unrounded reprices on the
/// active dispatch path.
pub(crate) struct InflationGammaCalculator<I>(PhantomData<fn() -> I>);

impl<I> InflationGammaCalculator<I> {
    /// Calculator for instruments of type `I`.
    pub(crate) const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<I: InflationIndexed + 'static> MetricCalculator for InflationGammaCalculator<I> {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let id = context.instrument_as::<I>()?.inflation_index_id().clone();
        let as_of = context.as_of;
        let base_pv = context.reprice_raw(context.curves.as_ref(), as_of)?;
        let curves_up = bumped_inflation_market(context, &id, INFLATION_BUMP_PCT)?;
        let pv_up = context.reprice_raw(&curves_up, as_of)?;
        let curves_down = bumped_inflation_market(context, &id, -INFLATION_BUMP_PCT)?;
        let pv_down = context.reprice_raw(&curves_down, as_of)?;
        let h_squared = INFLATION_BUMP_DECIMAL * INFLATION_BUMP_DECIMAL;
        Ok((pv_up - 2.0 * base_pv + pv_down) / h_squared)
    }
}

impl InflationIndexed for crate::inflation_swap::InflationSwap {
    fn inflation_index_id(&self) -> &CurveId {
        &self.inflation_index_id
    }
}

impl InflationIndexed for crate::inflation_swap::YoYInflationSwap {
    fn inflation_index_id(&self) -> &CurveId {
        &self.inflation_index_id
    }
}

impl InflationIndexed for crate::inflation_cap_floor::InflationCapFloor {
    fn inflation_index_id(&self) -> &CurveId {
        &self.inflation_index_id
    }
}

#[cfg(test)]
mod inflation_swap_inflation01_tests {
    use crate::inflation_swap::InflationSwap;
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
    use finstack_quant_core::math::units::ONE_BASIS_POINT;
    use finstack_quant_core::types::CurveId;
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_core::instruments::PricingOptions;
    use finstack_quant_valuations_core::metrics::MetricId;
    use time::macros::date;

    fn market(as_of: finstack_quant_core::dates::Date) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (10.0, 0.7)])
            .build()
            .expect("discount curve");
        // Use an epoch-anchored inflation curve so time fractions are computed
        // from the curve's own base_date, not the valuation date. This avoids
        // negative-time lookups when the lagged CPI date precedes as_of.
        let epoch =
            finstack_quant_core::dates::Date::from_calendar_date(1970, time::Month::January, 1)
                .expect("epoch");
        let inflation = InflationCurve::builder("US-CPI")
            .base_date(epoch)
            .base_cpi(100.0)
            .knots([(0.0, 100.0), (55.0, 155.0), (60.0, 180.0)])
            .build()
            .expect("inflation curve");
        MarketContext::new().insert(discount).insert(inflation)
    }

    fn sample_swap() -> InflationSwap {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::market_data::scalars::InflationLag;
        use rust_decimal::Decimal;
        use time::Month;
        // Build a swap whose start_date is 2024-01-15, maturity 2029-01-15.
        // as_of will be 2023-07-01 — well before start, so all CPI lookups lie
        // in the forward region of the inflation curve.
        InflationSwap::builder()
            .id(finstack_quant_core::types::InstrumentId::new("TEST-SWAP"))
            .notional(finstack_quant_core::money::Money::from((
                1_000_000_i64,
                Currency::USD,
            )))
            .start_date(
                finstack_quant_core::dates::Date::from_calendar_date(2024, Month::January, 15)
                    .expect("start"),
            )
            .maturity(
                finstack_quant_core::dates::Date::from_calendar_date(2029, Month::January, 15)
                    .expect("maturity"),
            )
            .fixed_rate(Decimal::try_from(0.02).expect("rate"))
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .side(finstack_quant_valuations_core::instruments::PayReceive::Pay)
            .lag(InflationLag::None)
            .attributes(finstack_quant_core::types::Attributes::new())
            .build()
            .expect("swap")
    }

    /// Inflation01 must equal `(pv_up - pv_down) / 2`, not
    /// `(pv_up - pv_down) / (2 * bump_decimal)`.
    ///
    /// The "…01" convention throughout the workspace (DV01, CS01, Inflation01)
    /// is PV-change-per-1bp. A ±1bp central bump produces that directly as
    /// `(pv_up - pv_down) / 2`. Dividing by the decimal bump value (0.0002)
    /// instead yields the per-*unit* derivative — 10 000× too large — which is
    /// the bug this test guards against.
    #[test]
    fn inflation01_is_per_1bp_not_per_unit() {
        let as_of = date!(2023 - 07 - 01);
        let swap = sample_swap();
        let market = market(as_of);

        // Compute the two bumped PVs the same way the calculator does.
        let bump = ONE_BASIS_POINT;
        let curves_up = market
            .bump([MarketBump::Curve {
                id: swap.inflation_index_id.clone(),
                spec: BumpSpec::inflation_shift_pct(bump * 100.0),
            }])
            .expect("bump up");
        let curves_down = market
            .bump([MarketBump::Curve {
                id: swap.inflation_index_id.clone(),
                spec: BumpSpec::inflation_shift_pct(-bump * 100.0),
            }])
            .expect("bump down");
        let pv_up = swap.value(&curves_up, as_of).expect("pv up").amount();
        let pv_down = swap.value(&curves_down, as_of).expect("pv down").amount();

        // Expected: per-1bp sensitivity — the half-difference.
        let expected_per_1bp = (pv_up - pv_down) / 2.0;
        // The old (buggy) value would be 10 000× larger.
        let buggy_per_unit = (pv_up - pv_down) / (2.0 * bump);

        let result = swap
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Inflation01],
                PricingOptions::default(),
            )
            .expect("price with metrics");
        let reported = *result
            .measures
            .get("inflation01")
            .expect("inflation01 measure");

        // Must match the per-1bp value.
        assert!(
            (reported - expected_per_1bp).abs() < 1e-8 * expected_per_1bp.abs().max(1.0),
            "Inflation01 {reported} should equal per-1bp value {expected_per_1bp}"
        );
        // Must NOT match the per-unit (buggy) value.
        assert!(
            (reported - buggy_per_unit).abs() > expected_per_1bp.abs() * 100.0,
            "Inflation01 {reported} must NOT equal the per-unit (10 000×-inflated) value {buggy_per_unit}"
        );
    }

    /// Inflation01 sign: a pay-fixed inflation swap benefits from rising
    /// inflation, so Inflation01 must be positive.
    #[test]
    fn inflation01_sign_positive_for_pay_fixed() {
        let as_of = date!(2023 - 07 - 01);
        let swap = sample_swap();
        let market = market(as_of);

        let result = swap
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Inflation01],
                PricingOptions::default(),
            )
            .expect("price with metrics");
        let reported = *result
            .measures
            .get("inflation01")
            .expect("inflation01 measure");

        assert!(
            reported > 0.0,
            "Pay-fixed Inflation01 should be positive (higher inflation → higher PV), got {reported}"
        );
    }
}
