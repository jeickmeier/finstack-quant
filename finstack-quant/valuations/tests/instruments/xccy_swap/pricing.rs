use super::fixtures::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::StubKind;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::xccy_swap::{
    LegSide, NotionalExchange, XccySwap,
};
use rust_decimal::Decimal;

#[test]
fn requires_fx_matrix_when_reporting_currency_differs() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 1, 2);
    let swap = XccySwap::new(
        "XCCY-TEST",
        leg_usd_receive(base, maturity),
        leg_eur_pay(base, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::None);

    let market = market_without_fx();
    let err = swap.value(&market, base).unwrap_err();
    assert!(err.to_string().to_ascii_lowercase().contains("fx_matrix"));
}

#[test]
fn prices_with_fx_and_curves() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 1, 2);
    let swap = XccySwap::new(
        "XCCY-TEST",
        leg_usd_receive(base, maturity),
        leg_eur_pay(base, maturity),
        Currency::USD,
    );

    let market = market_with_fx();
    let pv = swap.value(&market, base).unwrap();
    assert_eq!(pv.currency(), Currency::USD);
    assert!(pv.amount().is_finite());
}

#[test]
fn notional_exchange_changes_pv() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 1, 2);
    let swap_none = XccySwap::new(
        "XCCY-TEST-NONE",
        leg_usd_receive(base, maturity),
        leg_eur_pay(base, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::None);

    let swap_ex = XccySwap::new(
        "XCCY-TEST-EX",
        leg_usd_receive(base, maturity),
        leg_eur_pay(base, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::InitialAndFinal);

    let market = market_with_fx();
    let pv_none = swap_none.value(&market, base).unwrap().amount();
    let pv_ex = swap_ex.value(&market, base).unwrap().amount();
    assert!((pv_none - pv_ex).abs() > 1e-10);
}

// =============================================================================
// Stub Period Tests
// =============================================================================

#[test]
fn short_front_stub_prices_correctly() {
    // Start date is not on a regular roll date, creating a short front stub
    let base = d(2025, 1, 15); // Mid-month start
    let maturity = d(2026, 1, 2); // Regular quarterly end

    let swap = XccySwap::new(
        "XCCY-SHORT-STUB",
        leg_usd_receive(base, maturity),
        leg_eur_pay(base, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::None);

    let market = market_with_fx();
    let pv = swap.value(&market, base).unwrap();

    assert!(pv.amount().is_finite(), "PV should be finite with stub");
    // Stub handling should not produce zero PV
    assert!(pv.amount().abs() > 1e-10, "PV should be non-zero with stub");
}

#[test]
fn long_back_stub_prices_correctly() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 2, 15); // Creates a long back stub

    let mut usd_leg = leg_usd_receive(base, maturity);
    usd_leg.stub = StubKind::LongBack;
    let mut eur_leg = leg_eur_pay(base, maturity);
    eur_leg.stub = StubKind::LongBack;

    let swap = XccySwap::new("XCCY-LONG-STUB", usd_leg, eur_leg, Currency::USD)
        .with_notional_exchange(NotionalExchange::None);

    let market = market_with_fx();
    let pv = swap.value(&market, base).unwrap();

    assert!(
        pv.amount().is_finite(),
        "PV should be finite with long back stub"
    );
}

// =============================================================================
// Payment Lag Tests
// =============================================================================

#[test]
fn payment_lag_affects_pv() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 1, 2);

    let leg_no_lag = leg_usd_receive(base, maturity);
    let mut leg_with_lag = leg_usd_receive(base, maturity);
    leg_with_lag.payment_lag_days = 2;
    leg_with_lag.allow_calendar_fallback = true;

    let swap_no_lag = XccySwap::new(
        "XCCY-NO-LAG",
        leg_no_lag,
        leg_eur_pay(base, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::None);

    let mut leg_eur_lag = leg_eur_pay(base, maturity);
    leg_eur_lag.allow_calendar_fallback = true;

    let swap_with_lag = XccySwap::new("XCCY-WITH-LAG", leg_with_lag, leg_eur_lag, Currency::USD)
        .with_notional_exchange(NotionalExchange::None);

    let market = market_with_fx();
    let pv_no_lag = swap_no_lag.value(&market, base).unwrap().amount();
    let pv_with_lag = swap_with_lag.value(&market, base).unwrap().amount();

    // Payment lag should affect discounting, thus change PV
    assert!(
        (pv_no_lag - pv_with_lag).abs() > 1e-6,
        "Payment lag should affect PV: no_lag={}, with_lag={}",
        pv_no_lag,
        pv_with_lag
    );
}

// =============================================================================
// Near-Expiry Tests
// =============================================================================

#[test]
fn near_expiry_swap_prices_correctly() {
    // Swap that expires tomorrow (T+1)
    let base = d(2025, 1, 2);
    let start = d(2025, 1, 2);
    let maturity = d(2025, 1, 3); // 1 day maturity

    let mut leg_usd = leg_usd_receive(start, maturity);
    leg_usd.allow_calendar_fallback = true;
    let mut leg_eur = leg_eur_pay(start, maturity);
    leg_eur.allow_calendar_fallback = true;

    let swap = XccySwap::new("XCCY-NEAR-EXPIRY", leg_usd, leg_eur, Currency::USD)
        .with_notional_exchange(NotionalExchange::InitialAndFinal);

    let market = market_with_fx();
    let pv = swap.value(&market, base).unwrap();

    assert!(pv.amount().is_finite(), "Near-expiry PV should be finite");
}

#[test]
fn expired_swap_returns_zero_pv() {
    // Valuation date is after maturity
    let base = d(2025, 1, 2);
    let start = d(2024, 1, 2);
    let maturity = d(2025, 1, 1); // Already expired

    let mut leg_usd = leg_usd_receive(start, maturity);
    leg_usd.allow_calendar_fallback = true;
    let mut leg_eur = leg_eur_pay(start, maturity);
    leg_eur.allow_calendar_fallback = true;

    let swap = XccySwap::new("XCCY-EXPIRED", leg_usd, leg_eur, Currency::USD)
        .with_notional_exchange(NotionalExchange::None);

    let market = market_with_fx();
    let pv = swap.value(&market, base).unwrap();

    // All cashflows are in the past, PV should be zero (or very close)
    assert!(
        pv.amount().abs() < 1e-10,
        "Expired swap should have zero PV, got {}",
        pv.amount()
    );
}

// =============================================================================
// Validation Tests
// =============================================================================

#[test]
fn rejects_negative_notional() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 1, 2);

    let mut leg_usd = leg_usd_receive(base, maturity);
    leg_usd.notional = Money::new(-1_000_000.0, Currency::USD);

    let swap = XccySwap::new(
        "XCCY-NEG-NOTIONAL",
        leg_usd,
        leg_eur_pay(base, maturity),
        Currency::USD,
    );

    let market = market_with_fx();
    let err = swap.value(&market, base).unwrap_err();
    assert!(
        err.to_string().contains("positive"),
        "Should reject negative notional: {}",
        err
    );
}

#[test]
fn rejects_zero_notional() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 1, 2);

    let mut leg_usd = leg_usd_receive(base, maturity);
    leg_usd.notional = Money::new(0.0, Currency::USD);

    let swap = XccySwap::new(
        "XCCY-ZERO-NOTIONAL",
        leg_usd,
        leg_eur_pay(base, maturity),
        Currency::USD,
    );

    let market = market_with_fx();
    let err = swap.value(&market, base).unwrap_err();
    assert!(
        err.to_string().contains("positive"),
        "Should reject zero notional: {}",
        err
    );
}

// NOTE: NaN/infinite spread values are now rejected at the binding layer (Python/WASM),
// since BasisSwapLeg::spread_bp is `Decimal` which is always finite by construction.
// The test below verifies that extreme but valid spreads are accepted.
#[test]
fn accepts_extreme_but_finite_spread() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 1, 2);

    let mut leg_usd = leg_usd_receive(base, maturity);
    leg_usd.spread_bp = Decimal::from(100); // 100bp - large but valid

    let swap = XccySwap::new(
        "XCCY-EXTREME-SPREAD",
        leg_usd,
        leg_eur_pay(base, maturity),
        Currency::USD,
    );

    let market = market_with_fx();
    // Should succeed - extreme spreads are allowed at the core level
    let result = swap.value(&market, base);
    assert!(result.is_ok(), "Should accept valid extreme spread");
}

// =============================================================================
// Spread-Only Pricing Tests
// =============================================================================

#[test]
fn spread_affects_pv() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 1, 2);

    let leg_no_spread = leg_usd_receive(base, maturity);
    let mut leg_with_spread = leg_usd_receive(base, maturity);
    leg_with_spread.spread_bp = Decimal::from(50); // 50bp spread

    let swap_no_spread = XccySwap::new(
        "XCCY-NO-SPREAD",
        leg_no_spread,
        leg_eur_pay(base, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::None);

    let swap_with_spread = XccySwap::new(
        "XCCY-WITH-SPREAD",
        leg_with_spread,
        leg_eur_pay(base, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::None);

    let market = market_with_fx();
    let pv_no_spread = swap_no_spread.value(&market, base).unwrap().amount();
    let pv_with_spread = swap_with_spread.value(&market, base).unwrap().amount();

    // Spread should increase received leg value
    assert!(
        pv_with_spread > pv_no_spread,
        "Positive spread on receive leg should increase PV: no_spread={}, with_spread={}",
        pv_no_spread,
        pv_with_spread
    );
}

#[test]
fn negative_spread_decreases_pv() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 1, 2);

    let leg_no_spread = leg_usd_receive(base, maturity);
    let mut leg_negative_spread = leg_usd_receive(base, maturity);
    leg_negative_spread.spread_bp = Decimal::from(-50); // -50bp spread

    let swap_no_spread = XccySwap::new(
        "XCCY-NO-SPREAD",
        leg_no_spread,
        leg_eur_pay(base, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::None);

    let swap_negative = XccySwap::new(
        "XCCY-NEG-SPREAD",
        leg_negative_spread,
        leg_eur_pay(base, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::None);

    let market = market_with_fx();
    let pv_no_spread = swap_no_spread.value(&market, base).unwrap().amount();
    let pv_negative = swap_negative.value(&market, base).unwrap().amount();

    // Negative spread on receive leg should decrease PV
    assert!(
        pv_negative < pv_no_spread,
        "Negative spread on receive leg should decrease PV: no_spread={}, negative={}",
        pv_no_spread,
        pv_negative
    );
}

// =============================================================================
// Multi-Period Schedule Tests
// =============================================================================

#[test]
fn long_dated_swap_prices_with_many_periods() {
    // 10-year swap with quarterly payments = 40 periods
    let base = d(2025, 1, 2);
    let maturity = d(2035, 1, 2);

    let mut leg_usd = leg_usd_receive(base, maturity);
    leg_usd.allow_calendar_fallback = true;
    let mut leg_eur = leg_eur_pay(base, maturity);
    leg_eur.allow_calendar_fallback = true;

    let swap = XccySwap::new("XCCY-10Y", leg_usd, leg_eur, Currency::USD)
        .with_notional_exchange(NotionalExchange::InitialAndFinal);

    let market = market_with_extended_curves();
    let pv = swap.value(&market, base).unwrap();

    assert!(
        pv.amount().is_finite(),
        "Long-dated swap PV should be finite"
    );
}

// =============================================================================
// Final-Only Exchange Test
// =============================================================================

#[test]
fn final_only_exchange_differs_from_initial_and_final() {
    // Start date is in the future so we can observe the initial exchange value
    let as_of = d(2025, 1, 2);
    let start_date = d(2025, 1, 9); // T+5 forward start
    let maturity = d(2026, 1, 9);

    let swap_final = XccySwap::new(
        "XCCY-FINAL",
        leg_usd_receive(start_date, maturity),
        leg_eur_pay(start_date, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::Final);

    let swap_both = XccySwap::new(
        "XCCY-BOTH",
        leg_usd_receive(start_date, maturity),
        leg_eur_pay(start_date, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::InitialAndFinal);

    let market = market_with_fx();
    let pv_final = swap_final.value(&market, as_of).unwrap().amount();
    let pv_both = swap_both.value(&market, as_of).unwrap().amount();

    // Initial exchange affects PV when start_date > as_of
    // With Initial+Final, we add the initial exchange cashflows (pay one currency, receive another)
    // The USD leg receives $1M and EUR leg pays €900k at start (worth $990k at 1.10 spot)
    // Initial exchange net effect: +$1M - $990k = +$10k for USD receiver
    // But this is a forward start, so it's discounted
    assert!(
        (pv_final - pv_both).abs() > 1e-6,
        "Final-only vs Initial+Final should differ: final={}, both={}",
        pv_final,
        pv_both
    );
}

// =============================================================================
// Leg Side Sign Tests
// =============================================================================

#[test]
fn receive_vs_pay_legs_have_opposite_signs() {
    let base = d(2025, 1, 2);
    let maturity = d(2026, 1, 2);

    // USD receive / EUR pay
    let swap_usd_receive = XccySwap::new(
        "XCCY-USD-RECEIVE",
        leg_usd_receive(base, maturity),
        leg_eur_pay(base, maturity),
        Currency::USD,
    )
    .with_notional_exchange(NotionalExchange::None);

    // USD pay / EUR receive (flip the sides)
    let mut leg_usd_pay = leg_usd_receive(base, maturity);
    leg_usd_pay.side = LegSide::Pay;
    let mut leg_eur_receive = leg_eur_pay(base, maturity);
    leg_eur_receive.side = LegSide::Receive;

    let swap_usd_pay = XccySwap::new("XCCY-USD-PAY", leg_usd_pay, leg_eur_receive, Currency::USD)
        .with_notional_exchange(NotionalExchange::None);

    let market = market_with_fx();
    let pv_receive = swap_usd_receive.value(&market, base).unwrap().amount();
    let pv_pay = swap_usd_pay.value(&market, base).unwrap().amount();

    // Flipping all sides should negate the PV
    assert!(
        (pv_receive + pv_pay).abs() < 1e-6,
        "Flipping sides should negate PV: receive={}, pay={}, sum={}",
        pv_receive,
        pv_pay,
        pv_receive + pv_pay
    );
}
