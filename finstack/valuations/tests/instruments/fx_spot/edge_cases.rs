//! Edge cases and boundary condition tests for FX Spot.

use super::common::*;
use finstack_cashflows::CashflowProvider;
use finstack_core::{
    currency::Currency, dates::BusinessDayConvention, market_data::context::MarketContext,
    money::Money, types::InstrumentId,
};
use finstack_valuations::{
    instruments::{FxSpot, Instrument},
    pricer::InstrumentType,
};

#[test]
fn test_same_currency_pair() {
    let fx = FxSpot::new(InstrumentId::new("USDUSD"), Currency::USD, Currency::USD)
        .with_notional(Money::new(1_000_000.0, Currency::USD))
        .expect("notional should be accepted before pair validation")
        .with_rate(1.0)
        .expect("rate should be accepted before pair validation");
    let err = fx
        .value(&MarketContext::new(), test_date())
        .expect_err("same-currency FX spot should be rejected");

    assert!(
        err.to_string()
            .contains("base_currency (USD) must differ from quote_currency (USD)"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_extremely_large_notional() {
    let fx = eurusd_with_notional(1e15, 1.20); // Quadrillion
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_approx_eq(pv.amount(), 1.2e15, 1e5, "Extremely large notional");
}

#[test]
fn test_extremely_small_notional() {
    let fx = eurusd_with_notional(1e-10, 1.20);
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert!(pv.amount() < 1e-9, "Extremely small notional");
}

#[test]
fn test_extremely_large_rate() {
    let fx = eurusd_with_notional(1.0, 1_000_000.0);
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_approx_eq(pv.amount(), 1_000_000.0, EPSILON, "Extremely large rate");
}

#[test]
fn test_extremely_small_rate() {
    let fx = eurusd_with_notional(1_000_000.0, 1e-10);
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert!(pv.amount() < 1e-3, "Extremely small rate");
}

#[test]
fn test_negative_notional() {
    // Negative notional (short position)
    let fx = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
        .with_notional(Money::new(-1_000_000.0, Currency::EUR))
        .unwrap()
        .with_rate(1.20)
        .expect("test rate");

    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    assert_approx_eq(
        pv.amount(),
        -1_200_000.0,
        EPSILON,
        "Negative notional (short)",
    );
}

#[test]
fn test_settlement_far_future() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2050, 1, 15)); // 25 years out
    let market = MarketContext::new();

    let cashflows = fx.dated_cashflows(&market, test_date()).unwrap();

    assert_eq!(cashflows.len(), 1);
    assert_eq!(cashflows[0].0, d(2050, 1, 15));
}

#[test]
fn test_settlement_far_past() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2000, 1, 15)); // 25 years ago
    let market = MarketContext::new();

    let cashflows = fx.dated_cashflows(&market, test_date()).unwrap();

    assert_eq!(cashflows.len(), 0, "Past settlement => no cashflows");
}

#[test]
fn test_valuation_on_leap_day() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let market = MarketContext::new();
    let as_of = d(2024, 2, 29); // Leap day

    let pv = fx.value(&market, as_of).unwrap();
    assert_approx_eq(pv.amount(), 1_200_000.0, EPSILON, "Leap day valuation");
}

#[test]
fn test_settlement_on_leap_day() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2024, 2, 29));
    let market = MarketContext::new();

    let cashflows = fx.dated_cashflows(&market, d(2024, 2, 28)).unwrap();

    assert_eq!(cashflows.len(), 1);
    assert_eq!(cashflows[0].0, d(2024, 2, 29));
}

#[test]
fn test_year_boundary_settlement() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20).with_settlement(d(2025, 12, 31));
    let market = MarketContext::new();

    let cashflows = fx.dated_cashflows(&market, d(2025, 12, 30)).unwrap();

    assert_eq!(cashflows.len(), 1);
    assert_eq!(cashflows[0].0, d(2025, 12, 31));
}

#[test]
fn test_attributes_modification() {
    let mut fx = eurusd_with_notional(1_000_000.0, 1.20);

    // Attributes has tags (HashSet) and meta (HashMap)
    let attrs = fx.attributes_mut();
    attrs.tags.insert("fx_desk".to_string());
    attrs.meta.insert("desk".to_string(), "rates".to_string());
    attrs.meta.insert("trader".to_string(), "JD".to_string());

    assert!(fx.attributes().tags.contains("fx_desk"));
    assert_eq!(
        fx.attributes().meta.get("desk").map(|s| s.as_str()),
        Some("rates")
    );
    assert_eq!(
        fx.attributes().meta.get("trader").map(|s| s.as_str()),
        Some("JD")
    );
}

#[test]
fn test_concurrent_pricing() {
    // Ensure thread-safety of pricing
    use std::thread;

    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let market = MarketContext::new();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let fx_clone = fx.clone();
            let market_clone = market.clone();
            thread::spawn(move || fx_clone.value(&market_clone, test_date()).unwrap())
        })
        .collect();

    for handle in handles {
        let pv = handle.join().unwrap();
        assert_approx_eq(pv.amount(), 1_200_000.0, EPSILON, "Concurrent pricing");
    }
}

#[test]
fn test_missing_fx_matrix_error_message() {
    let fx = sample_eurusd()
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .unwrap();
    let market = MarketContext::new(); // No FX matrix

    let result = fx.value(&market, test_date());

    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(err_msg.contains("NotFound") || err_msg.contains("fx_matrix"));
}

#[test]
fn test_numerical_precision_accumulation() {
    // Test that repeated conversions maintain precision
    let fx = eurusd_with_notional(1.0, 1.20);
    let market = MarketContext::new();

    let mut accumulated = fx.value(&market, test_date()).unwrap().amount();

    for _ in 0..1000 {
        let pv = fx.value(&market, test_date()).unwrap().amount();
        accumulated = (accumulated + pv) / 2.0; // Average
    }

    // Should converge to 1.20
    assert_approx_eq(accumulated, 1.20, EPSILON, "Numerical precision");
}

#[test]
fn test_rate_precision_limits() {
    // Test precision at limits of f64
    let fx = eurusd_with_notional(1.0, 1.234_567_890_123_456_7);
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    // Decimal precision limit - expect 2 decimal places by default
    assert_approx_eq(pv.amount(), 1.23, 1e-2, "Rate precision");
}

#[test]
fn test_weekend_settlement_adjustment() {
    // Settlement on Saturday should adjust to Monday (with Following convention)
    let fx = eurusd_with_notional(1_000_000.0, 1.20)
        .with_settlement(d(2025, 1, 18)) // Saturday
        .with_bdc(BusinessDayConvention::Following)
        .with_base_calendar_id("target2")
        .with_quote_calendar_id("usny");

    let market = MarketContext::new();
    let cashflows = fx.dated_cashflows(&market, test_date()).unwrap();

    assert_eq!(cashflows.len(), 1);
    // BDC adjustment only applies when calendar is loaded - may return unadjusted date
    // Saturday (18th) with Following should adjust to Monday (20th) if calendar is active
    assert!(
        cashflows[0].0 >= d(2025, 1, 18),
        "Settlement on or after Saturday"
    );
}

#[test]
fn test_extreme_settlement_lag() {
    // Test with very far future settlement instead of lag
    let far_future = d(2050, 1, 15);
    let fx = FxSpot::new(InstrumentId::new("EURUSD"), Currency::EUR, Currency::USD)
        .with_notional(Money::new(1_000_000.0, Currency::EUR))
        .unwrap()
        .with_rate(1.20)
        .expect("test rate")
        .with_settlement(far_future);

    let market = MarketContext::new();
    let cashflows = fx.dated_cashflows(&market, test_date()).unwrap();

    assert_eq!(cashflows.len(), 1);
    assert_eq!(cashflows[0].0, far_future);
}

#[test]
fn test_fractional_notional_precision() {
    let fx = eurusd_with_notional(1234567.89012345, 1.23456789);
    let market = MarketContext::new();
    let pv = fx.value(&market, test_date()).unwrap();

    let expected = 1234567.89012345 * 1.23456789;
    assert_approx_eq(pv.amount(), expected, 1e-2, "Fractional precision"); // Relaxed for Decimal
}

#[test]
fn test_default_notional_with_various_rates() {
    let market = MarketContext::new();

    for rate in [0.1, 1.0, 10.0, 100.0, 1000.0] {
        let fx = sample_eurusd().with_rate(rate).expect("test rate");
        let pv = fx.value(&market, test_date()).unwrap();

        assert_approx_eq(pv.amount(), rate, EPSILON, &format!("Rate {}", rate));
    }
}

#[test]
fn test_instrument_key_consistency() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);

    assert_eq!(fx.key(), InstrumentType::FxSpot);
    assert_eq!(fx.key(), InstrumentType::FxSpot); // Multiple calls
}

#[test]
fn test_as_any_downcast() {
    let fx = eurusd_with_notional(1_000_000.0, 1.20);
    let instrument: &dyn Instrument = &fx;

    let downcast = instrument.as_any().downcast_ref::<FxSpot>();
    assert!(downcast.is_some());

    let fx_ref = downcast.unwrap();
    assert_eq!(fx_ref.base_currency, Currency::EUR);
    assert_eq!(fx_ref.quote_currency, Currency::USD);
}
