//! Integration tests for NDF pricing.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fx::ndf::{Ndf, NdfQuoteConvention};
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::pricer::{standard_registry, InstrumentType, ModelKey, PricerKey};
use std::sync::Arc;
use time::Month;

/// Create a test market with USD discount curve and FX matrix.
fn create_test_market(as_of: Date) -> MarketContext {
    // Create USD discount curve at 5% using builder
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (0.5, 0.9753), (1.0, 0.9512)])
        .build()
        .expect("should build");

    // Create FX provider with CNY/USD = 7.25
    let fx_provider = {
        let p = Arc::new(SimpleFxProvider::new());
        p.set_quote(Currency::CNY, Currency::USD, 7.25)
            .expect("valid rate");
        p
    };
    let fx_matrix = FxMatrix::new(fx_provider);

    MarketContext::new().insert(usd_curve).insert_fx(fx_matrix)
}

#[test]
fn test_ndf_pricing_pre_fixing_at_market() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let fixing_date = Date::from_calendar_date(2024, Month::April, 13).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("valid date");
    let market = create_test_market(as_of);

    // Create NDF at market spot rate
    let ndf = Ndf::builder()
        .id(InstrumentId::new("USDCNY-ATM"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(fixing_date)
        .maturity(maturity)
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25) // At market
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .forward_rate_override_opt(Some(7.25))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = ndf.value(&market, as_of).expect("should price");

    // At-market NDF should have PV ≈ 0 (small residual from curve interpolation)
    assert!(
        npv.amount().abs() < 100.0,
        "At-market NDF PV should be near zero, got {}",
        npv.amount()
    );
    assert_eq!(npv.currency(), Currency::USD);
}

#[test]
fn test_ndf_pricing_post_fixing_favorable() {
    let as_of = Date::from_calendar_date(2024, Month::April, 14).expect("valid date");
    let fixing_date = Date::from_calendar_date(2024, Month::April, 13).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("valid date");
    let market = create_test_market(as_of);

    // Create NDF with favorable fixing (CNY weakened)
    let ndf = Ndf::builder()
        .id(InstrumentId::new("USDCNY-FIXED"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(fixing_date)
        .maturity(maturity)
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25)
        .fixing_rate_opt(Some(7.30)) // CNY weakened, fixing rate > contract
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = ndf.value(&market, as_of).expect("should price");

    // Fixing rate > contract rate: we're receiving more USD than contracted
    // Settlement = 10M × (1/7.25 - 1/7.30) ≈ positive
    assert!(
        npv.amount() > 0.0,
        "NDF with favorable fixing should have positive PV, got {}",
        npv.amount()
    );
    assert_eq!(npv.currency(), Currency::USD);
}

#[test]
fn test_ndf_pricing_post_fixing_unfavorable() {
    let as_of = Date::from_calendar_date(2024, Month::April, 14).expect("valid date");
    let fixing_date = Date::from_calendar_date(2024, Month::April, 13).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("valid date");
    let market = create_test_market(as_of);

    // Create NDF with unfavorable fixing (CNY strengthened)
    let ndf = Ndf::builder()
        .id(InstrumentId::new("USDCNY-FIXED"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(fixing_date)
        .maturity(maturity)
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25)
        .fixing_rate_opt(Some(7.20)) // CNY strengthened, fixing rate < contract
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = ndf.value(&market, as_of).expect("should price");

    // Fixing rate < contract rate means negative PV
    assert!(
        npv.amount() < 0.0,
        "NDF with unfavorable fixing should have negative PV, got {}",
        npv.amount()
    );
}

#[test]
fn test_ndf_pricing_expired() {
    let as_of = Date::from_calendar_date(2024, Month::April, 20).expect("valid date");
    let fixing_date = Date::from_calendar_date(2024, Month::April, 13).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("valid date");
    let market = create_test_market(as_of);

    let ndf = Ndf::builder()
        .id(InstrumentId::new("USDCNY-EXPIRED"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(fixing_date)
        .maturity(maturity)
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .forward_rate_override_opt(Some(7.25))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = ndf.value(&market, as_of).expect("should price");
    assert_eq!(npv.amount(), 0.0);
}

#[test]
fn test_ndf_registry_pricer() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let fixing_date = Date::from_calendar_date(2024, Month::April, 13).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("valid date");
    let market = create_test_market(as_of);

    let ndf = Ndf::builder()
        .id(InstrumentId::new("USDCNY-REGISTRY"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(fixing_date)
        .maturity(maturity)
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25)
        .forward_rate_override_opt(Some(7.25))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let registry = standard_registry();

    // Verify pricer is registered
    assert!(
        registry
            .get_pricer(PricerKey::new(InstrumentType::Ndf, ModelKey::Discounting))
            .is_some(),
        "NDF pricer should be registered"
    );

    // Price through registry
    let result = registry
        .price_with_metrics(
            &ndf,
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
fn test_ndf_pre_fixing_requires_forward_input() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let fixing_date = Date::from_calendar_date(2024, Month::April, 13).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("valid date");
    let market = create_test_market(as_of);

    let ndf = Ndf::builder()
        .id(InstrumentId::new("USDCNY-NO-FWD"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(fixing_date)
        .maturity(maturity)
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let err = ndf
        .value(&market, as_of)
        .expect_err("pre-fixing NDF should require foreign curve or forward override");
    assert!(
        err.to_string().contains("forward_rate_override"),
        "error should mention explicit forward input: {}",
        err
    );
}

#[test]
fn test_ndf_pricing_with_foreign_curve() {
    let as_of = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");
    let fixing_date = Date::from_calendar_date(2024, Month::April, 13).expect("valid date");
    let maturity = Date::from_calendar_date(2024, Month::April, 15).expect("valid date");

    // Create market with both USD and CNY curves
    let usd_curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (0.5, 0.9753), (1.0, 0.9512)])
        .build()
        .expect("should build");

    // CNY at 2%: DF(0.5) = exp(-0.02*0.5) ≈ 0.9900, DF(1.0) = exp(-0.02) ≈ 0.9802
    let cny_curve = DiscountCurve::builder("CNY-OIS")
        .base_date(as_of)
        .knots(vec![(0.0, 1.0), (0.5, 0.9900), (1.0, 0.9802)])
        .build()
        .expect("should build");

    let fx_provider = {
        let p = Arc::new(SimpleFxProvider::new());
        p.set_quote(Currency::CNY, Currency::USD, 7.25)
            .expect("valid rate");
        p
    };
    let fx_matrix = FxMatrix::new(fx_provider);

    let market = MarketContext::new()
        .insert(usd_curve)
        .insert(cny_curve)
        .insert_fx(fx_matrix);

    // NDF with foreign curve for proper CIRP calculation
    let ndf = Ndf::builder()
        .id(InstrumentId::new("USDCNY-CIRP"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(fixing_date)
        .maturity(maturity)
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id_opt(Some(CurveId::new("CNY-OIS")))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let npv = ndf.value(&market, as_of).expect("should price");

    // With foreign curve, CIRP should be used
    // USD rate (5%) > CNY rate (2%), so CNY should appreciate in forward
    // Forward rate should be slightly less than spot
    // At-market with spot rate should result in small PV due to CIRP adjustment
    assert_eq!(npv.currency(), Currency::USD);
}

#[test]
fn test_ndf_instrument_key() {
    let ndf = Ndf::example();
    assert_eq!(ndf.key(), InstrumentType::Ndf);
}

#[test]
fn test_ndf_is_fixed() {
    let ndf_unfixed = Ndf::example();
    assert!(!ndf_unfixed.is_fixed());

    let ndf_fixed = Ndf::example().with_fixing_rate(7.30).expect("valid rate");
    assert!(ndf_fixed.is_fixed());
}

#[test]
fn test_ndf_serde_roundtrip() {
    let ndf = Ndf::example();

    let json = serde_json::to_string_pretty(&ndf).expect("serialize");
    let parsed: Ndf = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(ndf.id.as_str(), parsed.id.as_str());
    assert_eq!(ndf.base_currency, parsed.base_currency);
    assert_eq!(ndf.settlement_currency, parsed.settlement_currency);
    assert_eq!(ndf.contract_rate, parsed.contract_rate);
}
