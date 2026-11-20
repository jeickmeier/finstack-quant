//! A swaption's shared discount/projection source is one DV01 factor.

use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::{
    VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions, RatesCurveKind};
use finstack_quant_valuations_core::metrics::sensitivities::dv01::{
    Dv01CalculatorConfig, UnifiedDv01Calculator,
};
use finstack_quant_valuations_core::metrics::{MetricId, bump_discount_curve_parallel};
use finstack_quant_valuations_rates::swaption::Swaption;
use time::macros::date;

#[test]
fn swaption_single_curve_dv01_bumps_the_shared_source_once() {
    let as_of = date!(2026 - 01 - 15);
    let mut swaption = Swaption::example().expect("single-curve swaption");
    let curve_id = swaption.get_discount_curve_id().clone();
    let discount = DiscountCurve::builder(curve_id.clone())
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([0.0, 1.0, 5.0, 10.0].map(|t| (t, (-0.03_f64 * t).exp())))
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let vols = VolSurface::builder(swaption.vol_surface_id.clone())
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .expiries(&[0.25, 5.0])
        .secondary_values(&[0.01, 0.05])
        .row(&[0.20; 2])
        .row(&[0.20; 2])
        .build()
        .expect("volatility grid");
    let market = MarketContext::new()
        .try_insert(discount)
        .expect("insert discount")
        .insert_surface(vols);
    let calculator = UnifiedDv01Calculator::new(Dv01CalculatorConfig::parallel_combined());

    assert_eq!(
        calculator.collect_curves(&swaption, &market).unwrap(),
        [(curve_id.clone(), RatesCurveKind::Discount)]
    );
    let priced = swaption
        .price_with_metrics(&market, as_of, &[MetricId::Dv01], PricingOptions::default())
        .expect("same-ID swaption prices with DV01");
    assert!(priced.value.amount() > 0.0);
    let pv_at = |bp| {
        let bumped = bump_discount_curve_parallel(&market, &curve_id, bp).unwrap();
        swaption.value(&bumped, as_of).unwrap().amount()
    };
    let expected = (pv_at(1.0) - pv_at(-1.0)) / 2.0;
    assert!(expected > 0.0);
    assert!((priced.measures["dv01"] - expected).abs() < 1e-8);

    swaption.underlying_float_leg.forward_curve_id = "MISSING-PROJECTION".into();
    let error = calculator
        .collect_curves(&swaption, &market)
        .expect_err("a distinct projection curve is still required");
    assert!(error.to_string().contains("MISSING-PROJECTION"));
}
