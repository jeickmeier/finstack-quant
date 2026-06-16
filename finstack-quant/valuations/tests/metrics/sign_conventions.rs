//! Tests verifying documented sign conventions for all greeks across instruments.
//!
//! Validates that metric signs match market conventions as documented in METRICS.md:
//! - Long Call Delta: Positive
//! - Long Put Delta: Negative
//! - Long Vega: Positive (both calls and puts)
//! - Long Gamma: Positive (both calls and puts, long position)
//! - Long Theta: Negative (time decay)
//! - Long Rho: Positive for calls, negative for puts (rate increase benefits calls, hurts puts)
//! - Bond DV01: Negative (price falls as rates rise)
//! - CS01 (credit spread sensitivity):
//!   - Protection buyer: Positive (spread widening benefits protection leg)
//!   - Protection seller: Negative

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::equity::equity_option::EquityOption;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{ExerciseStyle, OptionType};
use finstack_quant_valuations::instruments::{PricingOverrides, SettlementType};
use finstack_quant_valuations::metrics::{standard_registry, MetricContext, MetricId};
use std::sync::Arc;
use time::macros::date;

fn create_option_market(
    as_of: Date,
    spot: f64,
    vol: f64,
    rate: f64,
    div_yield: f64,
) -> MarketContext {
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

    let vol_surface = VolSurface::builder("AAPL_VOL")
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
        .insert_price("AAPL", MarketScalar::Price(Money::new(spot, Currency::USD)))
        .insert_price("AAPL_DIV", MarketScalar::Unitless(div_yield))
}

#[test]
fn test_call_delta_positive() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let option = EquityOption {
        id: "CALL_TEST".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: Some("AAPL_DIV".into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, 100.0, 0.25, 0.05, 0.02);
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

    assert!(
        delta > 0.0,
        "Long call delta should be positive, got {}",
        delta
    );
}

#[test]
fn test_put_delta_negative() {
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let option = EquityOption {
        id: "PUT_TEST".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Put,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: Some("AAPL_DIV".into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, 100.0, 0.25, 0.05, 0.02);
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

    assert!(
        delta < 0.0,
        "Long put delta should be negative, got {}",
        delta
    );
}

#[test]
fn test_theta_negative_for_long_positions() {
    // Theta (time decay) should be negative for long option positions
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    for option_type in [OptionType::Call, OptionType::Put] {
        let option = EquityOption {
            id: "THETA_TEST".into(),
            underlying_ticker: "AAPL".to_string(),
            strike: 100.0,
            option_type,
            exercise_style: ExerciseStyle::European,
            expiry,
            notional: Money::new(100.0, Currency::USD),
            day_count: DayCount::Act365F,
            settlement: SettlementType::Cash,
            discount_curve_id: "USD-OIS".into(),
            spot_id: "AAPL".into(),
            vol_surface_id: "AAPL_VOL".into(),
            div_yield_id: Some("AAPL_DIV".into()),
            discrete_dividends: Vec::new(),
            pricing_overrides: PricingOverrides::default(),
            exercise_schedule: None,
            attributes: Default::default(),
        };

        let market = create_option_market(as_of, 100.0, 0.25, 0.05, 0.02);
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

        assert!(
            theta < 0.0,
            "Theta should be strictly negative for long {} option with 1Y expiry, got {}",
            if matches!(option_type, OptionType::Call) {
                "call"
            } else {
                "put"
            },
            theta
        );
    }
}

#[test]
fn test_bond_dv01_negative() {
    // Bond DV01 should be negative under the convention: DV01 = PV(bumped +1bp) - PV(base)
    // When rates rise by 1bp, bond prices fall, so dv01 < 0 for a long position
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "DV01_TEST",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([(0.0f64, 1.0f64), (5.0f64, (-0.05f64 * 5.0f64).exp())])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc_curve);
    let registry = standard_registry();
    let pv = bond.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let results = registry.compute(&[MetricId::Dv01], &mut context).unwrap();
    let dv01 = *results.get(&MetricId::Dv01).unwrap();

    assert!(dv01 < 0.0, "Bond DV01 should be negative, got {}", dv01);
}

#[test]
fn test_vega_always_positive() {
    // Vega should be positive for long option positions (both calls and puts)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    for option_type in [OptionType::Call, OptionType::Put] {
        let option = EquityOption {
            id: "VEGA_TEST".into(),
            underlying_ticker: "AAPL".to_string(),
            strike: 100.0,
            option_type,
            exercise_style: ExerciseStyle::European,
            expiry,
            notional: Money::new(100.0, Currency::USD),
            day_count: DayCount::Act365F,
            settlement: SettlementType::Cash,
            discount_curve_id: "USD-OIS".into(),
            spot_id: "AAPL".into(),
            vol_surface_id: "AAPL_VOL".into(),
            div_yield_id: Some("AAPL_DIV".into()),
            discrete_dividends: Vec::new(),
            pricing_overrides: PricingOverrides::default(),
            exercise_schedule: None,
            attributes: Default::default(),
        };

        let market = create_option_market(as_of, 100.0, 0.25, 0.05, 0.02);
        let registry = standard_registry();
        let pv = option.value(&market, as_of).unwrap();
        let mut context = MetricContext::new(
            Arc::new(option),
            Arc::new(market),
            as_of,
            pv,
            MetricContext::default_config(),
        );

        let results = registry.compute(&[MetricId::Vega], &mut context).unwrap();
        let vega = *results.get(&MetricId::Vega).unwrap();

        assert!(
            vega > 0.0,
            "Vega should be positive for long {} option, got {}",
            if matches!(option_type, OptionType::Call) {
                "call"
            } else {
                "put"
            },
            vega
        );
    }
}

#[test]
fn test_gamma_always_positive() {
    // Gamma should be positive for long option positions (both calls and puts)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    for option_type in [OptionType::Call, OptionType::Put] {
        let option = EquityOption {
            id: "GAMMA_TEST".into(),
            underlying_ticker: "AAPL".to_string(),
            strike: 100.0,
            option_type,
            exercise_style: ExerciseStyle::European,
            expiry,
            notional: Money::new(100.0, Currency::USD),
            day_count: DayCount::Act365F,
            settlement: SettlementType::Cash,
            discount_curve_id: "USD-OIS".into(),
            spot_id: "AAPL".into(),
            vol_surface_id: "AAPL_VOL".into(),
            div_yield_id: Some("AAPL_DIV".into()),
            discrete_dividends: Vec::new(),
            pricing_overrides: PricingOverrides::default(),
            exercise_schedule: None,
            attributes: Default::default(),
        };

        let market = create_option_market(as_of, 100.0, 0.25, 0.05, 0.02);
        let registry = standard_registry();
        let pv = option.value(&market, as_of).unwrap();
        let mut context = MetricContext::new(
            Arc::new(option),
            Arc::new(market),
            as_of,
            pv,
            MetricContext::default_config(),
        );

        let results = registry.compute(&[MetricId::Gamma], &mut context).unwrap();
        let gamma = *results.get(&MetricId::Gamma).unwrap();

        assert!(
            gamma >= 0.0,
            "Gamma should be non-negative for long {} option, got {}",
            if matches!(option_type, OptionType::Call) {
                "call"
            } else {
                "put"
            },
            gamma
        );
    }
}

#[test]
fn test_call_rho_positive() {
    // Call rho should be positive (rate increase benefits calls)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let option = EquityOption {
        id: "CALL_RHO_TEST".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: Some("AAPL_DIV".into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, 100.0, 0.25, 0.05, 0.02);
    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let results = registry.compute(&[MetricId::Rho], &mut context).unwrap();
    let rho = *results.get(&MetricId::Rho).unwrap();

    assert!(rho > 0.0, "Call rho should be positive, got {}", rho);
}

#[test]
fn test_put_rho_negative() {
    // Put rho should be negative (rate increase hurts puts)
    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);

    let option = EquityOption {
        id: "PUT_RHO_TEST".into(),
        underlying_ticker: "AAPL".to_string(),
        strike: 100.0,
        option_type: OptionType::Put,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(100.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: Some("AAPL_DIV".into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, 100.0, 0.25, 0.05, 0.02);
    let registry = standard_registry();
    let pv = option.value(&market, as_of).unwrap();
    let mut context = MetricContext::new(
        Arc::new(option),
        Arc::new(market),
        as_of,
        pv,
        MetricContext::default_config(),
    );

    let results = registry.compute(&[MetricId::Rho], &mut context).unwrap();
    let rho = *results.get(&MetricId::Rho).unwrap();

    assert!(rho < 0.0, "Put rho should be negative, got {}", rho);
}

#[test]
fn test_cds_cs01_protection_buyer_positive() {
    // Protection buyer CS01 should be positive: when spreads widen, the protection
    // leg value increases more than premium payments owed, benefiting the buyer.
    // This is the market-standard directional sign convention.
    let as_of = date!(2025 - 01 - 01);

    use finstack_quant_core::market_data::term_structures::HazardCurve;

    let cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "CS01_BUY_TEST",
        Money::new(1_000_000.0, Currency::USD),
        200.0, // 200bp spread
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
        "HAZARD",
    )
    .expect("CDS construction should succeed");

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-0.05f64).exp()),
            (2.0f64, (-0.10f64).exp()),
            (3.0f64, (-0.15f64).exp()),
            (4.0f64, (-0.20f64).exp()),
            (5.0f64, (-0.25f64).exp()),
        ])
        .build()
        .unwrap();

    let hazard_curve = HazardCurve::builder("HAZARD")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([
            (0.0f64, 0.02f64),
            (1.0f64, 0.025f64),
            (2.0f64, 0.03f64),
            (3.0f64, 0.035f64),
            (4.0f64, 0.04f64),
            (5.0f64, 0.045f64),
        ])
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

    let results = registry.compute(&[MetricId::Cs01], &mut context).unwrap();
    let cs01 = *results.get(&MetricId::Cs01).unwrap();

    // Protection buyer benefits from spread widening, so CS01 > 0
    assert!(
        cs01 > 0.0,
        "Protection buyer CS01 should be positive, got {}",
        cs01
    );
}

#[test]
fn test_cds_cs01_protection_seller_negative() {
    // Protection seller CS01 should be negative: when spreads widen, the seller's
    // liability increases more than premium income, hurting the seller.
    // This validates that CS01 sign correctly reflects position direction.
    let as_of = date!(2025 - 01 - 01);

    use finstack_quant_core::market_data::term_structures::HazardCurve;

    let cds = crate::finstack_quant_test_utils::cds_sell_protection(
        "CS01_SELL_TEST",
        Money::new(1_000_000.0, Currency::USD),
        200.0, // 200bp spread
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
        "HAZARD",
    )
    .expect("CDS construction should succeed");

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-0.05f64).exp()),
            (2.0f64, (-0.10f64).exp()),
            (3.0f64, (-0.15f64).exp()),
            (4.0f64, (-0.20f64).exp()),
            (5.0f64, (-0.25f64).exp()),
        ])
        .build()
        .unwrap();

    let hazard_curve = HazardCurve::builder("HAZARD")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([
            (0.0f64, 0.02f64),
            (1.0f64, 0.025f64),
            (2.0f64, 0.03f64),
            (3.0f64, 0.035f64),
            (4.0f64, 0.04f64),
            (5.0f64, 0.045f64),
        ])
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

    let results = registry.compute(&[MetricId::Cs01], &mut context).unwrap();
    let cs01 = *results.get(&MetricId::Cs01).unwrap();

    // Protection seller is hurt by spread widening, so CS01 < 0
    assert!(
        cs01 < 0.0,
        "Protection seller CS01 should be negative, got {}",
        cs01
    );
}

#[test]
fn test_cds_cs01_opposite_signs() {
    // Verify that buy and sell protection CS01 have opposite signs for the same market.
    // This is a fundamental property: they are opposite positions.
    let as_of = date!(2025 - 01 - 01);

    use finstack_quant_core::market_data::term_structures::HazardCurve;

    let cds_buy = crate::finstack_quant_test_utils::cds_buy_protection(
        "CS01_BUY",
        Money::new(1_000_000.0, Currency::USD),
        200.0,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
        "HAZARD",
    )
    .expect("CDS construction should succeed");

    let cds_sell = crate::finstack_quant_test_utils::cds_sell_protection(
        "CS01_SELL",
        Money::new(1_000_000.0, Currency::USD),
        200.0,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
        "HAZARD",
    )
    .expect("CDS construction should succeed");

    let disc_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0f64, 1.0f64),
            (1.0f64, (-0.05f64).exp()),
            (2.0f64, (-0.10f64).exp()),
            (3.0f64, (-0.15f64).exp()),
            (4.0f64, (-0.20f64).exp()),
            (5.0f64, (-0.25f64).exp()),
        ])
        .build()
        .unwrap();

    let hazard_curve = HazardCurve::builder("HAZARD")
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([
            (0.0f64, 0.02f64),
            (1.0f64, 0.025f64),
            (2.0f64, 0.03f64),
            (3.0f64, 0.035f64),
            (4.0f64, 0.04f64),
            (5.0f64, 0.045f64),
        ])
        .build()
        .unwrap();

    let market = MarketContext::new().insert(disc_curve).insert(hazard_curve);

    let registry = standard_registry();

    // Compute CS01 for buy protection
    let pv_buy = cds_buy.value(&market, as_of).unwrap();
    let mut context_buy = MetricContext::new(
        Arc::new(cds_buy),
        Arc::new(market.clone()),
        as_of,
        pv_buy,
        MetricContext::default_config(),
    );
    let results_buy = registry
        .compute(&[MetricId::Cs01], &mut context_buy)
        .unwrap();
    let cs01_buy = *results_buy.get(&MetricId::Cs01).unwrap();

    // Compute CS01 for sell protection
    let pv_sell = cds_sell.value(&market, as_of).unwrap();
    let mut context_sell = MetricContext::new(
        Arc::new(cds_sell),
        Arc::new(market),
        as_of,
        pv_sell,
        MetricContext::default_config(),
    );
    let results_sell = registry
        .compute(&[MetricId::Cs01], &mut context_sell)
        .unwrap();
    let cs01_sell = *results_sell.get(&MetricId::Cs01).unwrap();

    // Buy and sell should have opposite signs
    assert!(
        cs01_buy * cs01_sell < 0.0,
        "Buy CS01 ({}) and sell CS01 ({}) should have opposite signs",
        cs01_buy,
        cs01_sell
    );

    // Their magnitudes should be approximately equal
    let diff_pct = ((cs01_buy.abs() - cs01_sell.abs()) / cs01_buy.abs()).abs();
    assert!(
        diff_pct < 0.01,
        "Buy and sell CS01 magnitudes should be approximately equal: buy={}, sell={}, diff={:.2}%",
        cs01_buy.abs(),
        cs01_sell.abs(),
        diff_pct * 100.0
    );
}

#[test]
fn test_put_call_parity() {
    // Put-Call Parity: C - P = S·exp(-q·T) - K·exp(-r·T)
    // This fundamental arbitrage relationship validates consistency of option pricing.
    // For ATM options, C - P ≈ (forward - strike) * DF when forward ≈ spot * exp((r-q)*T)
    use finstack_quant_core::dates::DayCountContext;

    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;
    let strike = 100.0; // ATM
    let rate = 0.05;
    let div_yield = 0.02;
    let vol = 0.25;

    // Create call option
    let call = EquityOption {
        id: "PARITY_CALL".into(),
        underlying_ticker: "AAPL".to_string(),
        strike,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(1.0, Currency::USD), // Unit notional for cleaner parity check
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: Some("AAPL_DIV".into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    // Create put option with same parameters
    let put = EquityOption {
        id: "PARITY_PUT".into(),
        underlying_ticker: "AAPL".to_string(),
        strike,
        option_type: OptionType::Put,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(1.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: Some("AAPL_DIV".into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, spot, vol, rate, div_yield);

    // Price both options
    let call_pv = call.value(&market, as_of).unwrap();
    let put_pv = put.value(&market, as_of).unwrap();

    // Calculate time to expiry in years (Act365F is simple: T = days / 365)
    let t = DayCount::Act365F
        .year_fraction(as_of, expiry, DayCountContext::default())
        .unwrap();

    // Put-call parity: C - P = S·exp(-q·T) - K·exp(-r·T)
    // Forward price: F = S·exp((r-q)·T) = S·exp(-q·T)·exp(r·T)
    // Discounted forward: S·exp(-q·T)
    // Discounted strike: K·exp(-r·T)
    let forward_value = spot * (-div_yield * t).exp();
    let strike_pv = strike * (-rate * t).exp();
    let parity_rhs = forward_value - strike_pv;
    let parity_lhs = call_pv.amount() - put_pv.amount();

    // Put-call parity should hold within 0.5% relative tolerance
    // Small differences can occur due to Money rounding in PV calculation
    let parity_error = (parity_lhs - parity_rhs).abs();
    let relative_error = parity_error / parity_rhs.abs().max(1e-10);
    assert!(
        relative_error < 0.005,
        "Put-call parity violated: C-P = {:.6}, expected = {:.6}, rel_error = {:.4}%",
        parity_lhs,
        parity_rhs,
        relative_error * 100.0
    );
}

#[test]
fn test_put_call_parity_delta_relationship() {
    // Derivative of put-call parity with respect to spot:
    // ∂/∂S (C - P) = ∂/∂S (S·exp(-q·T) - K·exp(-r·T))
    // Delta_C - Delta_P = exp(-q·T)
    // For small div yield: Delta_C - Delta_P ≈ 1 (normalized per share)
    use finstack_quant_core::dates::DayCountContext;

    let as_of = date!(2024 - 01 - 01);
    let expiry = date!(2025 - 01 - 01);
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let div_yield = 0.02;
    let vol = 0.25;

    let call = EquityOption {
        id: "DELTA_PARITY_CALL".into(),
        underlying_ticker: "AAPL".to_string(),
        strike,
        option_type: OptionType::Call,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(1.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: Some("AAPL_DIV".into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let put = EquityOption {
        id: "DELTA_PARITY_PUT".into(),
        underlying_ticker: "AAPL".to_string(),
        strike,
        option_type: OptionType::Put,
        exercise_style: ExerciseStyle::European,
        expiry,
        notional: Money::new(1.0, Currency::USD),
        day_count: DayCount::Act365F,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD-OIS".into(),
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL_VOL".into(),
        div_yield_id: Some("AAPL_DIV".into()),
        discrete_dividends: Vec::new(),
        pricing_overrides: PricingOverrides::default(),
        exercise_schedule: None,
        attributes: Default::default(),
    };

    let market = create_option_market(as_of, spot, vol, rate, div_yield);
    let registry = standard_registry();

    // Compute deltas
    let call_pv = call.value(&market, as_of).unwrap();
    let mut call_context = MetricContext::new(
        Arc::new(call),
        Arc::new(market.clone()),
        as_of,
        call_pv,
        MetricContext::default_config(),
    );
    let call_results = registry
        .compute(&[MetricId::Delta], &mut call_context)
        .unwrap();
    let call_delta = *call_results.get(&MetricId::Delta).unwrap();

    let put_pv = put.value(&market, as_of).unwrap();
    let mut put_context = MetricContext::new(
        Arc::new(put),
        Arc::new(market),
        as_of,
        put_pv,
        MetricContext::default_config(),
    );
    let put_results = registry
        .compute(&[MetricId::Delta], &mut put_context)
        .unwrap();
    let put_delta = *put_results.get(&MetricId::Delta).unwrap();

    // Delta_C - Delta_P = exp(-q·T)
    let t = DayCount::Act365F
        .year_fraction(as_of, expiry, DayCountContext::default())
        .unwrap();
    let expected_diff = (-div_yield * t).exp();
    let actual_diff = call_delta - put_delta;

    // Allow 1% tolerance for numerical differences
    let error = (actual_diff - expected_diff).abs() / expected_diff;
    assert!(
        error < 0.01,
        "Delta parity violated: Delta_C - Delta_P = {:.6}, exp(-q·T) = {:.6}, error = {:.2}%",
        actual_diff,
        expected_diff,
        error * 100.0
    );
}
