//! Market-standard FRA validation tests.
//!
//! Validates FRA implementation against market conventions:
//! - Settlement adjustment formula: PV = N * DF * tau * (F - K) / (1 + F*tau)
//! - Day count conventions (ACT/360 for USD, ACT/365 for GBP)
//! - Sign conventions (receive fixed vs pay fixed)
//! - Forward rate calculation matching curve interpolation

use crate::fra::common::*;
use finstack_core::currency::Currency;
use finstack_core::dates::DayCount;
use finstack_core::prelude::MarketContext;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;

#[test]
fn test_settlement_adjustment_formula() {
    // Validate the market-standard settlement adjustment: 1 / (1 + F * tau)
    // Reference: Hull, "Options, Futures and Other Derivatives"

    let market = standard_market(); // 5% flat

    let (fixing, start, end) = standard_fra_dates();

    // Create FRA with known parameters
    let fra = TestFraBuilder::new()
        .dates(fixing, start, end)
        .fixed_rate(0.06) // 1% above market
        .notional(1_000_000.0, Currency::USD)
        .day_count(DayCount::Act360)
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap().amount();

    // Compute values from the actual curves/conventions (not hardcoded approximations)
    let forward_rate = 0.05; // market rate from flat forward curve
    let fixed_rate = 0.06;
    let rate_diff = forward_rate - fixed_rate; // -0.01

    // Compute tau from actual dates and day count convention (ACT/360)
    // start = 2024-04-01, end = 2024-07-01 = 91 days
    let days = (end - start).whole_days() as f64;
    let tau = days / 360.0; // ACT/360

    // Compute DF from the discount curve
    // Time to start date from base date (2024-01-01 to 2024-04-01)
    let time_to_start = (start - BASE_DATE).whole_days() as f64 / 365.0;
    let df = (-0.05 * time_to_start).exp(); // Using flat 5% rate

    // PV = N * DF * tau * (F - K) / (1 + F * tau)
    let settlement_adj = 1.0 / (1.0 + forward_rate * tau);
    let base_pv = 1_000_000.0 * df * tau * rate_diff * settlement_adj;
    // pay_fixed = true (receive fixed) → FRA receives fixed, pays floating
    // When receiving above market, should have positive PV
    let expected_pv = -base_pv;

    // With properly computed values, we should be within 0.5% (was 15% with approximations)
    let diff_pct = ((pv - expected_pv) / expected_pv.abs().max(1e-10)).abs();
    assert!(
        diff_pct < 0.005,
        "PV should match settlement adjustment formula within 0.5%: \
        pv={:.2}, expected={:.2}, diff={:.4}%, tau={:.6}, df={:.6}",
        pv,
        expected_pv,
        diff_pct * 100.0,
        tau,
        df
    );
}

#[test]
fn test_sign_convention_receive_fixed_positive_when_above_market() {
    // Standard convention: receive fixed above market → positive PV
    let market = standard_market(); // 5%

    let fra = TestFraBuilder::new()
        .fixed_rate(0.06) // receive 6% (above market)
        .receive_fixed(true) // true = receive fixed
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_positive(
        pv.amount(),
        "Receive fixed above market should have positive PV (standard convention)",
    );
}

#[test]
fn test_sign_convention_pay_fixed_negative_when_above_market() {
    // Standard convention: pay fixed above market → negative PV
    let market = standard_market(); // 5%

    let fra = TestFraBuilder::new()
        .fixed_rate(0.06) // pay 6%
        .receive_fixed(false) // false = pay fixed
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_negative(
        pv.amount(),
        "Pay fixed above market should have negative PV (standard convention)",
    );
}

#[test]
fn test_forward_rate_matches_par_rate() {
    // Par rate should equal the forward rate from the curve
    let market = standard_market(); // 5% flat

    let fra = create_standard_fra();

    let result = fra
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::ParRate],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_rate = *result.measures.get("par_rate").unwrap();

    assert_approx_equal(
        par_rate,
        0.05,
        0.001,
        "Par rate should match forward curve rate (market standard)",
    );
}

#[test]
fn test_dv01_sign_convention() {
    // Standard convention: receive fixed → negative DV01, pay fixed → positive DV01
    let market = standard_market();

    let receive_fixed = TestFraBuilder::new().receive_fixed(true).build(); // true = receive fixed
    let pay_fixed = TestFraBuilder::new().receive_fixed(false).build(); // false = pay fixed

    let result_receive = receive_fixed
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01_receive = *result_receive.measures.get("dv01").unwrap();

    let result_pay = pay_fixed
        .price_with_metrics(
            &market,
            BASE_DATE,
            &[MetricId::Dv01],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01_pay = *result_pay.measures.get("dv01").unwrap();

    assert_negative(
        dv01_receive,
        "Receive fixed should have negative DV01 (market convention)",
    );
    assert_positive(
        dv01_pay,
        "Pay fixed should have positive DV01 (market convention)",
    );
}

#[test]
fn test_zero_curve_zero_pv() {
    let disc = build_flat_discount_curve(0.0, BASE_DATE, "USD_OIS");
    let fwd = build_flat_forward_curve(0.0, BASE_DATE, "USD_LIBOR_3M");
    let market = MarketContext::new().insert(disc).insert(fwd);

    let fra = TestFraBuilder::new().fixed_rate(0.0).build();
    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_near_zero(pv.amount(), 1.0, "Zero curve should imply near-zero PV");
}

#[test]
fn test_pv_independent_of_notional_currency() {
    // PV calculation should be independent of currency (same rate environment)
    let market = standard_market();

    let fra_usd = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::USD)
        .fixed_rate(0.06)
        .build();

    let fra_eur = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::EUR)
        .fixed_rate(0.06)
        .build();

    let pv_usd = fra_usd.value(&market, BASE_DATE).unwrap().amount();
    let pv_eur = fra_eur.value(&market, BASE_DATE).unwrap().amount();

    // Same calculation, same result (just different currency labels)
    assert_approx_equal(
        pv_usd,
        pv_eur,
        0.01,
        "PV calculation should be independent of currency label",
    );
}
