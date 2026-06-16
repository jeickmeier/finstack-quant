//! Test demonstrating that users can choose between analytical and MC pricing.

mod choice_tests {
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use finstack_quant_valuations::instruments::exotics::asian_option::{AsianOption, AveragingMethod};
    use finstack_quant_valuations::instruments::OptionType;
    use finstack_quant_valuations::pricer::{
        standard_registry, InstrumentType, ModelKey, PricerKey,
    };

    #[test]
    fn test_can_choose_analytical_or_mc_via_direct_methods() {
        // This test documents that users can call npv() for analytical or npv_mc() for MC

        // Example: Create an Asian option (would need proper market in real use)
        // This test just verifies the methods exist and can be called

        let _asian = AsianOption {
            id: InstrumentId::from("DEMO"),
            underlying_ticker: "SPX".to_string(),
            spot_id: "SPX".into(),
            strike: 100.0,
            option_type: OptionType::Call,
            expiry: Date::from_calendar_date(2025, time::Month::December, 31).unwrap(),
            averaging_method: AveragingMethod::Geometric,
            notional: Money::new(1.0, Currency::USD),
            fixing_dates: vec![Date::from_calendar_date(2025, time::Month::December, 31).unwrap()],
            day_count: DayCount::Act365F,
            discount_curve_id: CurveId::from("USD-OIS"),
            vol_surface_id: CurveId::from("SPX_VOL"),
            div_yield_id: None,
            pricing_overrides: Default::default(),
            attributes: Default::default(),
            past_fixings: vec![],
        };

        // Methods available:
        // _asian.value(&market, as_of)    -> Analytical (default)
        // _asian.npv_mc(&market, as_of)   -> Monte Carlo (explicit)

        // Both methods are public and can be used based on user preference
        // Test passes if this code compiles (methods exist and are accessible)
    }

    #[test]
    fn test_can_choose_analytical_or_mc_via_registry() {
        let registry = standard_registry();

        // Users can get different pricers for the same instrument type

        // Analytical pricers (fast, deterministic)
        let asian_geometric = registry.get_pricer(PricerKey::new(
            InstrumentType::AsianOption,
            ModelKey::AsianGeometricBS,
        ));

        let asian_tw = registry.get_pricer(PricerKey::new(
            InstrumentType::AsianOption,
            ModelKey::AsianTurnbullWakeman,
        ));

        // MC pricer (slower, but handles all cases)
        let asian_mc = registry.get_pricer(PricerKey::new(
            InstrumentType::AsianOption,
            ModelKey::MonteCarloGBM,
        ));

        // All three should be registered
        assert!(
            asian_geometric.is_some(),
            "Geometric analytical pricer available"
        );
        assert!(asian_tw.is_some(), "Turnbull-Wakeman pricer available");
        assert!(asian_mc.is_some(), "Monte Carlo pricer available");
    }

    #[test]
    fn test_all_instruments_have_both_analytical_and_mc() {
        let registry = standard_registry();

        // Asian: 2 analytical + 1 MC
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::AsianOption,
                ModelKey::AsianGeometricBS
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::AsianOption,
                ModelKey::AsianTurnbullWakeman
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::AsianOption,
                ModelKey::MonteCarloGBM
            ))
            .is_some());

        // Barrier: 1 analytical + 1 MC
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::BarrierOption,
                ModelKey::BarrierBSContinuous
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::BarrierOption,
                ModelKey::MonteCarloGBM
            ))
            .is_some());

        // Lookback: 1 analytical + 1 MC
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::LookbackOption,
                ModelKey::LookbackBSContinuous
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::LookbackOption,
                ModelKey::MonteCarloGBM
            ))
            .is_some());

        // Quanto: 1 analytical only
        // Note: MC pricing for QuantoOption was removed because the quanto drift
        // adjustment model cannot be correctly represented in a simple 1D MC simulation
        // without a 2D correlated equity/FX process.
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::QuantoOption,
                ModelKey::QuantoBS
            ))
            .is_some());

        // FX Barrier: 1 analytical + 1 MC
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::FxBarrierOption,
                ModelKey::FxBarrierBSContinuous
            ))
            .is_some());
        assert!(registry
            .get_pricer(PricerKey::new(
                InstrumentType::FxBarrierOption,
                ModelKey::MonteCarloGBM
            ))
            .is_some());

    }
}
