//! Integration tests for FX Forward pricing.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fx::fx_forward::FxForward;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::pricer::{standard_registry, InstrumentType, ModelKey, PricerKey};
use std::sync::Arc;
use time::Month;

/// Create a test market with USD and EUR discount curves and FX matrix.
fn create_test_market(as_of: Date) -> MarketContext {
    // Create discount curves using builder (flat 5% USD, 3% EUR)
    // Approximate DFs: DF(0.5) = exp(-0.05*0.5) ≈ 0.9753, DF(1.0) = exp(-0.05) ≈ 0.9512
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (0.5, 0.9753), (1.0, 0.9512)])
        .build()
        .expect("should build");

    // EUR at 3%: DF(0.5) = exp(-0.03*0.5) ≈ 0.9851, DF(1.0) = exp(-0.03) ≈ 0.9704
    let eur_curve = DiscountCurve::builder("EUR-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (0.5, 0.9851), (1.0, 0.9704)])
        .build()
        .expect("should build");

    // Create FX provider with EUR/USD = 1.10
    let fx_provider = Arc::new(SimpleFxProvider::new());
    fx_provider
        .set_quote(Currency::EUR, Currency::USD, 1.10)
        .expect("valid rate");
    let fx_matrix = FxMatrix::new(fx_provider);

    MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve)
        .insert_fx(fx_matrix)
}

#[test]
fn test_fx_forward_pricing_at_market() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::July, 15).expect("valid date");
    let market = create_test_market(as_of);

    // Create at-market forward (no contract rate specified)
    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-ATM"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = forward.value(&market, as_of).expect("should price");

    // At-market forward should have PV ≈ 0
    assert!(
        npv.amount().abs() < 1.0,
        "At-market forward PV should be near zero, got {}",
        npv.amount()
    );
    assert_eq!(npv.currency(), Currency::USD);
}

#[test]
fn test_fx_forward_pricing_favorable_contract_rate() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::July, 15).expect("valid date");
    let market = create_test_market(as_of);

    // Create forward with favorable contract rate (we buy EUR cheap)
    // Spot is 1.10, so if we lock in 1.05, we're getting a good deal
    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-FAVORABLE"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .contract_rate_opt(Some(1.05))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = forward.value(&market, as_of).expect("should price");

    // Contract rate below market forward means positive PV
    assert!(
        npv.amount() > 0.0,
        "Forward with favorable rate should have positive PV, got {}",
        npv.amount()
    );
    assert_eq!(npv.currency(), Currency::USD);
}

#[test]
fn test_fx_forward_pricing_unfavorable_contract_rate() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::July, 15).expect("valid date");
    let market = create_test_market(as_of);

    // Create forward with unfavorable contract rate
    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-UNFAVORABLE"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .contract_rate_opt(Some(1.15))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = forward.value(&market, as_of).expect("should price");

    // Contract rate above market forward means negative PV
    assert!(
        npv.amount() < 0.0,
        "Forward with unfavorable rate should have negative PV, got {}",
        npv.amount()
    );
}

#[test]
fn test_fx_forward_market_forward_rate() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::July, 15).expect("valid date");
    let market = create_test_market(as_of);

    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-FWD"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let fwd_rate = forward
        .market_forward_rate(&market, as_of)
        .expect("should calculate");

    // CIRP: F = S × DF_foreign / DF_domestic
    let spot = 1.10;
    let usd_curve = market.get_discount("USD-OIS").unwrap();
    let eur_curve = market.get_discount("EUR-OIS").unwrap();
    let df_domestic = usd_curve.df_between_dates(as_of, maturity).unwrap();
    let df_foreign = eur_curve.df_between_dates(as_of, maturity).unwrap();
    let expected = spot * df_foreign / df_domestic;

    assert!(
        (fwd_rate - expected).abs() < 1e-6,
        "Forward rate should match CIP: got={}, expected={}",
        fwd_rate,
        expected
    );
}

#[test]
fn test_fx_forward_expired() {
    let as_of = Date::from_calendar_date(2024, Month::July, 20).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::July, 15).expect("valid date");
    let market = create_test_market(as_of);

    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-EXPIRED"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    // Expired forward should return zero PV (settled trade)
    let npv = forward.value(&market, as_of).expect("should price");
    assert_eq!(npv.amount(), 0.0, "Expired forward should have zero PV");
}

#[test]
fn test_fx_forward_same_day_maturity() {
    let as_of = Date::from_calendar_date(2024, Month::July, 15).expect("valid date");
    let market = create_test_market(as_of);

    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-SAMEDAY"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(as_of)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    // Same-day maturity should return zero PV
    let npv = forward.value(&market, as_of).expect("should price");
    assert_eq!(
        npv.amount(),
        0.0,
        "Same-day maturity forward should have zero PV"
    );
}

#[test]
fn test_fx_forward_with_spot_override() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::July, 15).expect("valid date");
    let market = create_test_market(as_of);

    // Create forward with spot rate override different from FX matrix
    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-OVERRIDE"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .contract_rate_opt(Some(1.12))
        .spot_rate_override_opt(Some(1.12)) // Override spot to match contract
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = forward.value(&market, as_of).expect("should price");

    // With spot=contract=1.12 but fair forward ≈ 1.1313 (from 5%/3% rates), NPV ≈ $11k
    // 20k bound allows for market parameter variation while still catching large errors
    assert!(
        npv.amount().abs() < 20_000.0,
        "PV should be reasonable for interest-differential scenario, got {}",
        npv.amount()
    );
}

#[test]
fn test_fx_forward_with_forward_points() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::July, 15).expect("valid date");
    let market = create_test_market(as_of);

    // Create forward using forward points
    let spot = 1.10;
    let forward_points = 0.005; // 50 pips

    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-POINTS"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build")
        .with_forward_points(spot, forward_points)
        .expect("valid forward points");

    assert_eq!(forward.spot_rate_override, Some(1.10));
    assert!((forward.contract_rate.unwrap() - 1.105).abs() < 1e-10);

    let npv = forward.value(&market, as_of).expect("should price");
    assert_eq!(npv.currency(), Currency::USD);
}

#[test]
fn test_fx_forward_registry_pricer() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::July, 15).expect("valid date");
    let market = create_test_market(as_of);

    let forward = FxForward::builder()
        .id(InstrumentId::new("EURUSD-REGISTRY"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(maturity)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let registry = standard_registry();

    // Verify pricer is registered
    assert!(
        registry
            .get_pricer(PricerKey::new(
                InstrumentType::FxForward,
                ModelKey::Discounting
            ))
            .is_some(),
        "FxForward pricer should be registered"
    );

    // Price through registry
    let result = registry
        .price_with_metrics(
            &forward,
            ModelKey::Discounting,
            &market,
            as_of,
            &[],
            Default::default(),
        )
        .expect("should price through registry");

    assert_eq!(result.value.currency(), Currency::USD);
}

#[test]
fn test_fx_forward_instrument_key() {
    let forward = FxForward::example().unwrap();
    assert_eq!(forward.key(), InstrumentType::FxForward);
}

#[test]
fn test_fx_forward_serde_roundtrip() {
    let forward = FxForward::example().unwrap();

    let json = serde_json::to_string_pretty(&forward).expect("serialize");
    let parsed: FxForward = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(forward.id.as_str(), parsed.id.as_str());
    assert_eq!(forward.base_currency, parsed.base_currency);
    assert_eq!(forward.quote_currency, parsed.quote_currency);
    assert_eq!(forward.contract_rate, parsed.contract_rate);
}
