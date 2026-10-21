//! InflationSwap metrics module.
//!
//! Provides metric calculators specific to `InflationSwap`. The calculators compose with the shared metrics framework and are
//! registered via `register_inflation_swap_metrics`.
//!
//! Exposed metrics:
//! - Breakeven inflation
//! - Fixed leg PV
//! - Inflation leg PV
//! - DV01 and bucketed DV01
//! - Inflation01 and inflation convexity (central finite differences on the
//!   inflation curve, see `crate::inflation_fd`)

use crate::inflation_fd::{Inflation01Calculator, InflationGammaCalculator};
use crate::inflation_swap::InflationSwap;
use crate::inflation_swap::YoYInflationSwap;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all inflation swap metrics with the registry
pub(crate) fn register_inflation_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    // Note: `Npv01` is intentionally NOT registered — it was an exact
    // duplicate of `Dv01` (same `parallel_combined` config). Use `Dv01`.
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::InflationSwap,
        metrics: [
            (ParRate, ParRateCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (Inflation01, Inflation01Calculator::<InflationSwap>::new()),
            (InflationConvexity, InflationGammaCalculator::<InflationSwap>::new()),
        ],
        custom: [
            // Breakeven inflation is the par fixed rate of the zero-coupon swap.
            ("breakeven", ParRateCalculator),
            ("fixed_leg_pv", FixedLegPvCalculator),
            ("inflation_leg_pv", InflationLegPvCalculator),
        ]
    }

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::YoYInflationSwap,
        metrics: [
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (Inflation01, Inflation01Calculator::<YoYInflationSwap>::new()),
        ]
    }
    Ok(())
}

/// Calculates PV of fixed leg for inflation swaps.
pub(crate) struct FixedLegPvCalculator;

impl MetricCalculator for FixedLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let s: &InflationSwap = context.instrument_as()?;
        let pv_fixed = s.pv_fixed_leg(&context.curves, context.as_of)?;
        Ok(pv_fixed.amount())
    }
}

/// Calculates PV of inflation leg for inflation swaps.
pub(crate) struct InflationLegPvCalculator;

impl MetricCalculator for InflationLegPvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let s: &InflationSwap = context.instrument_as()?;
        let pv_inflation = s.pv_inflation_leg(&context.curves, context.as_of)?;
        Ok(pv_inflation.amount())
    }
}

/// Calculates the par real rate for an inflation swap.
pub(crate) struct ParRateCalculator;

impl MetricCalculator for ParRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let swap: &InflationSwap = context.instrument_as()?;
        swap.par_rate(context.curves.as_ref(), context.as_of)
    }
}

#[cfg(test)]
mod yoy_inflation01_tests {
    use crate::inflation_swap::YoYInflationSwap;
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
            .frequency(Tenor::annual())
            .inflation_index_id(CurveId::new("US-CPI"))
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .side(finstack_quant_valuations_core::instruments::PayReceive::Pay)
            .lag(InflationLag::None)
            .attributes(finstack_quant_valuations_core::instruments::Attributes::new())
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
