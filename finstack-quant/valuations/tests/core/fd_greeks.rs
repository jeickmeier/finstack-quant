//! Finite-difference greeks over a built-in option.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{GenericFdVanna, GenericFdVega, GenericFdVolga};
use std::sync::Arc;
use time::macros::date;

#[test]
fn generic_greeks_follow_the_active_override_without_a_surface() {
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::{surfaces::VolSurface, term_structures::DiscountCurve};
    use finstack_quant_valuations::instruments::EquityOption;
    use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext};
    let as_of = date!(2025 - 01 - 01);
    let mut option = EquityOption::example().expect("option");
    option.expiry = date!(2026 - 01 - 01);
    option.strike = 120.0;
    option.spot_id = "EQ-SPOT".into();
    option.vol_surface_id = "EQ-VOL".into();
    option.div_yield_id = None;
    let bare = MarketContext::new()
        .insert(
            DiscountCurve::builder("USD-OIS")
                .base_date(as_of)
                .knots([(0.0, 1.0), (10.0, 1.0)])
                .build()
                .expect("curve"),
        )
        .insert_price("EQ-SPOT", MarketScalar::Unitless(100.0));
    let quoted = bare.clone().insert_surface(
        VolSurface::builder("EQ-VOL")
            .expiries(&[1.0])
            .strikes(&[120.0])
            .row(&[0.2])
            .build()
            .expect("surface"),
    );
    let evaluate = |inst: &EquityOption, market: &MarketContext, calc: &dyn MetricCalculator| {
        let base = inst.value(market, as_of).expect("PV");
        let mut context = MetricContext::new(
            Arc::new(inst.clone()),
            Arc::new(market.clone()),
            as_of,
            base,
            MetricContext::default_config(),
        );
        calc.calculate(&mut context).expect("active quote Greek")
    };
    let mut overridden = option.clone();
    overridden.instrument_pricing_overrides = overridden
        .instrument_pricing_overrides
        .with_implied_volatility(0.2);
    let calculators: Vec<Box<dyn MetricCalculator>> = vec![
        Box::new(GenericFdVega::<EquityOption>::default()),
        Box::new(GenericFdVolga::<EquityOption>::default()),
        Box::new(GenericFdVanna::<EquityOption>::default()),
    ];
    for calc in calculators {
        let expected = evaluate(&option, &quoted, calc.as_ref());
        assert!(expected.abs() > 1e-6);
        for market in [&quoted, &bare] {
            let actual = evaluate(&overridden, market, calc.as_ref());
            assert!((actual - expected).abs() < 1e-8, "{actual} vs {expected}");
        }
    }
}
