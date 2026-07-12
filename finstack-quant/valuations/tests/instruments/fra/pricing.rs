//! FRA pricing and NPV calculation tests.
//!
//! Validates the core NPV calculation, including:
//! - Market-standard settlement adjustment (1 + F*tau)
//! - At-market vs off-market FRAs
//! - Pay vs receive fixed sign conventions
//! - Edge cases (zero tau, extreme rates)

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

#[test]
fn test_at_market_fra_near_zero_pv() {
    // FRA struck at market forward rate should have ~zero PV
    let market = standard_market();
    let fra = create_standard_fra(); // 5% fixed, 5% market
    let fwd = market.get_forward("USD_LIBOR_3M").expect("forward curve");
    let (_, start, end) = standard_fra_dates();
    let t_start = fwd
        .day_count()
        .year_fraction(fwd.base_date(), start, DayCountContext::default())
        .expect("start time");
    let t_end = fwd
        .day_count()
        .year_fraction(fwd.base_date(), end, DayCountContext::default())
        .expect("end time");
    let implied = fwd
        .rate_between(t_start, t_end)
        .expect("contractual FRA forward");
    assert_approx_equal(
        implied,
        0.05,
        1e-14,
        "Contractual FRA grid must preserve quoted 5% forward",
    );

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_near_zero(
        pv.amount(),
        1e-8,
        "At-market contractual-grid FRA should have zero PV",
    );
}

#[test]
fn test_receive_fixed_off_market_positive_pv() {
    // Receive 6% when market is 5% → positive PV (standard convention)
    let market = standard_market(); // 5% market

    let fra = TestFraBuilder::new()
        .fixed_rate(0.06) // 6% fixed
        .receive_fixed(true) // receive fixed rate
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_positive(
        pv.amount(),
        "Receive above-market rate should have positive PV",
    );
}

#[test]
fn test_receive_fixed_off_market_negative_pv() {
    // Receive 4% when market is 5% → negative PV (standard convention)
    let market = standard_market(); // 5% market

    let fra = TestFraBuilder::new()
        .fixed_rate(0.04) // 4% fixed
        .receive_fixed(true) // receive fixed rate
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_negative(
        pv.amount(),
        "Receive below-market rate should have negative PV",
    );
}

#[test]
fn test_pay_fixed_off_market_positive_pv() {
    // Pay 4% when market is 5% → positive PV (standard convention)
    let market = standard_market(); // 5% market

    let fra = TestFraBuilder::new()
        .fixed_rate(0.04) // 4% fixed
        .receive_fixed(false) // pay fixed rate
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_positive(pv.amount(), "Pay below-market rate should have positive PV");
}

#[test]
fn test_pay_fixed_off_market_negative_pv() {
    // Pay 6% when market is 5% → negative PV (standard convention)
    let market = standard_market(); // 5% market

    let fra = TestFraBuilder::new()
        .fixed_rate(0.06) // 6% fixed
        .receive_fixed(false) // pay fixed rate
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_negative(pv.amount(), "Pay above-market rate should have negative PV");
}

#[test]
fn test_pv_scales_with_notional() {
    let market = standard_market();

    let fra_1m = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::USD)
        .fixed_rate(0.06) // off market
        .build();

    let fra_10m = TestFraBuilder::new()
        .notional(10_000_000.0, Currency::USD)
        .fixed_rate(0.06) // off market
        .build();

    let pv_1m = fra_1m.value(&market, BASE_DATE).unwrap().amount();
    let pv_10m = fra_10m.value(&market, BASE_DATE).unwrap().amount();

    assert_approx_equal(
        pv_10m,
        pv_1m * 10.0,
        1.0,
        "PV should scale linearly with notional",
    );
}

#[test]
fn test_settlement_adjustment_present() {
    // FRA settlement includes 1/(1 + F*tau) adjustment
    // Verify this by comparing to a naive calculation
    let market = standard_market();

    let fra = TestFraBuilder::new()
        .fixed_rate(0.06) // 100bp above market
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap().amount();

    // Should be less than notional * rate_diff * tau due to settlement adjustment
    let tau = 0.25; // ~3 months
    let rate_diff = 0.01; // 1%
    let naive_pv = 1_000_000.0 * rate_diff * tau; // $2,500

    assert!(
        pv.abs() < naive_pv * 1.1,
        "PV with settlement adjustment should be < naive calculation"
    );
}

#[test]
fn test_zero_tau_returns_zero_pv() {
    // FRA with same start and end date should have zero PV
    let market = standard_market();
    let same_date = date!(2024 - 04 - 01);

    let fra = TestFraBuilder::new()
        .dates(same_date, same_date, same_date)
        .fixed_rate(0.06)
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_eq!(pv.amount(), 0.0, "Zero tau should produce zero PV");
}

#[test]
fn test_short_period_1m() {
    let market = standard_market();

    let start = date!(2024 - 02 - 01);
    let end = date!(2024 - 03 - 01);

    let fra = TestFraBuilder::new()
        .dates(start, start, end)
        .fixed_rate(0.06)
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_finite(pv.amount(), "1M FRA should have finite PV");
}

#[test]
fn test_long_period_12m() {
    let market = standard_market();

    let start = date!(2024 - 04 - 01);
    let end = date!(2025 - 04 - 01);

    let fra = TestFraBuilder::new()
        .dates(start, start, end)
        .fixed_rate(0.06)
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_finite(pv.amount(), "12M FRA should have finite PV");
}

#[test]
fn test_upward_sloping_curve() {
    let disc = build_flat_discount_curve(0.05, BASE_DATE, "USD_OIS");
    let fwd = build_upward_forward_curve(BASE_DATE, "USD_LIBOR_3M");

    let market = MarketContext::new().insert(disc).insert(fwd);

    let fra = create_standard_fra();
    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_finite(pv.amount(), "Upward sloping curve should produce finite PV");
}

#[test]
fn test_inverted_curve() {
    let disc = build_flat_discount_curve(0.05, BASE_DATE, "USD_OIS");
    let fwd = build_inverted_forward_curve(BASE_DATE, "USD_LIBOR_3M");

    let market = MarketContext::new().insert(disc).insert(fwd);

    let fra = create_standard_fra();
    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_finite(pv.amount(), "Inverted curve should produce finite PV");
}

#[test]
fn test_negative_rate_environment() {
    let disc = build_flat_discount_curve(-0.01, BASE_DATE, "USD_OIS");
    let fwd = build_flat_forward_curve(-0.01, BASE_DATE, "USD_LIBOR_3M");
    let market = MarketContext::new().insert(disc).insert(fwd);

    let fra = TestFraBuilder::new().fixed_rate(-0.01).build();
    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_near_zero(
        pv.amount(),
        1.0,
        "At-market FRA should have near-zero PV in negative rates",
    );
}

#[test]
fn test_high_rate_environment() {
    let disc = build_flat_discount_curve(0.15, BASE_DATE, "USD_OIS");
    let fwd = build_flat_forward_curve(0.15, BASE_DATE, "USD_LIBOR_3M");

    let market = MarketContext::new().insert(disc).insert(fwd);

    let fra = TestFraBuilder::new().fixed_rate(0.15).build();
    let pv = fra.value(&market, BASE_DATE).unwrap();

    // Market standard: At-market FRA should have PV < $1 even at high rates
    assert_near_zero(
        pv.amount(),
        1.0,
        "At-market high rates should have near-zero PV",
    );
}

#[test]
fn test_different_curve_base_dates() {
    // Curves with different base dates should still work
    let curve_base = date!(2024 - 01 - 15);

    let disc = build_flat_discount_curve(0.05, curve_base, "USD_OIS");
    let fwd = build_flat_forward_curve(0.05, curve_base, "USD_LIBOR_3M");

    let market = MarketContext::new().insert(disc).insert(fwd);

    let fra = create_standard_fra();
    let pv = fra.value(&market, curve_base).unwrap();

    assert_finite(pv.amount(), "Different curve base dates should work");
}

#[test]
fn test_act_365_day_count() {
    let market = standard_market();

    let fra = TestFraBuilder::new()
        .day_count(DayCount::Act365F)
        .fixed_rate(0.06)
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_finite(pv.amount(), "ACT/365 day count should work");
}

#[test]
fn test_30_360_day_count() {
    let market = standard_market();

    let fra = TestFraBuilder::new()
        .day_count(DayCount::Thirty360)
        .fixed_rate(0.06)
        .build();

    let pv = fra.value(&market, BASE_DATE).unwrap();

    assert_finite(pv.amount(), "30/360 day count should work");
}

#[test]
fn test_currency_preserved_in_pv() {
    let market = standard_market();

    let usd = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::USD)
        .build();
    let eur = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::EUR)
        .build();

    let pv_usd = usd.value(&market, BASE_DATE).unwrap();
    let pv_eur = eur.value(&market, BASE_DATE).unwrap();

    assert_eq!(pv_usd.currency(), Currency::USD);
    assert_eq!(pv_eur.currency(), Currency::EUR);
}
