//! Market standards and industry benchmark tests for FX Spot.

use super::super::common::*;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::HashMap;
use finstack_quant_core::{
    currency::Currency,
    market_data::context::MarketContext,
    money::{fx::FxMatrix, Money},
    types::InstrumentId,
};
use finstack_quant_valuations::{
    instruments::{FxSpot, Instrument},
    pricer::InstrumentType,
};
use std::sync::Arc;

#[test]
fn test_standard_eurusd_t_plus_2_settlement() {
    // Market standard: FX spot settles T+2 business days
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let market = MarketContext::new();
    let as_of = d(2025, 1, 15); // Wednesday

    let cashflows = fx.dated_cashflows(&market, as_of).unwrap();

    assert_eq!(cashflows.len(), 1);
    // T+2 from Wed = Fri (accounting for weekends)
    assert_eq!(cashflows[0].0, d(2025, 1, 17));
}

#[test]
fn test_standard_major_currency_pairs() {
    // Test standard major currency pairs
    let market = market_with_fx_matrix();
    let as_of = test_date();

    let pairs = vec![
        (Currency::EUR, Currency::USD, "EURUSD", 1.20),
        (Currency::GBP, Currency::USD, "GBPUSD", 1.40),
        (Currency::USD, Currency::JPY, "USDJPY", 110.0),
    ];

    for (base, quote, name, expected_rate) in pairs {
        let fx = FxSpot::new(InstrumentId::new(name), base, quote)
            .with_notional(Money::new(1_000_000.0, base))
            .unwrap();

        let pv = fx.value(&market, as_of).unwrap();
        assert_eq!(pv.currency(), quote);

        let expected_pv = 1_000_000.0 * expected_rate;
        assert_approx_eq(pv.amount(), expected_pv, LARGE_EPSILON, name);
    }
}

#[test]
fn test_standard_notional_sizes() {
    // Test industry-standard notional sizes
    let market = MarketContext::new();
    let rate = 1.20;

    let notionals = vec![
        (1_000_000.0, "Standard"),
        (5_000_000.0, "Large"),
        (10_000_000.0, "Very Large"),
        (100_000.0, "Small"),
    ];

    for (notional, desc) in notionals {
        let fx = eurusd_with_notional(notional, rate);
        let pv = fx.value(&market, test_date()).unwrap();

        let expected = notional * rate;
        assert_approx_eq(pv.amount(), expected, LARGE_EPSILON, desc);
    }
}

#[test]
fn test_spot_rate_market_convention() {
    // Spot rate convention: base per quote (e.g., EUR/USD = USD per 1 EUR)
    let fx = eurusd_with_notional(1.0, 1.20); // 1 EUR = 1.20 USD
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_eq!(pv.amount(), 1.20, "1 EUR = 1.20 USD");
}

#[test]
fn test_cross_rate_consistency() {
    // Test cross-rate consistency: EUR/GBP = EUR/USD / GBP/USD
    let market = market_with_fx_matrix();
    let as_of = test_date();

    let eur_usd = sample_eurusd()
        .with_notional(Money::new(1.0, Currency::EUR))
        .unwrap();
    let gbp_usd = sample_gbpusd()
        .with_notional(Money::new(1.0, Currency::GBP))
        .unwrap();

    let eur_usd_rate = eur_usd.value(&market, as_of).unwrap().amount(); // 1.20
    let gbp_usd_rate = gbp_usd.value(&market, as_of).unwrap().amount(); // 1.40

    let expected_eur_gbp = eur_usd_rate / gbp_usd_rate; // 1.20 / 1.40

    let eur_gbp = FxSpot::new(InstrumentId::new("EURGBP"), Currency::EUR, Currency::GBP)
        .with_notional(Money::new(1.0, Currency::EUR))
        .unwrap();

    let eur_gbp_rate = eur_gbp.value(&market, as_of).unwrap().amount();

    assert_approx_eq(
        eur_gbp_rate,
        expected_eur_gbp,
        LARGE_EPSILON,
        "Cross-rate consistency",
    );
}

#[test]
fn test_inverse_pair_consistency() {
    // EUR/USD and USD/EUR should be reciprocals
    let market = market_with_fx_matrix();
    let as_of = test_date();

    let eur_usd = sample_eurusd()
        .with_notional(Money::new(1.0, Currency::EUR))
        .unwrap();
    let usd_eur = FxSpot::new(InstrumentId::new("USDEUR"), Currency::USD, Currency::EUR)
        .with_notional(Money::new(1.0, Currency::USD))
        .unwrap();

    let eur_usd_rate = eur_usd.value(&market, as_of).unwrap().amount();
    let usd_eur_rate = usd_eur.value(&market, as_of).unwrap().amount();

    let product = eur_usd_rate * usd_eur_rate;
    assert_approx_eq(product, 1.0, LARGE_EPSILON, "Inverse pair consistency");
}

#[test]
fn test_pip_value_calculation() {
    // Standard pip value calculation for FX
    // 1 pip = 0.0001 for most pairs
    let fx = eurusd_with_notional(100_000.0, 1.2000);
    let market = MarketContext::new();

    let pv1 = fx.value(&market, test_date()).unwrap().amount();

    let fx_plus_pip = eurusd_with_notional(100_000.0, 1.2001);
    let pv2 = fx_plus_pip.value(&market, test_date()).unwrap().amount();

    let pip_value = pv2 - pv1;

    // 1 pip on 100k notional = 10 USD
    assert_approx_eq(pip_value, 10.0, LARGE_EPSILON, "Standard pip value");
}

#[test]
fn test_basis_point_sensitivity() {
    // Test 1 basis point (0.01%) sensitivity
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let market = MarketContext::new();

    let pv_base = fx.value(&market, test_date()).unwrap().amount();

    // 1 bp change in rate
    let fx_shifted = eurusd_with_notional(1_000_000.0, 1.20 * 1.0001);
    let pv_shifted = fx_shifted.value(&market, test_date()).unwrap().amount();

    let sensitivity = pv_shifted - pv_base;

    // Expected: 1M * 1.20 * 0.0001 = 120
    assert_approx_eq(sensitivity, 120.0, LARGE_EPSILON, "Basis point sensitivity");
}

#[test]
fn test_triangular_arbitrage_absence() {
    // Verify no triangular arbitrage: EUR/USD * USD/JPY = EUR/JPY
    // Need to add EUR/JPY to the mock provider
    let mut rates = HashMap::default();
    rates.insert((Currency::EUR, Currency::USD), 1.20);
    rates.insert((Currency::USD, Currency::JPY), 110.0);
    rates.insert((Currency::EUR, Currency::JPY), 1.20 * 110.0); // 132.0

    let provider = MockFxProvider { rates };
    let fx_matrix = FxMatrix::new(Arc::new(provider));
    let market = MarketContext::new().insert_fx(fx_matrix);
    let as_of = test_date();

    let eur_usd = sample_eurusd()
        .with_notional(Money::new(1.0, Currency::EUR))
        .unwrap()
        .value(&market, as_of)
        .unwrap()
        .amount();

    let usd_jpy = sample_usdjpy()
        .with_notional(Money::new(1.0, Currency::USD))
        .unwrap()
        .value(&market, as_of)
        .unwrap()
        .amount();

    let eur_jpy_implied = eur_usd * usd_jpy;

    let eur_jpy = FxSpot::new(InstrumentId::new("EURJPY"), Currency::EUR, Currency::JPY)
        .with_notional(Money::new(1.0, Currency::EUR))
        .unwrap()
        .value(&market, as_of)
        .unwrap()
        .amount();

    assert_approx_eq(
        eur_jpy,
        eur_jpy_implied,
        LARGE_EPSILON,
        "No triangular arbitrage",
    );
}

#[test]
fn test_zero_value_at_par() {
    // At spot rate, the NPV represents the quote currency amount
    // This is not zero-value like a swap at par, but confirms pricing logic
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_eq!(pv.currency(), Currency::USD);
    assert_approx_eq(pv.amount(), 1_200_000.0, EPSILON, "PV at spot");
}

#[test]
fn test_instrument_trait_compliance() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);

    // Test Instrument trait methods
    assert_eq!(fx.id(), "EURUSD");
    assert_eq!(fx.key(), InstrumentType::FxSpot);
    assert!(fx.as_any().is::<FxSpot>());

    // Test clone
    let _cloned = fx.clone_box();
}
