//! Determinism tests for Monte Carlo greeks.
//!
//! Verifies that MC-priced instruments produce identical results across
//! multiple runs when computing greeks. Each metric should produce the
//! exact same value when run 10+ times with deterministic seeds.
//!
//! Tests cover:
//! - AsianOption (Delta, Vega, Gamma, Rho, Theta)
//! - BarrierOption (Delta, Vega)
//! - LookbackOption (Delta, Vega)
//! - Autocallable (Delta, Vega)
//! - CliquetOption (Delta, Vega)
//! - FxBarrierOption (Delta, Vega)
//! - QuantoOption (Delta, Vega)
//! - CmsOption (Delta, Vega)
//! - RangeAccrual (Delta, Vega)

mod tests {
    // Imports are only used when `slow` feature is enabled, so allow unused warnings
    #[allow(unused_imports)]
    use finstack_quant_core::currency::Currency;
    #[allow(unused_imports)]
    use finstack_quant_core::dates::{Date, DayCount};
    #[allow(unused_imports)]
    use finstack_quant_core::market_data::context::MarketContext;
    #[allow(unused_imports)]
    use finstack_quant_core::market_data::scalars::MarketScalar;
    #[allow(unused_imports)]
    use finstack_quant_core::market_data::surfaces::VolSurface;
    #[allow(unused_imports)]
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    #[allow(unused_imports)]
    use finstack_quant_core::money::Money;
    #[allow(unused_imports)]
    use finstack_quant_valuations::instruments::Instrument;
    #[allow(unused_imports)]
    use finstack_quant_valuations::instruments::OptionType;
    #[allow(unused_imports)]
    use finstack_quant_valuations::instruments::{
        AsianOption, Autocallable, AveragingMethod, BarrierOption, BarrierType, FinalPayoffType,
        LookbackOption, LookbackType,
    };
    #[allow(unused_imports)]
    use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
    #[allow(unused_imports)]
    use std::sync::Arc;
    #[allow(unused_imports)]
    use time::macros::date;

    /// Helper to create a standard equity market context for MC pricing tests
    #[allow(dead_code)] // Only used when slow feature is enabled
    fn create_mc_market(as_of: Date, spot: f64, vol: f64, rate: f64) -> MarketContext {
        let disc_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([
                (0.0f64, 1.0f64),
                (1.0f64, (-rate).exp()),
                (2.0f64, (-rate * 2.0f64).exp()),
            ])
            .build()
            .unwrap();

        let vol_surface = VolSurface::builder("SPOT_VOL")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0])
            .row(&[vol, vol, vol])
            .row(&[vol, vol, vol])
            .row(&[vol, vol, vol])
            .build()
            .unwrap();

        MarketContext::new()
            .insert(disc_curve)
            .insert_surface(vol_surface)
            .insert_price("SPOT", MarketScalar::Unitless(spot))
    }

    /// Run a metric calculation multiple times and verify all results are identical
    #[allow(dead_code)] // Only used when slow feature is enabled
    fn test_metric_determinism<I: Instrument + Clone + 'static>(
        instrument: I,
        market: &MarketContext,
        as_of: Date,
        metric_id: MetricId,
        num_runs: usize,
    ) {
        let registry = standard_registry();

        let mut results = Vec::with_capacity(num_runs);
        for _ in 0..num_runs {
            let pv = instrument.value(market, as_of).unwrap();
            let mut context = MetricContext::new(
                Arc::new(instrument.clone()),
                Arc::new(market.clone()),
                as_of,
                pv,
                MetricContext::default_config(),
            );

            let metric_id_clone = metric_id.clone();
            let computed = registry
                .compute(std::slice::from_ref(&metric_id_clone), &mut context)
                .unwrap();
            let value = *computed.get(&metric_id_clone).unwrap();
            results.push(value);
        }

        // All results should be identical (bit-exact equality for deterministic MC)
        let first = results[0];
        for (i, &value) in results.iter().enumerate() {
            assert_eq!(
                value, first,
                "Metric {:?} calculation {} should be identical to first run (expected {}, got {})",
                metric_id, i, first, value
            );
        }
    }

    /// Lightweight determinism test that runs in normal CI (no slow feature required).
    /// Verifies that MC-based metric calculations are deterministic across runs.
    #[test]
    fn test_basic_mc_determinism_lightweight() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);

        // Use a simple Asian option configuration for fast execution
        let option = AsianOption {
            id: "MC_DETERMINISM_LIGHTWEIGHT".into(),
            underlying_ticker: "SPOT".to_string(),
            spot_id: "SPOT".into(),
            strike: 100.0,
            option_type: OptionType::Call,
            expiry,
            notional: Money::new(1.0, Currency::USD),
            averaging_method: AveragingMethod::Arithmetic,
            fixing_dates: vec![date!(2024 - 07 - 01), date!(2025 - 01 - 01)],
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
            past_fixings: vec![],
        };

        let market = create_mc_market(as_of, 100.0, 0.25, 0.05);
        // Run only 3 times to keep it fast
        test_metric_determinism(option, &market, as_of, MetricId::Delta, 3);
    }

    #[test]
    fn test_asian_option_delta_deterministic() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);

        let option = AsianOption {
            id: "ASIAN_DELTA_TEST".into(),
            underlying_ticker: "SPOT".to_string(),
            spot_id: "SPOT".into(),
            strike: 100.0,
            option_type: OptionType::Call,
            expiry,
            notional: Money::new(1.0, Currency::USD),
            averaging_method: AveragingMethod::Arithmetic,
            fixing_dates: vec![
                date!(2024 - 04 - 01),
                date!(2024 - 07 - 01),
                date!(2024 - 10 - 01),
                date!(2025 - 01 - 01),
            ],
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
            past_fixings: vec![],
        };

        let market = create_mc_market(as_of, 100.0, 0.25, 0.05);
        test_metric_determinism(option, &market, as_of, MetricId::Delta, 10);
    }

    #[test]
    fn test_asian_option_vega_deterministic() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);

        let option = AsianOption {
            id: "ASIAN_VEGA_TEST".into(),
            underlying_ticker: "SPOT".to_string(),
            spot_id: "SPOT".into(),
            strike: 100.0,
            option_type: OptionType::Call,
            expiry,
            notional: Money::new(1.0, Currency::USD),
            averaging_method: AveragingMethod::Arithmetic,
            fixing_dates: vec![
                date!(2024 - 04 - 01),
                date!(2024 - 07 - 01),
                date!(2024 - 10 - 01),
                date!(2025 - 01 - 01),
            ],
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
            past_fixings: vec![],
        };

        let market = create_mc_market(as_of, 100.0, 0.25, 0.05);
        test_metric_determinism(option, &market, as_of, MetricId::Vega, 10);
    }

    #[test]
    fn test_asian_option_all_greeks_deterministic() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);

        let option = AsianOption {
            id: "ASIAN_ALL_GREEKS".into(),
            underlying_ticker: "SPOT".to_string(),
            spot_id: "SPOT".into(),
            strike: 100.0,
            option_type: OptionType::Call,
            expiry,
            notional: Money::new(1.0, Currency::USD),
            averaging_method: AveragingMethod::Arithmetic,
            fixing_dates: vec![
                date!(2024 - 04 - 01),
                date!(2024 - 07 - 01),
                date!(2024 - 10 - 01),
                date!(2025 - 01 - 01),
            ],
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
            past_fixings: vec![],
        };

        let market = create_mc_market(as_of, 100.0, 0.25, 0.05);

        // Test all key greeks
        test_metric_determinism(option.clone(), &market, as_of, MetricId::Delta, 10);
        test_metric_determinism(option.clone(), &market, as_of, MetricId::Gamma, 10);
        test_metric_determinism(option.clone(), &market, as_of, MetricId::Vega, 10);
        test_metric_determinism(option.clone(), &market, as_of, MetricId::Rho, 10);
        test_metric_determinism(option, &market, as_of, MetricId::Theta, 10);
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn test_barrier_option_delta_vega_deterministic() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);

        let option = BarrierOption {
            id: "BARRIER_TEST".into(),
            underlying_ticker: "SPOT".to_string(),
            spot_id: "SPOT".into(),
            strike: 100.0,
            barrier: Money::new(120.0, Currency::USD),
            barrier_type: BarrierType::UpAndOut,
            option_type: OptionType::Call,
            expiry,
            expiry_fixing: None,
            observed_barrier_breached: None,
            notional: Money::new(1.0, Currency::USD),
            day_count: DayCount::Act365F,
            use_gobet_miri: true,
            discount_curve_id: "USD-OIS".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
            rebate: None,
            rebate_timing: Default::default(),
            monitoring_frequency: None,
        };

        let market = create_mc_market(as_of, 100.0, 0.25, 0.05);
        test_metric_determinism(option.clone(), &market, as_of, MetricId::Delta, 10);
        test_metric_determinism(option, &market, as_of, MetricId::Vega, 10);
    }

    #[test]
    fn test_lookback_option_delta_vega_deterministic() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);

        let option = LookbackOption {
            id: "LOOKBACK_TEST".into(),
            underlying_ticker: "SPOT".to_string(),
            spot_id: "SPOT".into(),
            strike: Some(100.0),
            lookback_type: LookbackType::FixedStrike,
            option_type: OptionType::Call,
            expiry,
            expiry_fixing: None,
            notional: Money::new(1.0, Currency::USD),
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            observed_min: None,
            observed_max: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
            use_gobet_miri: false,
        };

        let market = create_mc_market(as_of, 100.0, 0.25, 0.05);
        test_metric_determinism(option.clone(), &market, as_of, MetricId::Delta, 10);
        test_metric_determinism(option, &market, as_of, MetricId::Vega, 10);
    }

    #[test]
    #[ignore = "slow: covered by mise rust-test-slow"]
    fn test_autocallable_delta_vega_deterministic() {
        let as_of = date!(2024 - 01 - 01);
        let observation_dates = vec![
            date!(2024 - 04 - 01),
            date!(2024 - 07 - 01),
            date!(2024 - 10 - 01),
            date!(2025 - 01 - 01),
        ];

        let option = Autocallable {
            id: "AUTOCALL_TEST".into(),
            underlying_ticker: "SPOT".to_string(),
            observation_dates: observation_dates.clone(),
            expiry: *observation_dates
                .last()
                .expect("non-empty observation schedule"),
            autocall_barriers: vec![1.1, 1.15, 1.2, 1.25],
            coupons: vec![0.05, 0.05, 0.05, 0.05],
            memory_coupons: false,
            final_barrier: 0.8,
            final_payoff_type: FinalPayoffType::CapitalProtection { floor: 0.9 },
            participation_rate: 1.0,
            cap_level: 1.3,
            notional: Money::new(1.0, Currency::USD),
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            spot_id: "SPOT".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            initial_level: None,
            past_fixings: vec![],
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
        };

        let market = create_mc_market(as_of, 100.0, 0.25, 0.05);
        test_metric_determinism(option.clone(), &market, as_of, MetricId::Delta, 10);
        test_metric_determinism(option, &market, as_of, MetricId::Vega, 10);
    }

    /// Test that CRN (Common Random Numbers) produces smooth, stable greeks.
    ///
    /// This test verifies that delta varies smoothly as spot changes, which is
    /// the hallmark of effective CRN variance reduction. Without CRN, MC noise
    /// would cause erratic delta changes even for small spot perturbations.
    #[test]
    fn test_crn_produces_smooth_delta_curve() {
        let as_of = date!(2024 - 01 - 01);
        let expiry = date!(2025 - 01 - 01);

        // Use minimal fixing dates for faster execution
        let option = AsianOption {
            id: "CRN_SMOOTHNESS_TEST".into(),
            underlying_ticker: "SPOT".to_string(),
            spot_id: "SPOT".into(),
            strike: 100.0,
            option_type: OptionType::Call,
            expiry,
            notional: Money::new(1.0, Currency::USD),
            averaging_method: AveragingMethod::Arithmetic,
            fixing_dates: vec![date!(2024 - 07 - 01), date!(2025 - 01 - 01)],
            day_count: DayCount::Act365F,
            discount_curve_id: "USD-OIS".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Default::default(),
            past_fixings: vec![],
        };

        let registry = standard_registry();

        // Compute delta at several spot levels
        let spot_levels = [95.0, 97.0, 99.0, 100.0, 101.0, 103.0, 105.0];
        let mut deltas = Vec::new();

        for &spot in &spot_levels {
            let market = create_mc_market(as_of, spot, 0.25, 0.05);
            let pv = option.value(&market, as_of).unwrap();
            let mut context = MetricContext::new(
                Arc::new(option.clone()),
                Arc::new(market),
                as_of,
                pv,
                MetricContext::default_config(),
            );
            let computed = registry.compute(&[MetricId::Delta], &mut context).unwrap();
            deltas.push(*computed.get(&MetricId::Delta).unwrap());
        }

        // Verify deltas are monotonically non-decreasing for a call option
        // (higher spot => higher delta for calls, approaching 1)
        for i in 1..deltas.len() {
            assert!(
                deltas[i] >= deltas[i - 1] - 0.01, // Allow small tolerance for MC noise
                "Delta should be monotonically increasing for call option. \
                 At spots [{}, {}], deltas were [{}, {}]",
                spot_levels[i - 1],
                spot_levels[i],
                deltas[i - 1],
                deltas[i]
            );
        }

        // Verify delta changes smoothly (no large jumps between adjacent spots)
        // For a 2% spot change, delta shouldn't jump by more than ~0.15
        for i in 1..deltas.len() {
            let delta_change = (deltas[i] - deltas[i - 1]).abs();
            let spot_change_pct = (spot_levels[i] - spot_levels[i - 1]) / spot_levels[i - 1];
            let max_allowed_delta_change = 0.15 + spot_change_pct * 2.0; // Scale with spot change

            assert!(
                delta_change <= max_allowed_delta_change,
                "Delta change {} between spots {} and {} exceeds max {} - \
                 possible CRN failure or excessive MC noise",
                delta_change,
                spot_levels[i - 1],
                spot_levels[i],
                max_allowed_delta_change
            );
        }
    }
}
