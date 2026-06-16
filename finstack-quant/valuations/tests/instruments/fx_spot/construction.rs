//! FX Spot construction and builder pattern tests.

use super::common::*;
use finstack_quant_core::{
    currency::Currency, dates::BusinessDayConvention, money::Money, types::InstrumentId,
};
use finstack_quant_valuations::instruments::fx::fx_spot::FxSpot;

#[test]
fn test_basic_construction() {
    let fx = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD);

    assert_eq!(fx.id.as_str(), "EURUSD");
    assert_eq!(fx.base_currency, Currency::EUR);
    assert_eq!(fx.quote_currency, Currency::USD);
    assert!(fx.settlement.is_none());
    assert!(fx.spot_rate.is_none());
    assert_eq!(fx.notional, Money::new(1.0, Currency::EUR));
    // Default BDC is ModifiedFollowing per ISDA FX settlement standard
    assert_eq!(fx.bdc, BusinessDayConvention::ModifiedFollowing);
}

#[test]
fn test_construction_with_rate() {
    let fx = sample_eurusd().with_rate(1.18).expect("test rate");

    assert_eq!(fx.spot_rate, Some(1.18));
}

#[test]
fn test_construction_with_notional() {
    let fx = sample_eurusd()
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .unwrap();

    assert_eq!(fx.notional.amount(), 1_000_000.0);
    assert_eq!(fx.notional.currency(), Currency::EUR);
}

#[test]
fn test_construction_with_mismatched_currency_fails() {
    let result = sample_eurusd().with_notional(Money::new(1_000_000.0, Currency::USD));

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        finstack_quant_core::Error::CurrencyMismatch { .. }
    ));
}

#[test]
fn test_construction_with_settlement() {
    let settlement = d(2025, 1, 17);
    let fx = sample_eurusd().with_settlement(settlement);

    assert_eq!(fx.settlement, Some(settlement));
}

#[test]
fn test_construction_with_calendar() {
    let fx = sample_eurusd()
        .with_base_calendar_id("target2")
        .with_quote_calendar_id("usny");

    assert_eq!(fx.base_calendar_id.as_deref(), Some("target2"));
    assert_eq!(fx.quote_calendar_id.as_deref(), Some("usny"));
}

#[test]
fn test_construction_full_builder() {
    let fx = FxSpot::new(InstrumentId::new("GBPUSD"), Currency::GBP, Currency::USD)
        .with_notional(Money::new(5_000_000.0, Currency::GBP))
        .unwrap()
        .with_rate(1.32)
        .expect("test rate")
        .with_settlement(d(2025, 1, 17))
        .with_bdc(BusinessDayConvention::ModifiedFollowing)
        .with_base_calendar_id("London")
        .with_quote_calendar_id("USNY");

    assert_eq!(fx.id.as_str(), "GBPUSD");
    assert_eq!(fx.base_currency, Currency::GBP);
    assert_eq!(fx.quote_currency, Currency::USD);
    assert_eq!(fx.notional.amount(), 5_000_000.0);
    assert_eq!(fx.spot_rate, Some(1.32));
    assert_eq!(fx.settlement, Some(d(2025, 1, 17)));
    assert_eq!(fx.bdc, BusinessDayConvention::ModifiedFollowing);
    assert_eq!(fx.base_calendar_id.as_deref(), Some("London"));
    assert_eq!(fx.quote_calendar_id.as_deref(), Some("USNY"));
}

#[test]
fn test_effective_notional_default() {
    let fx = sample_eurusd();
    let notional = fx.effective_notional();

    assert_eq!(notional.amount(), 1.0);
    assert_eq!(notional.currency(), Currency::EUR);
}

#[test]
fn test_effective_notional_with_explicit_value() {
    let fx = sample_eurusd()
        .with_notional(Money::new(2_500_000.0, Currency::EUR))
        .unwrap();
    let notional = fx.effective_notional();

    assert_eq!(notional.amount(), 2_500_000.0);
    assert_eq!(notional.currency(), Currency::EUR);
}

#[test]
fn test_pair_name_generation() {
    let eurusd = sample_eurusd();
    assert_eq!(eurusd.pair_name(), "EURUSD");

    let gbpusd = sample_gbpusd();
    assert_eq!(gbpusd.pair_name(), "GBPUSD");

    let usdjpy = sample_usdjpy();
    assert_eq!(usdjpy.pair_name(), "USDJPY");
}

#[test]
fn test_construction_with_various_currencies() {
    let pairs = vec![
        (Currency::EUR, Currency::USD, "EURUSD"),
        (Currency::GBP, Currency::USD, "GBPUSD"),
        (Currency::USD, Currency::JPY, "USDJPY"),
        (Currency::AUD, Currency::USD, "AUDUSD"),
        (Currency::USD, Currency::CHF, "USDCHF"),
    ];

    for (base, quote, expected_name) in pairs {
        let fx = FxSpot::new(InstrumentId::new(expected_name), base, quote);
        assert_eq!(fx.base_currency, base);
        assert_eq!(fx.quote_currency, quote);
        assert_eq!(fx.pair_name(), expected_name);
    }
}

#[test]
fn test_construction_with_large_notional() {
    let fx = sample_eurusd()
        .with_notional(Money::new(1_000_000_000.0, Currency::EUR))
        .unwrap();

    assert_eq!(fx.effective_notional().amount(), 1_000_000_000.0);
}

#[test]
fn test_construction_with_small_notional() {
    let fx = sample_eurusd()
        .with_notional(Money::new(0.01, Currency::EUR))
        .unwrap();

    assert_approx_eq(
        fx.effective_notional().amount(),
        0.01,
        EPSILON,
        "Small notional",
    );
}

#[test]
fn test_clone_preserves_all_fields() {
    let fx = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .unwrap()
        .with_rate(1.18)
        .expect("test rate")
        .with_settlement(d(2025, 1, 17));

    let cloned = fx.clone();

    assert_eq!(cloned.id.as_str(), fx.id.as_str());
    assert_eq!(cloned.base_currency, fx.base_currency);
    assert_eq!(cloned.quote_currency, fx.quote_currency);
    assert_eq!(cloned.notional, fx.notional);
    assert_eq!(cloned.spot_rate, fx.spot_rate);
    assert_eq!(cloned.settlement, fx.settlement);
}

#[test]
fn test_debug_representation() {
    let fx = sample_eurusd().with_rate(1.20);
    let debug_str = format!("{:?}", fx);

    assert!(debug_str.contains("FxSpot"));
    assert!(debug_str.contains("EUR"));
    assert!(debug_str.contains("USD"));
}

#[test]
fn test_with_notional_valid_currency() {
    let fx = sample_eurusd()
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .unwrap();

    assert_eq!(fx.effective_notional().amount(), 1_000_000.0);
}
