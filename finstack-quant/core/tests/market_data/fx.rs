//! Additional tests for FX providers

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::fx::SimpleFxProvider;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxProvider};
use time::Month;

fn test_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 15).unwrap()
}

#[test]
fn simple_fx_provider_new_empty() {
    let provider = SimpleFxProvider::new();

    // Empty provider should fail for any non-identity pair
    let result = provider.rate(
        Currency::USD,
        Currency::EUR,
        test_date(),
        FxConversionPolicy::CashflowDate,
    );
    assert!(result.is_err());
}

#[test]
fn simple_fx_provider_identity_rates() {
    let provider = SimpleFxProvider::new();

    // Identity should always work
    let rate = provider
        .rate(
            Currency::USD,
            Currency::USD,
            test_date(),
            FxConversionPolicy::CashflowDate,
        )
        .unwrap();
    assert_eq!(rate, 1.0);

    let rate = provider
        .rate(
            Currency::EUR,
            Currency::EUR,
            test_date(),
            FxConversionPolicy::PeriodEnd,
        )
        .unwrap();
    assert_eq!(rate, 1.0);
}

#[test]
fn simple_fx_provider_set_and_get_direct() {
    let provider = SimpleFxProvider::new();

    provider
        .set_quote(Currency::EUR, Currency::USD, 1.15)
        .expect("valid test quote");

    // Direct lookup
    assert_eq!(
        provider.get_direct(Currency::EUR, Currency::USD),
        Some(1.15)
    );

    // Opposite direction not set
    assert_eq!(provider.get_direct(Currency::USD, Currency::EUR), None);
}

#[test]
fn simple_fx_provider_reciprocal_fallback() {
    let provider = SimpleFxProvider::new();

    provider
        .set_quote(Currency::GBP, Currency::USD, 1.30)
        .expect("valid test quote");

    // Direct rate
    let rate_direct = provider
        .rate(
            Currency::GBP,
            Currency::USD,
            test_date(),
            FxConversionPolicy::CashflowDate,
        )
        .unwrap();
    assert_eq!(rate_direct, 1.30);

    // Reciprocal rate (USD -> GBP)
    let rate_recip = provider
        .rate(
            Currency::USD,
            Currency::GBP,
            test_date(),
            FxConversionPolicy::CashflowDate,
        )
        .unwrap();
    assert!((rate_recip - 1.0 / 1.30).abs() < 1e-12);
}

#[test]
fn simple_fx_provider_set_quotes_bulk() {
    let provider = SimpleFxProvider::new();

    provider
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.10),
            (Currency::GBP, Currency::USD, 1.25),
            (Currency::JPY, Currency::USD, 0.0091),
        ])
        .expect("valid test quotes");

    // All should be retrievable
    assert_eq!(
        provider.get_direct(Currency::EUR, Currency::USD),
        Some(1.10)
    );
    assert_eq!(
        provider.get_direct(Currency::GBP, Currency::USD),
        Some(1.25)
    );
    assert_eq!(
        provider.get_direct(Currency::JPY, Currency::USD),
        Some(0.0091)
    );
}

#[test]
fn simple_fx_provider_set_quotes_is_atomic() {
    let provider = SimpleFxProvider::new();
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.10)
        .unwrap();

    assert!(provider
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.20),
            (Currency::GBP, Currency::USD, f64::NAN),
        ])
        .is_err());
    assert_eq!(
        provider.get_direct(Currency::EUR, Currency::USD),
        Some(1.10)
    );
    assert_eq!(provider.get_direct(Currency::GBP, Currency::USD), None);
}

#[test]
fn simple_fx_provider_update_existing_quote() {
    let provider = SimpleFxProvider::new();

    // Set initial quote
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.10)
        .expect("valid test quote");
    assert_eq!(
        provider.get_direct(Currency::EUR, Currency::USD),
        Some(1.10)
    );

    // Update quote
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.15)
        .expect("valid test quote");
    assert_eq!(
        provider.get_direct(Currency::EUR, Currency::USD),
        Some(1.15)
    );
}

#[test]
fn simple_fx_provider_respects_policies() {
    let provider = SimpleFxProvider::new();
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.12)
        .expect("valid test quote");

    // Provider should work with all policy types
    for policy in [
        FxConversionPolicy::CashflowDate,
        FxConversionPolicy::PeriodEnd,
        FxConversionPolicy::PeriodAverage,
        FxConversionPolicy::Custom,
    ] {
        let rate = provider
            .rate(Currency::EUR, Currency::USD, test_date(), policy)
            .unwrap();
        assert_eq!(rate, 1.12);
    }
}

#[test]
fn simple_fx_provider_zero_rate_is_rejected() {
    let provider = SimpleFxProvider::new();

    let result = provider.set_quote(Currency::EUR, Currency::USD, 0.0);
    assert!(result.is_err());
}

#[test]
fn simple_fx_provider_multiple_pairs() {
    let provider = SimpleFxProvider::new();

    // Set up multiple currency pairs
    provider
        .set_quotes(&[
            (Currency::USD, Currency::EUR, 0.92),
            (Currency::USD, Currency::GBP, 0.79),
            (Currency::USD, Currency::JPY, 110.0),
            (Currency::EUR, Currency::GBP, 0.86),
        ])
        .expect("valid test quotes");

    // All direct rates should work
    assert_eq!(
        provider
            .rate(
                Currency::USD,
                Currency::EUR,
                test_date(),
                FxConversionPolicy::CashflowDate
            )
            .unwrap(),
        0.92
    );
    assert_eq!(
        provider
            .rate(
                Currency::USD,
                Currency::JPY,
                test_date(),
                FxConversionPolicy::CashflowDate
            )
            .unwrap(),
        110.0
    );

    // Reciprocals should work
    let eur_usd = provider
        .rate(
            Currency::EUR,
            Currency::USD,
            test_date(),
            FxConversionPolicy::CashflowDate,
        )
        .unwrap();
    assert!((eur_usd - 1.0 / 0.92).abs() < 1e-12);
}

#[test]
fn simple_fx_provider_error_messages() {
    let provider = SimpleFxProvider::new();

    let result = provider.rate(
        Currency::EUR,
        Currency::GBP,
        test_date(),
        FxConversionPolicy::CashflowDate,
    );

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = format!("{}", err);
    assert!(err_msg.contains("FX:EUR->GBP") || err_msg.contains("not found"));
}

#[test]
fn simple_fx_provider_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let provider = Arc::new(SimpleFxProvider::new());

    // Set initial quote in main thread
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.10)
        .expect("valid test quote");

    // Clone for thread
    let provider_clone = Arc::clone(&provider);

    // Update from another thread
    let handle = thread::spawn(move || {
        provider_clone
            .set_quote(Currency::GBP, Currency::USD, 1.25)
            .expect("valid test quote");
    });

    handle.join().unwrap();

    // Both quotes should be available
    assert_eq!(
        provider.get_direct(Currency::EUR, Currency::USD),
        Some(1.10)
    );
    assert_eq!(
        provider.get_direct(Currency::GBP, Currency::USD),
        Some(1.25)
    );
}

#[test]
fn simple_fx_provider_many_currencies() {
    let provider = SimpleFxProvider::new();

    // Test with a wide variety of currencies
    let pairs = [
        (Currency::USD, Currency::EUR, 0.92),
        (Currency::USD, Currency::GBP, 0.79),
        (Currency::USD, Currency::JPY, 110.0),
        (Currency::USD, Currency::CHF, 0.88),
        (Currency::USD, Currency::CAD, 1.35),
        (Currency::USD, Currency::AUD, 1.45),
    ];

    provider.set_quotes(&pairs).expect("valid test quotes");

    // Verify all are stored correctly
    for (from, to, expected) in pairs {
        let rate = provider.get_direct(from, to);
        assert_eq!(rate, Some(expected));
    }
}

// ===================================================================
// Edge Case Tests (Market Standards Review)
// ===================================================================

#[test]
fn simple_fx_provider_very_small_rate() {
    let provider = SimpleFxProvider::new();
    provider
        .set_quote(Currency::EUR, Currency::USD, 1e-10)
        .expect("valid test quote");

    let direct = provider
        .rate(
            Currency::EUR,
            Currency::USD,
            test_date(),
            FxConversionPolicy::CashflowDate,
        )
        .unwrap();
    assert_eq!(direct, 1e-10);

    let recip = provider
        .rate(
            Currency::USD,
            Currency::EUR,
            test_date(),
            FxConversionPolicy::CashflowDate,
        )
        .unwrap();
    assert!(
        recip.is_finite() && recip > 1e9,
        "Reciprocal of small rate should be large but finite: {}",
        recip
    );
}
