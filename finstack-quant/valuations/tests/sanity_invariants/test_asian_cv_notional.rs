//! Regression tests for the arithmetic-Asian control-variate notional scaling.
//!
//! The CV adjustment mixes per-path MC payoffs (notional-scaled) with the
//! analytical geometric-Asian control mean. The control must be scaled to the
//! same notional units before the adjustment; a per-unit control silently
//! collapses the adjusted price by roughly `β·P_geo·(N−1)` for notional `N`
//! .
//!
//! With a shared deterministic seed the whole estimator is linear in notional,
//! so `price(notional = N) / N` must match `price(notional = 1)` up to
//! currency rounding of the unit price.

mod cv_notional_tests {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::prelude::VolSurface;
    use finstack_quant_core::types::InstrumentId;
    use finstack_quant_valuations::instruments::exotics::asian_option::{
        AsianOption, AveragingMethod,
    };
    use finstack_quant_valuations::instruments::{InstrumentPricingOverrides, OptionType};
    use finstack_quant_valuations::pricer::{
        standard_registry, InstrumentType, ModelKey, PricerKey,
    };
    use time::Month;

    const SPOT: f64 = 100.0;
    const STRIKE: f64 = 100.0;
    const VOL: f64 = 0.20;
    const RATE: f64 = 0.05;
    const DIV_YIELD: f64 = 0.01;

    fn date(y: i32, m: Month, d: u8) -> Date {
        Date::from_calendar_date(y, m, d).expect("valid test date")
    }

    fn market(as_of: Date) -> MarketContext {
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-RATE * 5.0).exp())])
            .build()
            .expect("discount curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[70.0, 85.0, 100.0, 115.0, 130.0])
            .row(&[VOL; 5])
            .row(&[VOL; 5])
            .row(&[VOL; 5])
            .row(&[VOL; 5])
            .build()
            .expect("vol surface");
        MarketContext::new()
            .insert(discount)
            .insert_surface(surface)
            .insert_price("SPX", MarketScalar::Price(Money::new(SPOT, Currency::USD)))
            .insert_price("SPX-DIV", MarketScalar::Unitless(DIV_YIELD))
    }

    fn arithmetic_asian(option_type: OptionType, notional: f64, expiry: Date) -> AsianOption {
        let mut overrides = InstrumentPricingOverrides::default();
        overrides.model_config.mc_paths = Some(20_000);
        AsianOption {
            id: InstrumentId::new("ASIAN-CV-NOTIONAL"),
            underlying_ticker: "SPX".to_string(),
            spot_id: "SPX".into(),
            strike: STRIKE,
            option_type,
            expiry,
            averaging_method: AveragingMethod::Arithmetic,
            notional: Money::new(notional, Currency::USD),
            fixing_dates: vec![
                date(2025, Month::April, 1),
                date(2025, Month::July, 1),
                date(2025, Month::October, 1),
                date(2026, Month::January, 1),
            ],
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            vol_surface_id: "SPX-VOL".into(),
            div_yield_id: Some("SPX-DIV".into()),
            instrument_pricing_overrides: overrides,
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
            past_fixings: vec![],
        }
    }

    fn priced(option_type: OptionType, notional: f64) -> f64 {
        let as_of = date(2025, Month::January, 1);
        let expiry = date(2026, Month::January, 1);
        let market = market(as_of);
        let option = arithmetic_asian(option_type, notional, expiry);

        let registry = standard_registry();
        let pricer = registry
            .get_pricer(PricerKey::new(
                InstrumentType::AsianOption,
                ModelKey::MonteCarloGBM,
            ))
            .expect("Asian MC pricer is registered");
        pricer
            .price_dyn(&option, &market, as_of)
            .expect("Asian MC price")
            .value
            .amount()
    }

    /// `price(N) / N` must equal `price(1)` up to currency rounding of the
    /// unit price (same instrument id → same seed → identical paths; the
    /// CV-adjusted estimator is linear in notional).
    fn assert_notional_linear(option_type: OptionType) {
        let notional = 100_000.0;
        let unit_pv = priced(option_type, 1.0);
        let scaled_pv = priced(option_type, notional);

        assert!(unit_pv.is_finite() && unit_pv > 0.0, "unit pv: {unit_pv}");
        // `Money` rounds the unit price to currency scale (1e-2 for USD), so
        // allow that rounding plus float noise on the per-unit comparison.
        let per_unit = scaled_pv / notional;
        assert!(
            (per_unit - unit_pv).abs() < 0.01,
            "CV-adjusted Asian {option_type:?} price is not linear in notional: \
             unit pv = {unit_pv}, scaled pv / N = {per_unit}"
        );
    }

    #[test]
    fn arithmetic_asian_call_cv_price_scales_with_notional() {
        assert_notional_linear(OptionType::Call);
    }

    #[test]
    fn arithmetic_asian_put_cv_price_scales_with_notional() {
        assert_notional_linear(OptionType::Put);
    }
}
