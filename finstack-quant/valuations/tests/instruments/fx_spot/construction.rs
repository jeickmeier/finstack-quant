//! FX Spot construction and builder pattern tests.

use super::common::*;
use finstack_quant_core::{
    currency::Currency, dates::BusinessDayConvention, money::Money, types::InstrumentId,
};
use finstack_quant_valuations::instruments::fx::fx_spot::FxSpot;

#[test]
fn test_basic_construction() {
    let fx = FxSpot::builder()
        .id(InstrumentId::new("EURUSD"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .notional(Money::from((1_i64, Currency::EUR)))
        .build()
        .expect("valid FX spot");

    assert_eq!(fx.id.as_str(), "EURUSD");
    assert_eq!(fx.base_currency, Currency::EUR);
    assert_eq!(fx.quote_currency, Currency::USD);
    assert!(fx.settlement_date.is_none());
    assert!(fx.quoted_spot.is_none());
    assert_eq!(
        fx.notional,
        Money::new(1.0, Currency::EUR).expect("valid money fixture")
    );
    // Default BDC is ModifiedFollowing per ISDA FX settlement standard
    assert_eq!(
        fx.business_day_convention,
        BusinessDayConvention::ModifiedFollowing
    );
}

#[test]
fn test_construction_with_rate() {
    let fx = FxSpot {
        quoted_spot: Some(1.18),
        ..sample_eurusd()
    };

    assert_eq!(fx.quoted_spot, Some(1.18));
}

#[test]
fn test_construction_with_notional() {
    let fx = FxSpot {
        notional: Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        ..sample_eurusd()
    };

    assert_eq!(fx.notional.amount(), 1_000_000.0);
    assert_eq!(fx.notional.currency(), Currency::EUR);
}

#[test]
fn test_construction_with_mismatched_currency_fails() {
    let result = FxSpot::builder()
        .id(InstrumentId::new("EURUSD"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .build();

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        finstack_quant_core::Error::CurrencyMismatch { .. }
    ));
}

#[test]
fn test_construction_with_settlement() {
    let settlement = d(2025, 1, 17);
    let fx = FxSpot {
        settlement_date: Some(settlement),
        ..sample_eurusd()
    };

    assert_eq!(fx.settlement_date, Some(settlement));
}

#[test]
fn test_construction_with_calendar() {
    let fx = FxSpot {
        base_calendar_id: Some("target2".into()),
        quote_calendar_id: Some("usny".into()),
        ..sample_eurusd()
    };

    assert_eq!(fx.base_calendar_id.as_deref(), Some("target2"));
    assert_eq!(fx.quote_calendar_id.as_deref(), Some("usny"));
}

#[test]
fn test_construction_full_builder() {
    let fx = FxSpot::builder()
        .id(InstrumentId::new("GBPUSD"))
        .base_currency(Currency::GBP)
        .quote_currency(Currency::USD)
        .notional(Money::new(5_000_000.0, Currency::GBP).expect("valid money fixture"))
        .quoted_spot(1.32)
        .settlement_date(d(2025, 1, 17))
        .business_day_convention(BusinessDayConvention::ModifiedFollowing)
        .base_calendar_id("London".into())
        .quote_calendar_id("USNY".into())
        .build()
        .expect("valid FX spot");

    assert_eq!(fx.id.as_str(), "GBPUSD");
    assert_eq!(fx.base_currency, Currency::GBP);
    assert_eq!(fx.quote_currency, Currency::USD);
    assert_eq!(fx.notional.amount(), 5_000_000.0);
    assert_eq!(fx.quoted_spot, Some(1.32));
    assert_eq!(fx.settlement_date, Some(d(2025, 1, 17)));
    assert_eq!(
        fx.business_day_convention,
        BusinessDayConvention::ModifiedFollowing
    );
    assert_eq!(fx.base_calendar_id.as_deref(), Some("London"));
    assert_eq!(fx.quote_calendar_id.as_deref(), Some("USNY"));
}

#[test]
fn test_effective_notional_default() {
    let fx = sample_eurusd();
    let notional = fx.notional;

    assert_eq!(notional.amount(), 1.0);
    assert_eq!(notional.currency(), Currency::EUR);
}

#[test]
fn test_effective_notional_with_explicit_value() {
    let fx = FxSpot {
        notional: Money::new(2_500_000.0, Currency::EUR).expect("valid money fixture"),
        ..sample_eurusd()
    };
    let notional = fx.notional;

    assert_eq!(notional.amount(), 2_500_000.0);
    assert_eq!(notional.currency(), Currency::EUR);
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
        let fx = FxSpot::builder()
            .id(InstrumentId::new(expected_name))
            .base_currency(base)
            .quote_currency(quote)
            .notional(Money::from((1_i64, base)))
            .build()
            .expect("valid FX spot");
        assert_eq!(fx.base_currency, base);
        assert_eq!(fx.quote_currency, quote);
    }
}

#[test]
fn test_construction_with_large_notional() {
    let fx = FxSpot {
        notional: Money::new(1_000_000_000.0, Currency::EUR).expect("valid money fixture"),
        ..sample_eurusd()
    };

    assert_eq!(fx.notional.amount(), 1_000_000_000.0);
}

#[test]
fn test_construction_with_small_notional() {
    let fx = FxSpot {
        notional: Money::new(0.01, Currency::EUR).expect("valid money fixture"),
        ..sample_eurusd()
    };

    approx_eq(fx.notional.amount(), 0.01, EPSILON, "Small notional");
}

#[test]
fn test_clone_preserves_all_fields() {
    let fx = FxSpot::builder()
        .id(InstrumentId::new("EURUSD"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
        .quoted_spot(1.18)
        .settlement_date(d(2025, 1, 17))
        .build()
        .expect("valid FX spot");

    let cloned = fx.clone();

    assert_eq!(cloned.id.as_str(), fx.id.as_str());
    assert_eq!(cloned.base_currency, fx.base_currency);
    assert_eq!(cloned.quote_currency, fx.quote_currency);
    assert_eq!(cloned.notional, fx.notional);
    assert_eq!(cloned.quoted_spot, fx.quoted_spot);
    assert_eq!(cloned.settlement_date, fx.settlement_date);
}

#[test]
fn test_debug_representation() {
    let fx = FxSpot {
        quoted_spot: Some(1.20),
        ..sample_eurusd()
    };
    let debug_str = format!("{:?}", fx);

    assert!(debug_str.contains("FxSpot"));
    assert!(debug_str.contains("EUR"));
    assert!(debug_str.contains("USD"));
}

#[test]
fn test_with_notional_valid_currency() {
    let fx = FxSpot {
        notional: Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        ..sample_eurusd()
    };

    assert_eq!(fx.notional.amount(), 1_000_000.0);
}
