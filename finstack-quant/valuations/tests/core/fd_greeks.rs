//! Finite-difference Greeks over built-in options.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::sensitivities::fd_greeks::{
    GenericFdVanna, GenericFdVega, GenericFdVolga,
};
use std::sync::Arc;
use time::macros::date;

#[test]
fn generic_greeks_follow_the_active_override_without_a_surface() {
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::{surfaces::VolSurface, term_structures::DiscountCurve};
    use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
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

mod asian {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
    use finstack_quant_models::types::OptionType;
    use finstack_quant_valuations::instruments::exotics::asian_option::AsianOption;
    use finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides;
    use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
    use finstack_quant_valuations::metrics::MetricId;
    use finstack_quant_valuations::pricer::{ModelKey, STANDARD};
    use time::macros::date;

    fn option() -> AsianOption {
        AsianOption::builder()
            .id("ASIAN-FD-GREEKS".into())
            .underlying_ticker("SPX".to_string())
            .strike(100.0)
            .option_type(OptionType::Call)
            .averaging_method(AveragingMethod::Arithmetic)
            .expiry(date!(2026 - 01 - 01))
            .fixing_dates(vec![date!(2025 - 07 - 01), date!(2026 - 01 - 01)])
            .quantity(100.0)
            .currency(Currency::USD)
            .day_count(DayCount::Act365F)
            .discount_curve_id("USD-OIS".into())
            .spot_id("SPX".into())
            .vol_surface_id("SPX-VOL".into())
            .div_yield_id_opt(None)
            .instrument_pricing_overrides(InstrumentPricingOverrides::default().with_mc_paths(200))
            .attributes(Default::default())
            .build()
            .expect("valid Asian option")
    }

    #[test]
    fn asian_mc_registry_prices_all_five_fd_greeks() {
        let as_of = date!(2025 - 01 - 01);
        let option = option();
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .day_count(DayCount::Act365F)
                    .knots([(0.0, 1.0), (2.0, (-0.05_f64 * 2.0).exp())])
                    .build()
                    .expect("discount curve"),
            )
            .insert_surface(
                VolSurface::builder("SPX-VOL")
                    .expiries(&[0.5, 1.0, 2.0])
                    .strikes(&[80.0, 100.0, 120.0])
                    .row(&[0.25; 3])
                    .row(&[0.25; 3])
                    .row(&[0.25; 3])
                    .build()
                    .expect("volatility surface"),
            )
            .insert_price("SPX", MarketScalar::Unitless(100.0));
        let metrics = [
            MetricId::Delta,
            MetricId::Gamma,
            MetricId::Vega,
            MetricId::Vanna,
            MetricId::Volga,
        ];

        assert_eq!(Instrument::expiry(&option), Some(option.expiry));
        let result = STANDARD
            .pricers()
            .price_with_metrics(
                &option,
                ModelKey::MonteCarloGBM,
                &market,
                as_of,
                &metrics,
                PricingOptions::default(),
            )
            .expect("Asian MC price and all five finite-difference Greeks");

        assert_eq!(result.value.currency(), Currency::USD);
        assert!(result.value.amount().is_finite() && result.value.amount() > 0.0);
        assert_eq!(
            result.provenance.as_ref().expect("model provenance").model,
            ModelKey::MonteCarloGBM,
        );
        for metric in metrics {
            let value = result.measures.get(&metric).expect("requested Greek");
            assert!(value.is_finite(), "{metric} must be finite, got {value}");
        }
    }

    #[test]
    fn asian_mc_registry_expired_vanna_volga_need_only_realized_fixings() {
        let mut option = option();
        option.past_fixings = option
            .fixing_dates
            .iter()
            .copied()
            .zip([100.0, 120.0])
            .collect();
        let market = MarketContext::new();
        let metrics = [MetricId::Vanna, MetricId::Volga];
        let registry = STANDARD.pricers();

        for as_of in [option.expiry, date!(2026 - 01 - 02)] {
            let result = registry
                .price_with_metrics(
                    &option,
                    ModelKey::MonteCarloGBM,
                    &market,
                    as_of,
                    &metrics,
                    PricingOptions::default(),
                )
                .expect("expired Asian Greeks must not require live spot or volatility");
            assert_eq!(result.value, Money::from((1_000_i64, Currency::USD)));
            assert_eq!(result.measures.get(&MetricId::Vanna), Some(&0.0));
            assert_eq!(result.measures.get(&MetricId::Volga), Some(&0.0));
        }

        option.past_fixings.pop();
        let error = registry
            .price_with_metrics(
                &option,
                ModelKey::MonteCarloGBM,
                &market,
                option.expiry,
                &metrics,
                PricingOptions::default(),
            )
            .expect_err("expired pricing must still require every historical fixing");
        assert!(error.to_string().contains("missing a past fixing"));
    }
}
