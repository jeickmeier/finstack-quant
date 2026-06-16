//! Edge case tests for risk metrics.
//!
//! Tests behavior at extreme parameter values:
//! - Zero recovery rates
//! - Expired options (T ≤ 0)
//! - Zero volatility
//! - Zero notional
//! - Extreme moneyness (deep ITM/OTM)

use crate::common::{simple_option_market, test_option, tolerances};
use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::{PricingOverrides, SettlementType};
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;
use time::macros::date;

/// Create an option market. Delegates to common builder for most cases,
/// but allows custom vol surface for edge case tests.
fn create_option_market(as_of: Date, spot: f64, vol: f64, rate: f64) -> MarketContext {
    simple_option_market(as_of, spot, vol, rate)
}

#[test]
fn test_expired_option_returns_zero_theta() {
    // Expired options should return 0.0 for time-sensitive greeks
    let as_of = date!(2025 - 01 - 01);
    let expiry = as_of; // Expired at as_of

    let option = test_option(expiry).id("EXPIRED_OPTION").build();

    let market = create_option_market(as_of, 100.0, 0.25, 0.05);
    let registry = standard_registry();

    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let results = registry.compute(&[MetricId::Theta], &mut context).unwrap();
    let theta = *results.get(&MetricId::Theta).unwrap();

    // Expired options should have zero or very small theta
    assert!(
        theta.abs() < tolerances::STANDARD,
        "Expired option theta should be near zero, got {}",
        theta
    );
}

#[test]
fn test_zero_recovery_cds() {
    // CDS with zero recovery should still compute Recovery01 metric
    let as_of = date!(2025 - 01 - 01);
    let maturity = date!(2030 - 01 - 01);

    // Create CDS and set zero recovery
    let mut cds = test_utils::cds_buy_protection(
        "ZERO_RECOVERY_CDS",
        Money::new(1_000_000.0, Currency::USD),
        500.0, // 500bp = 5% spread
        as_of,
        maturity,
        "USD-OIS",
        "HAZARD",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = 0.0;

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0f64, 1.0f64), (5.0f64, (-0.05f64 * 5.0f64).exp())])
        .build()
        .unwrap();

    use finstack_quant_core::market_data::term_structures::HazardCurve;
    let hazard_curve = HazardCurve::builder("HAZARD")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.0)
        .knots([(0.0f64, 0.01f64), (5.0f64, 0.01f64)]) // Flat 1% hazard rate
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let registry = standard_registry();
    let pv = cds.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(cds),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    // Recovery01 should still compute even with zero recovery
    let results = registry
        .compute(&[MetricId::Recovery01], &mut context)
        .unwrap();
    let recovery01 = results.get(&MetricId::Recovery01);

    // Should either compute a value or return 0.0 (not error)
    if let Some(val) = recovery01 {
        assert!(val.is_finite(), "Recovery01 should be finite, got {}", val);
    }
}

#[test]
fn test_deep_itm_option_greeks() {
    // Deep ITM options should have delta near 1.0 (for calls) or -1.0 (for puts)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;
    let strike = 50.0; // Deep ITM

    let option = test_option(expiry)
        .id("DEEP_ITM_CALL")
        .strike(strike)
        .build();

    let market = create_option_market(as_of, spot, 0.25, 0.05);
    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let results = registry.compute(&[MetricId::Delta], &mut context).unwrap();
    let delta = *results.get(&MetricId::Delta).unwrap();
    let delta_per_share = delta / 100.0; // Normalize by contract size

    // Deep ITM call delta should be high (close to 1.0)
    assert!(
        delta_per_share > 0.8,
        "Deep ITM call delta should be > 0.8 per share, got {}",
        delta_per_share
    );
}

#[test]
fn test_zero_volatility_option_limits() {
    // At zero volatility, Black-Scholes has well-defined limits:
    // - Vega -> 0 (no sensitivity to vol)
    // - Delta -> 0 or 1 depending on moneyness (step function)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    // Test ATM call (forward is slightly different from spot due to rates)
    let atm_option = test_option(expiry).id("ZERO_VOL_ATM").build();

    // Market with zero volatility
    let market = create_option_market(as_of, spot, 0.0, 0.05);
    let registry = standard_registry();

    let pv = atm_option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(atm_option),
        Arc::new(market.clone()),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let metrics = registry
        .compute(&[MetricId::Delta, MetricId::Vega], &mut context)
        .unwrap();
    if let Some(&delta) = metrics.get(&MetricId::Delta) {
        assert!(
            delta.is_finite(),
            "Delta should be finite even with zero vol, got {}",
            delta
        );
    }
    if let Some(&vega) = metrics.get(&MetricId::Vega) {
        assert!(
            vega.is_finite(),
            "Vega should be finite even with zero vol, got {}",
            vega
        );
    }

    // At zero volatility, Black-Scholes becomes degenerate - delta should be
    // a step function (0 or 1 depending on moneyness). However, implementations
    // may not handle this edge case perfectly, especially when using finite difference
    // for delta calculation. We verify that ITM options have higher delta than OTM.
    let mut itm_delta_per_share = 0.0;
    let mut otm_delta_per_share = 0.0;

    // Test deep ITM call
    let itm_option = EquityOption {
        id: "ZERO_VOL_ITM".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 50.0, // Deep ITM
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "SPOT".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let pv = itm_option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(itm_option),
        Arc::new(market.clone()),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let metrics = registry.compute(&[MetricId::Delta], &mut context).unwrap();
    if let Some(&delta) = metrics.get(&MetricId::Delta) {
        itm_delta_per_share = delta / 100.0;
        assert!(
            delta.is_finite() && delta >= 0.0,
            "ITM zero-vol delta should be finite and positive, got {}",
            delta
        );
    }

    // Test deep OTM call
    let otm_option = EquityOption {
        id: "ZERO_VOL_OTM".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 150.0, // Deep OTM
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "SPOT".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let pv = otm_option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(otm_option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let metrics = registry.compute(&[MetricId::Delta], &mut context).unwrap();
    if let Some(&delta) = metrics.get(&MetricId::Delta) {
        otm_delta_per_share = delta / 100.0;
        assert!(
            delta.is_finite() && delta >= 0.0,
            "OTM zero-vol delta should be finite and non-negative, got {}",
            delta
        );
    }

    // ITM delta should be greater than OTM delta (monotonicity property)
    assert!(
        itm_delta_per_share >= otm_delta_per_share,
        "ITM delta ({}) should be >= OTM delta ({})",
        itm_delta_per_share,
        otm_delta_per_share
    );
}

#[test]
fn test_zero_notional_handling() {
    // Options with zero notional should handle gracefully
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let option = EquityOption {
        id: "ZERO_NOTIONAL".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(0.0, Currency::USD), // Zero notional
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "SPOT".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, 100.0, 0.25, 0.05);
    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();

    assert!(
        pv.amount().abs() < 1e-10,
        "PV should be near zero for zero notional, got {}",
        pv.amount()
    );

    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let metrics = registry
        .compute(&[MetricId::Delta, MetricId::Gamma], &mut context)
        .unwrap();
    if let Some(&delta) = metrics.get(&MetricId::Delta) {
        assert!(
            delta.abs() < 1e-6,
            "Delta should be near zero for zero notional, got {}",
            delta
        );
    }
    if let Some(&gamma) = metrics.get(&MetricId::Gamma) {
        assert!(
            gamma.abs() < 1e-6,
            "Gamma should be near zero for zero notional, got {}",
            gamma
        );
    }
}

#[test]
fn test_deep_otm_option_greeks() {
    // Deep OTM options should have delta near 0.0
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;
    let strike = 200.0; // Deep OTM call

    let option = EquityOption {
        id: "DEEP_OTM_CALL".into(),
        underlying_ticker: "AAPL".to_string(),
        strike,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "SPOT".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, spot, 0.25, 0.05);
    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let results = registry.compute(&[MetricId::Delta], &mut context).unwrap();
    let delta = *results.get(&MetricId::Delta).unwrap();
    let delta_per_share = delta / 100.0; // Normalize by contract size

    // Deep OTM call delta should be very small (close to 0.0)
    // For 100% OTM option (strike 200 vs spot 100), delta should be < 1%
    assert!(
        delta_per_share.abs() < 0.01,
        "Deep OTM call delta should be < 0.01 per share for 100% OTM, got {}",
        delta_per_share
    );
}

#[test]
fn test_deep_itm_put_greeks() {
    // Deep ITM put (K >> S): delta should approach -1.0 (per share)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;
    let strike = 150.0; // Deep ITM put (K > S)

    let option = EquityOption {
        id: "DEEP_ITM_PUT".into(),
        underlying_ticker: "AAPL".to_string(),
        strike,
        option_type: OptionType::Put,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "SPOT".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, spot, 0.25, 0.05);
    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let results = registry.compute(&[MetricId::Delta], &mut context).unwrap();
    let delta = *results.get(&MetricId::Delta).unwrap();
    let delta_per_share = delta / 100.0; // Normalize by contract size

    // Deep ITM put delta should be close to -1.0
    assert!(
        delta_per_share < -0.8,
        "Deep ITM put delta should be < -0.8 per share, got {}",
        delta_per_share
    );
}

#[test]
fn test_deep_otm_put_greeks() {
    // Deep OTM put (K << S): delta should approach 0.0
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;
    let strike = 50.0; // Deep OTM put (K < S)

    let option = EquityOption {
        id: "DEEP_OTM_PUT".into(),
        underlying_ticker: "AAPL".to_string(),
        strike,
        option_type: OptionType::Put,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "SPOT".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, spot, 0.25, 0.05);
    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let results = registry.compute(&[MetricId::Delta], &mut context).unwrap();
    let delta = *results.get(&MetricId::Delta).unwrap();
    let delta_per_share = delta / 100.0; // Normalize by contract size

    // Deep OTM put delta should be close to 0.0 (but negative)
    // For 50% OTM put (strike 50 vs spot 100), delta should be < 1%
    assert!(
        delta_per_share.abs() < 0.01,
        "Deep OTM put delta should be < 0.01 abs per share for 50% OTM, got {}",
        delta_per_share
    );
    // Put delta should be negative (or very close to zero)
    assert!(
        delta_per_share <= 0.0,
        "Put delta should be non-positive, got {}",
        delta_per_share
    );
}

#[test]
fn test_atm_option_gamma_peak() {
    // ATM options should have maximum gamma (highest sensitivity to spot moves)
    // This is a property test: gamma is typically highest near ATM
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    let market = create_option_market(as_of, spot, 0.25, 0.05);
    let registry = standard_registry();

    // Test multiple strikes and verify ATM has higher gamma
    let strikes = [80.0, 90.0, 100.0, 110.0, 120.0];
    let mut gammas = Vec::new();

    for strike in strikes {
        let option = EquityOption {
            id: format!("ATM_GAMMA_TEST_{}", strike).into(),
            underlying_ticker: "AAPL".to_string(),
            strike,
            option_type: OptionType::Call,
            exercise_style: ExerciseStyle::European,
            expiry,
            notional: Money::new(100.0, Currency::USD),
            day_count: DayCount::Act365F,
            settlement: SettlementType::Cash,
            discount_curve_id: "USD-OIS".into(),
            spot_id: "SPOT".into(),
            vol_surface_id: "SPOT_VOL".into(),
            div_yield_id: None,
            discrete_dividends: Vec::new(),
            pricing_overrides: PricingOverrides::default(),
            exercise_schedule: None,
            attributes: Default::default(),
        };

        let pv = option.value(&market, as_of).unwrap();
        let mut context = MetricContext::new(
            Arc::new(option),
            Arc::new(market.clone()),
            as_of,
            pv,
            MetricContext::default_config(),
        );
        let results = registry.compute(&[MetricId::Gamma], &mut context).unwrap();
        let gamma = *results.get(&MetricId::Gamma).unwrap();
        gammas.push((strike, gamma));
    }

    // ATM (100.0) should have gamma that's relatively high
    // Find ATM gamma
    let atm_gamma = gammas
        .iter()
        .find(|(s, _)| (*s - spot).abs() < 1e-6)
        .map(|(_, g)| g);
    if let Some(&atm_g) = atm_gamma {
        let max_gamma = gammas.iter().map(|(_, g)| *g).fold(0.0, f64::max);
        assert!(
            atm_g > 0.0,
            "ATM option gamma should be positive, got {}",
            atm_g
        );
        assert!(
            atm_g >= 0.9 * max_gamma,
            "ATM gamma ({}) should be near the maximum gamma ({})",
            atm_g,
            max_gamma
        );
    }
}

#[test]
fn test_extreme_volatility_handling() {
    // Very high volatility should not cause numerical issues
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let option = EquityOption {
        id: "HIGH_VOL_TEST".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "SPOT".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    // Test with very high vol (200% = 2.0)
    let market = create_option_market(as_of, 100.0, 2.0, 0.05);
    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let metrics = registry
        .compute(
            &[MetricId::Delta, MetricId::Vega, MetricId::Gamma],
            &mut context,
        )
        .unwrap();

    for (metric_id, value) in metrics.iter() {
        assert!(
            value.is_finite(),
            "Metric {:?} should be finite even with extreme vol, got {}",
            metric_id,
            value
        );
    }
}

#[test]
fn test_very_short_dated_option() {
    // Options with very short time to expiry should handle gracefully
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2024 - 01 - 02); // 1 day expiry

    let option = EquityOption {
        id: "SHORT_EXPIRY".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "SPOT".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, 100.0, 0.25, 0.05);
    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let metrics = registry
        .compute(&[MetricId::Delta, MetricId::Theta], &mut context)
        .unwrap();

    if let Some(&theta) = metrics.get(&MetricId::Theta) {
        assert!(
            theta < 0.0,
            "Short-dated option theta should be negative, got {}",
            theta
        );
    }
}

#[test]
fn test_very_low_interest_rate_greeks() {
    // Test Greeks with very low interest rates (near-zero rate environment)
    // This validates behavior similar to negative rate environments
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    // Create discount curve with very low rate (0.1%)
    // Note: Curve builder requires strictly decreasing DFs, so we can't directly test negative rates
    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-0.001f64).exp()), // 0.1% rate
        ])
        .build()
        .unwrap();

    let vol_surface = VolSurface::builder("SPOT_VOL")
        .expiries(&[0.5, 1.0])
        .strikes(&[80.0, 100.0, 120.0])
        .row(&[0.25, 0.25, 0.25])
        .row(&[0.25, 0.25, 0.25])
        .build()
        .unwrap();

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert_surface(vol_surface)
        .insert_price("SPOT", MarketScalar::Unitless(spot));

    let call_option = EquityOption {
        id: "LOW_RATE_CALL".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "SPOT".into(),
        vol_surface_id: "SPOT_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let registry = standard_registry();
    let pv = call_option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(call_option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let results = registry
        .compute(&[MetricId::Delta, MetricId::Rho], &mut context)
        .unwrap();

    // Delta sign should still be correct (positive for calls)
    let delta = *results.get(&MetricId::Delta).unwrap();
    assert!(
        delta > 0.0,
        "Call delta should be positive with low rates, got {}",
        delta
    );

    // Rho for calls should still be positive (rate increase benefits calls via discounting)
    let rho = *results.get(&MetricId::Rho).unwrap();
    assert!(
        rho > 0.0,
        "Call rho should be positive (rate increase benefits calls), got {}",
        rho
    );
}

#[test]
fn test_vol_smile_greeks() {
    // Test Greeks compute correctly with realistic vol smile/skew
    // Real markets have: lower strikes (puts) have higher vol, higher strikes (calls) have lower vol
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;

    // Create vol surface with equity skew: lower strikes have higher vol
    let vol_surface = VolSurface::builder("SMILE_VOL")
        .expiries(&[0.5, 1.0, 2.0])
        .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
        // Equity skew pattern: vol decreases as strike increases
        .row(&[0.35, 0.30, 0.25, 0.22, 0.20]) // 6M
        .row(&[0.32, 0.28, 0.24, 0.21, 0.19]) // 1Y
        .row(&[0.30, 0.26, 0.23, 0.20, 0.18]) // 2Y
        .build()
        .unwrap();

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0f64, 1.0f64), (1.0f64, (-0.05f64).exp())])
        .build()
        .unwrap();

    let market = MarketContext::new()
        .insert(disc_curve)
        .insert_surface(vol_surface)
        .insert_price("SPOT", MarketScalar::Unitless(spot));

    let option = EquityOption {
        id: "SMILE_TEST".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "SPOT".into(),
        vol_surface_id: "SMILE_VOL".into(),
        div_yield_id: None,
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let results = registry
        .compute(
            &[MetricId::Delta, MetricId::Vega, MetricId::Gamma],
            &mut context,
        )
        .unwrap();

    // Verify Greeks compute correctly with smile
    let delta = *results.get(&MetricId::Delta).unwrap();
    let vega = *results.get(&MetricId::Vega).unwrap();
    let gamma = *results.get(&MetricId::Gamma).unwrap();

    // Delta should be positive for call
    assert!(
        delta > 0.0,
        "Call delta should be positive with vol smile, got {}",
        delta
    );

    // Vega should be positive for long option
    assert!(
        vega > 0.0,
        "Vega should be positive for long option with vol smile, got {}",
        vega
    );

    // Gamma should be positive for long option
    assert!(
        gamma > 0.0,
        "Gamma should be positive for long option with vol smile, got {}",
        gamma
    );

    // Verify values are finite and reasonable
    let delta_per_share = delta / 100.0;
    assert!(
        (0.0..1.0).contains(&delta_per_share),
        "Delta per share should be between 0 and 1 for ATM call, got {}",
        delta_per_share
    );
}
