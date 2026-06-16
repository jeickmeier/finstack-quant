//! Inflation01 calculator for YoY inflation swaps.
//!
//! Computes the YoY inflation curve sensitivity per 1bp using central finite
//! differences on the inflation curve.
//!
//! # Finite-Difference Method
//!
//! ```text
//! Inflation01 = (PV(+1bp) − PV(−1bp)) / 2
//! ```
//!
//! Dividing by 2 (not by 2 × bump_size_decimal) produces the PV change for a
//! **1bp move**, consistent with the DV01/CS01/Inflation01 convention throughout
//! the workspace (see `sensitivity_central_diff` and the zero-coupon
//! `Inflation01Calculator`).

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::inflation_swap::YoYInflationSwap;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::Result;

/// Standard inflation curve bump: 1bp (0.0001 in decimal).
pub(crate) const INFLATION_BUMP_BP: f64 = 0.0001;

/// Inflation01 calculator for YoY inflation swaps.
pub(crate) struct YoYInflation01Calculator;

impl MetricCalculator for YoYInflation01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap: &YoYInflationSwap = context.instrument_as()?;
        let as_of = context.as_of;

        // Bump the inflation curve up by 1bp and reprice.
        let bump_spec = BumpSpec::inflation_shift_pct(INFLATION_BUMP_BP * 100.0);
        let curves_up = context.curves.as_ref().bump([MarketBump::Curve {
            id: swap.inflation_index_id.clone(),
            spec: bump_spec,
        }])?;
        let pv_up = swap.value(&curves_up, as_of)?.amount();

        // Bump the inflation curve down by 1bp and reprice.
        let bump_spec_down = BumpSpec::inflation_shift_pct(-INFLATION_BUMP_BP * 100.0);
        let curves_down = context.curves.as_ref().bump([MarketBump::Curve {
            id: swap.inflation_index_id.clone(),
            spec: bump_spec_down,
        }])?;
        let pv_down = swap.value(&curves_down, as_of)?.amount();

        // Central difference: Inflation01 = (PV_up - PV_down) / 2.
        //
        // The curve was bumped by exactly ±1bp (INFLATION_BUMP_BP = 0.0001).
        // A symmetric ±1bp central difference already produces the PV change
        // for a 1bp move; no further normalization by the decimal bump value is
        // needed. Dividing by 2 (not by 2 × INFLATION_BUMP_BP) is the correct
        // per-1bp convention — consistent with DV01 and CS01 in the workspace.
        Ok((pv_up - pv_down) / 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::INFLATION_BUMP_BP;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::rates::inflation_swap::YoYInflationSwap;
    use crate::instruments::PricingOptions;
    use crate::metrics::MetricId;
    use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, InflationCurve};
    use finstack_quant_core::types::CurveId;
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

    fn sample_swap() -> YoYInflationSwap {
        use finstack_quant_core::currency::Currency;
        use finstack_quant_core::dates::{DayCount, Tenor};
        use finstack_quant_core::market_data::scalars::InflationLag;
        use rust_decimal::Decimal;
        use time::Month;

        YoYInflationSwap::builder()
            .id(finstack_quant_core::types::InstrumentId::new(
                "TEST-YOY-SWAP",
            ))
            .notional(finstack_quant_core::money::Money::new(
                1_000_000.0,
                Currency::USD,
            ))
            .start_date(
                finstack_quant_core::dates::Date::from_calendar_date(2024, Month::January, 15)
                    .expect("start"),
            )
            .maturity(
                finstack_quant_core::dates::Date::from_calendar_date(2029, Month::January, 15)
                    .expect("maturity"),
            )
            .fixed_rate(Decimal::try_from(0.02).expect("rate"))
            .frequency(Tenor::annual())
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(crate::instruments::common_impl::parameters::legs::PayReceive::Pay)
            .lag_override(InflationLag::None)
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("yoy swap")
    }

    /// YoY Inflation01 must equal `(pv_up - pv_down) / 2`, not
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
        let bump = INFLATION_BUMP_BP;
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
            "YoY Inflation01 {reported} should equal per-1bp value {expected_per_1bp}"
        );
        // Must NOT match the per-unit (buggy) value.
        assert!(
            (reported - buggy_per_unit).abs() > expected_per_1bp.abs() * 100.0,
            "YoY Inflation01 {reported} must NOT equal the per-unit (10 000×-inflated) value {buggy_per_unit}"
        );
    }

    /// YoY Inflation01 sign: a pay-fixed inflation swap benefits from rising
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
            "Pay-fixed YoY Inflation01 should be positive (higher inflation → higher PV), got {reported}"
        );
    }
}
