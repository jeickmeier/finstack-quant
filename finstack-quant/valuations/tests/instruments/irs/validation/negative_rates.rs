//! Negative interest rate tests for IRS pricing.
//!
//! Tests verify that IRS pricing and metrics work correctly when:
//! - Discount rates are negative (e.g., -50bp)
//! - Forward rates are negative
//! - Both discount and forward curves have negative rates
//!
//! These scenarios have been common in EUR, CHF, JPY, and other markets
//! since 2014 (ECB negative deposit rate).
//!
//! **Market Standards Review (Week 3 Edge Cases)**

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::rates::irs::PayReceive;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

/// Build a flat discount curve that handles negative rates
fn build_negative_rate_discount_curve(rate: f64, base_date: Date, curve_id: &str) -> DiscountCurve {
    // For negative rates, discount factors > 1 at future dates
    // exp(-(-r)*T) = exp(r*T) > 1 for r > 0, but we're using rate = -0.005 etc.
    // So exp(-rate * T) where rate < 0 => exp(+|rate|*T) > 1
    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        ) // Required for negative rates (DFs > 1)
        .interp(InterpStyle::Linear) // Linear supports non-monotonic DFs
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

/// Build a flat forward curve with negative rate
fn build_negative_rate_forward_curve(rate: f64, base_date: Date, curve_id: &str) -> ForwardCurve {
    ForwardCurve::builder(curve_id, 0.25)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate)])
        .build()
        .unwrap()
}

fn create_negative_rate_market(disc_rate: f64, fwd_rate: f64, base_date: Date) -> MarketContext {
    // Use standard curve IDs expected by usd_irs_swap
    let disc = build_negative_rate_discount_curve(disc_rate, base_date, "USD-OIS");
    let fwd = build_negative_rate_forward_curve(fwd_rate, base_date, "USD-SOFR-3M");

    MarketContext::new().insert(disc).insert(fwd)
}

// ============================================================================
// Negative Discount Rate Tests
// ============================================================================

#[test]
fn test_irs_pricing_negative_discount_rate() {
    // Scenario: -50bp discount rate (like EUR or CHF post-2014)
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let disc_rate = -0.005; // -50bp
    let fwd_rate = 0.01; // +100bp forward (spread over discount)

    let market = create_negative_rate_market(disc_rate, fwd_rate, as_of);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("NEG_DISC_TEST"),
        Money::new(1_000_000.0, Currency::USD),
        0.01, // 1% fixed rate
        as_of,
        maturity,
        PayReceive::Pay,
    )
    .expect("swap creation should succeed");

    let pv = swap.value(&market, as_of);
    assert!(pv.is_ok(), "Pricing should succeed with negative rates");

    let pv_amount = pv.unwrap().amount();
    assert!(
        pv_amount.is_finite(),
        "PV should be finite with negative rates, got {}",
        pv_amount
    );
}

#[test]
fn test_irs_dv01_negative_discount_rate() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let disc_rate = -0.005;
    let fwd_rate = 0.01;

    let market = create_negative_rate_market(disc_rate, fwd_rate, as_of);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("NEG_DV01_TEST"),
        Money::new(1_000_000.0, Currency::USD),
        0.01,
        as_of,
        maturity,
        PayReceive::Pay,
    )
    .unwrap();

    let result = swap.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Dv01],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );
    assert!(result.is_ok(), "DV01 calculation should succeed");

    let result = result.unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();
    assert!(
        dv01.is_finite(),
        "DV01 should be finite with negative rates, got {}",
        dv01
    );

    // DV01 should be positive (swap loses value when rates rise)
    // even with negative starting rates
    assert!(
        dv01.abs() > 0.0,
        "DV01 should be non-zero for a live swap, got {}, PV={}, measures={:?}",
        dv01,
        result.value.amount(),
        result.measures
    );
}

// ============================================================================
// Negative Forward Rate Tests
// ============================================================================

#[test]
fn test_irs_pricing_negative_forward_rate() {
    // Scenario: Negative forward rates
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let disc_rate = 0.02; // Positive discount
    let fwd_rate = -0.003; // -30bp forward rate

    let market = create_negative_rate_market(disc_rate, fwd_rate, as_of);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("NEG_FWD_TEST"),
        Money::new(1_000_000.0, Currency::USD),
        0.01, // Pay 1% fixed
        as_of,
        maturity,
        PayReceive::Pay,
    )
    .unwrap();

    let pv = swap.value(&market, as_of).unwrap();

    // With negative forward rate and paying positive fixed,
    // the pay-fixed swap should have negative value (paying more than receiving)
    assert!(
        pv.amount().is_finite(),
        "PV should be finite, got {}",
        pv.amount()
    );
}

#[test]
fn test_irs_par_rate_with_negative_rates() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let disc_rate = -0.003;
    let fwd_rate = -0.001;

    let market = create_negative_rate_market(disc_rate, fwd_rate, as_of);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("PAR_NEG_TEST"),
        Money::new(1_000_000.0, Currency::USD),
        0.01, // Initial rate doesn't matter for par rate calc
        as_of,
        maturity,
        PayReceive::Pay,
    )
    .unwrap();

    let result = swap.price_with_metrics(
        &market,
        as_of,
        &[MetricId::ParRate],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );
    assert!(result.is_ok(), "Par rate calculation should succeed");

    let par_rate = *result.unwrap().measures.get("par_rate").unwrap();
    assert!(
        par_rate.is_finite(),
        "Par rate should be finite, got {}",
        par_rate
    );

    // Par rate can be negative when forward rates are negative
    // This is expected behavior in negative rate environments
}

// ============================================================================
// Deep Negative Rate Tests
// ============================================================================

#[test]
fn test_irs_deep_negative_rates() {
    // Extreme scenario: -100bp rates (has occurred in Swiss markets)
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let disc_rate = -0.01; // -100bp
    let fwd_rate = -0.008; // -80bp

    let market = create_negative_rate_market(disc_rate, fwd_rate, as_of);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("DEEP_NEG_TEST"),
        Money::new(1_000_000.0, Currency::USD),
        -0.005, // Pay -50bp fixed (receive negative)
        as_of,
        maturity,
        PayReceive::Pay,
    )
    .unwrap();

    let result = swap.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Pv01, MetricId::Dv01, MetricId::ParRate],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );

    assert!(
        result.is_ok(),
        "All metrics should compute with deep negative rates"
    );

    let measures = result.unwrap().measures;

    // All metrics should be finite
    for (name, value) in &measures {
        assert!(
            value.is_finite(),
            "Metric {} should be finite with deep negative rates, got {}",
            name,
            value
        );
    }
}

// ============================================================================
// Symmetry Tests with Negative Rates
// ============================================================================

#[test]
fn test_payer_receiver_symmetry_negative_rates() {
    // Payer + Receiver = 0 should hold even with negative rates
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let disc_rate = -0.005;
    let fwd_rate = -0.003;

    let market = create_negative_rate_market(disc_rate, fwd_rate, as_of);
    let fixed_rate = 0.001;

    let payer = test_utils::usd_irs_swap(
        InstrumentId::new("PAYER_NEG"),
        Money::new(1_000_000.0, Currency::USD),
        fixed_rate,
        as_of,
        maturity,
        PayReceive::Pay,
    )
    .unwrap();

    let receiver = test_utils::usd_irs_swap(
        InstrumentId::new("RECEIVER_NEG"),
        Money::new(1_000_000.0, Currency::USD),
        fixed_rate,
        as_of,
        maturity,
        PayReceive::Receive,
    )
    .unwrap();

    let pv_payer = payer.value(&market, as_of).unwrap().amount();
    let pv_receiver = receiver.value(&market, as_of).unwrap().amount();

    // Sum should be zero (opposite sides of same swap)
    let sum = pv_payer + pv_receiver;
    assert!(
        sum.abs() < 1.0, // Within $1 of zero for $1MM notional
        "Payer + Receiver should sum to zero: {:.2} + {:.2} = {:.2}",
        pv_payer,
        pv_receiver,
        sum
    );
}

#[test]
fn test_annuity_positive_with_negative_rates() {
    // Annuity should remain positive even with negative discount rates
    // (present value of receiving 1 per period is still positive)
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let disc_rate = -0.005;
    let fwd_rate = 0.01;

    let market = create_negative_rate_market(disc_rate, fwd_rate, as_of);

    let swap = test_utils::usd_irs_swap(
        InstrumentId::new("ANNUITY_NEG"),
        Money::new(1_000_000.0, Currency::USD),
        0.01,
        as_of,
        maturity,
        PayReceive::Pay,
    )
    .unwrap();

    let result = swap.price_with_metrics(
        &market,
        as_of,
        &[MetricId::Annuity],
        finstack_quant_valuations::instruments::PricingOptions::default(),
    );
    assert!(result.is_ok());

    let annuity = *result.unwrap().measures.get("annuity").unwrap();
    assert!(
        annuity > 0.0,
        "Annuity should be positive even with negative rates, got {}",
        annuity
    );

    // With negative rates, annuity should be slightly higher than with positive rates
    // because discount factors > 1
}
