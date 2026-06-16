//! FRA cashflow schedule generation tests.
//!
//! FRAs have a simple cashflow structure: settlement occurs at the start
//! of the accrual period (standard FRA convention). These tests validate
//! the CashflowProvider implementation.

use super::common::*;
use finstack_quant_cashflows::CashflowProvider;
use time::macros::date;

#[test]
fn test_fra_single_cashflow() {
    let market = standard_market();
    let fra = create_standard_fra();

    let flows = fra.dated_cashflows(&market, BASE_DATE).unwrap();

    // FRA should have exactly one cashflow (settlement at start)
    assert_eq!(flows.len(), 1, "FRA should have one cashflow");
}

#[test]
fn test_cashflow_date_is_start_date() {
    let market = standard_market();
    let fra = create_standard_fra();

    let flows = fra.dated_cashflows(&market, BASE_DATE).unwrap();
    let (date, _) = flows[0];

    assert_eq!(date, fra.start_date, "Cashflow should settle at start date");
}

#[test]
fn test_cashflow_amount_matches_npv() {
    let market = standard_market();
    let fra = TestFraBuilder::new().fixed_rate(0.06).build();

    let npv = fra.npv_raw(&market, BASE_DATE).unwrap();
    let flows = fra.dated_cashflows(&market, BASE_DATE).unwrap();
    let (_, amount) = flows[0];

    let disc = market.get_discount(fra.discount_curve_id.as_str()).unwrap();
    let df = disc
        .df_between_dates(BASE_DATE, fra.start_date)
        .expect("discount factor should exist");

    assert_approx_equal(
        amount.amount() * df,
        npv,
        5e-3,
        "Discounted cashflow amount should equal NPV",
    );
}

#[test]
fn test_no_cashflows_if_settled() {
    // If valuation date is after settlement, no future cashflows
    let market = standard_market();
    let fra = create_standard_fra();

    let after_settlement = date!(2024 - 07 - 01); // after start_date
    let flows = fra.dated_cashflows(&market, after_settlement).unwrap();

    assert_eq!(flows.len(), 0, "No cashflows should exist after settlement");
}

#[test]
fn test_no_cashflows_on_settlement_date() {
    let market = standard_market();
    let fra = create_standard_fra();

    let on_settlement = fra.start_date;
    let flows = fra.dated_cashflows(&market, on_settlement).unwrap();

    assert_eq!(flows.len(), 0, "No cashflows on settlement date itself");
}

#[test]
fn test_cashflows_before_settlement() {
    let market = standard_market();
    let fra = create_standard_fra();

    let before_settlement = date!(2024 - 03 - 01);
    let flows = fra.dated_cashflows(&market, before_settlement).unwrap();

    assert_eq!(flows.len(), 1, "Should have cashflow before settlement");
}

#[test]
fn test_positive_cashflow_for_in_the_money() {
    let market = standard_market(); // 5% market
    let fra = TestFraBuilder::new()
        .fixed_rate(0.06) // receive 6% (above market)
        .receive_fixed(true) // receive fixed rate
        .build();

    let flows = fra.dated_cashflows(&market, BASE_DATE).unwrap();
    let (_, amount) = flows[0];

    assert_positive(
        amount.amount(),
        "In-the-money receive-fixed should have positive cashflow",
    );
}

#[test]
fn test_negative_cashflow_for_out_of_the_money() {
    let market = standard_market(); // 5% market
    let fra = TestFraBuilder::new()
        .fixed_rate(0.04) // receive 4% (below market)
        .receive_fixed(true) // receive fixed rate
        .build();

    let flows = fra.dated_cashflows(&market, BASE_DATE).unwrap();
    let (_, amount) = flows[0];

    assert_negative(
        amount.amount(),
        "Out-of-the-money receive-fixed should have negative cashflow",
    );
}

#[test]
fn test_cashflow_currency_matches_notional() {
    use finstack_quant_core::currency::Currency;

    let market = standard_market();
    let fra = TestFraBuilder::new()
        .notional(1_000_000.0, Currency::EUR)
        .build();

    let flows = fra.dated_cashflows(&market, BASE_DATE).unwrap();
    let (_, amount) = flows[0];

    assert_eq!(amount.currency(), Currency::EUR);
}
