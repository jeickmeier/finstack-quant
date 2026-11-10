//! Complete public consumer controls for the CCM-I3 accrual investigation.
//!
//! Numerical/export work is untimed. Owner-local panels separately capture
//! actual prepared grid/vector fields; these public diagnostic dates are labelled.

use finstack_quant_valuations_fixed_income as fi;

#[path = "cashflow_accrual_support.rs"]
mod support;

use finstack_quant_cashflows::{CashflowScheduleSource, accrual::accrued_interest_amount};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_valuations_core::instruments::Instrument;
use serde_json::{Value, json};
use time::macros::date;

fn windowed_bond_record() -> Value {
    let as_of = date!(2025 - 03 - 17);
    let bond = support::windowed_bond();
    let market = support::bond_market();
    let before = serde_json::to_value(&market).expect("market descriptor");
    let schedule = bond
        .raw_cashflow_schedule(&market, as_of)
        .expect("full signed schedule");
    assert_eq!(schedule.coupons().count(), 20);
    for coupon in schedule.coupons() {
        support::close(coupon.amount.amount(), 2.5, support::AI_BOUND_USD);
    }
    let value = bond
        .value(&market, as_of)
        .expect("complete windowed callable price");
    let repeated = bond.value(&market, as_of).expect("repeat complete price");
    assert_eq!(value.amount().to_bits(), repeated.amount().to_bits());
    assert_eq!(
        value.amount_decimal().serialize(),
        repeated.amount_decimal().serialize()
    );
    assert_eq!(value.currency(), Currency::USD);
    let mut straight = bond.clone();
    straight.call_put = None;
    let straight_value = straight.value(&market, as_of).expect("straight bond price");
    assert!(value.amount() > 0.0 && value.amount() < straight_value.amount());
    let dates = [
        as_of,
        date!(2030 - 01 - 31),
        date!(2032 - 07 - 15),
        date!(2034 - 12 - 31),
    ];
    let queries = support::queries(&schedule, &bond.accrual_config(), &dates);
    assert_eq!(
        before,
        serde_json::to_value(&market).expect("unchanged market")
    );
    json!({
        "fixture_id": support::WINDOWED_BOND_ID,
        "fixture_source": "tests/bond/exercise_grid.rs::PAR-CALL-WINDOW with explicit tree_steps=100",
        "operation": "Instrument::value(&bond, &market, as_of)",
        "as_of": as_of.to_string(), "instrument": bond, "market": before,
        "schedule": support::schedule(&schedule), "value": support::money(value),
        "straight_value": support::money(straight_value), "repeat_value_bits_equal": true,
        "diagnostic_queries": queries,
        "actual_private_grid_capture": "bond::pricing::tree::tree_pricer::cashflow_accrual_owner",
        "bounds": {"independent_ai_usd": support::AI_BOUND_USD,
            "price": "positive and strictly below straight; exact C0 price bits frozen by capture"},
    })
}

fn callable_loan_record() -> Value {
    let as_of = date!(2025 - 01 - 01);
    let loan = support::callable_loan();
    let market = support::loan_market(as_of);
    let before = serde_json::to_value(&market).expect("market descriptor");
    let schedule = loan
        .raw_cashflow_schedule(&market, as_of)
        .expect("full signed schedule");
    assert_eq!(schedule.coupons().count(), 20);
    for coupon in schedule.coupons() {
        let accrual = coupon.accrual.as_ref().expect("loan coupon accrual");
        let expected =
            10_000_000.0 * 0.06 * (accrual.end - accrual.start).whole_days() as f64 / 360.0;
        support::close(coupon.amount.amount(), expected, support::AI_BOUND_USD);
    }
    let pricer = fi::term_loan::TermLoanTreePricer;
    let value = pricer
        .price_callable(&loan, &market, as_of)
        .expect("complete callable loan price");
    let repeated = pricer
        .price_callable(&loan, &market, as_of)
        .expect("repeat complete loan price");
    assert_eq!(value.amount().to_bits(), repeated.amount().to_bits());
    assert_eq!(
        value.amount_decimal().serialize(),
        repeated.amount_decimal().serialize()
    );
    assert_eq!(value.currency(), Currency::USD);
    let mut straight = loan.clone();
    straight.call_schedule = None;
    let straight_value = pricer
        .price_callable(&straight, &market, as_of)
        .expect("straight tree loan");
    assert!(value.amount() > 0.0 && value.amount() < straight_value.amount());
    let dates = [
        date!(2027 - 01 - 15),
        date!(2028 - 06 - 15),
        date!(2029 - 12 - 15),
    ];
    let queries = support::queries(&schedule, &loan.accrual_config(), &dates);
    let mut pinned = loan.clone();
    pinned.instrument_pricing_overrides.market_quotes.quoted_oas = Some(0.0075);
    let pinned_value = pricer
        .price_callable(&pinned, &market, as_of)
        .expect("existing OAS price pin");
    // One ulp apart between macOS and the Linux libm on GitHub Actions.
    let pinned_bits = pinned_value.amount().to_bits();
    assert!(
        [4_711_601_891_810_740_309, 4_711_601_891_810_740_308].contains(&pinned_bits),
        "callable loan OAS price moved off the macOS and Linux libm pins: bits={pinned_bits}"
    );
    assert_eq!(
        before,
        serde_json::to_value(&market).expect("unchanged market")
    );
    json!({
        "fixture_id": support::CALLABLE_LOAN_ID,
        "fixture_source": "tests/term_loan/tree_pricing.rs::build_callable_loan with explicit steps100/sigma0.01",
        "operation": "TermLoanTreePricer::price_callable(&loan, &market, as_of)",
        "as_of": as_of.to_string(), "settlement": loan.settlement_date(as_of).expect("settlement").to_string(),
        "instrument": loan, "market": before, "schedule": support::schedule(&schedule),
        "value": support::money(value), "straight_value": support::money(straight_value),
        "repeat_value_bits_equal": true, "diagnostic_queries": queries,
        "existing_decimal_oas_control": {"oas_decimal": 0.0075, "instrument": pinned, "value": support::money(pinned_value)},
        "actual_private_grid_capture": "term_loan::pricing::tree_engine::cashflow_accrual_owner",
        "bounds": {"independent_ai_usd": support::AI_BOUND_USD,
            "price": "positive and strictly below straight; existing 75bp OAS exact pin retained"},
    })
}

fn immediate_loan_record() -> Value {
    let as_of = date!(2025 - 02 - 15);
    let loan = support::immediate_loan();
    let market = support::loan_market(as_of);
    let schedule = loan
        .raw_cashflow_schedule(&market, as_of)
        .expect("full signed schedule");
    let accrued =
        accrued_interest_amount(&schedule, as_of, &loan.accrual_config()).expect("cash AI");
    let independent_ai = 10_000_000.0 * 0.06 * 45.0 / 360.0;
    support::close(accrued, independent_ai, support::AI_BOUND_USD);
    let value = fi::term_loan::TermLoanTreePricer
        .price_callable(&loan, &market, as_of)
        .expect("immediate par call");
    support::close(
        value.amount(),
        10_000_000.0 + independent_ai,
        support::IMMEDIATE_CALL_BOUND_USD,
    );
    json!({
        "fixture_id": support::IMMEDIATE_LOAN_ID,
        "fixture_source": "tests/term_loan/tree_pricing.rs::off_cycle_immediate_par_call_includes_accrued",
        "operation": "TermLoanTreePricer::price_callable(&loan, &market, as_of)",
        "as_of": as_of.to_string(), "instrument": loan, "market": market,
        "schedule": support::schedule(&schedule), "value": support::money(value),
        "accrued": support::scalar(accrued), "independent_ai": support::scalar(independent_ai),
        "independent_dirty_call": support::scalar(10_000_000.0 + independent_ai),
        "bounds": {"ai_usd": support::AI_BOUND_USD, "complete_value_usd": support::IMMEDIATE_CALL_BOUND_USD},
    })
}

fn pik_loan_record() -> Value {
    let market = finstack_quant_core::market_data::context::MarketContext::new();
    let mut records = Vec::new();
    for amortizing in [false, true] {
        let loan = support::pik_loan(amortizing);
        let schedule = loan
            .raw_cashflow_schedule(&market, loan.issue_date)
            .expect("full PIK schedule");
        let mut total_config = loan.accrual_config();
        total_config.include_pik = true;
        let dates: [Date; 2] = [date!(2025 - 07 - 01), date!(2026 - 01 - 01)];
        let cash = support::queries(&schedule, &loan.accrual_config(), &dates);
        let total = support::queries(&schedule, &total_config, &dates);
        let midyear_cash =
            accrued_interest_amount(&schedule, dates[0], &loan.accrual_config()).expect("cash AI");
        let midyear_total =
            accrued_interest_amount(&schedule, dates[0], &total_config).expect("total AI");
        support::close(midyear_cash, 0.0, 0.0);
        support::close(midyear_total, 5.0, support::AI_BOUND_USD);
        let path = schedule
            .outstanding_by_date()
            .expect("PIK outstanding path");
        let anniversary_principal = path
            .iter()
            .rfind(|(date, _)| *date <= dates[1])
            .expect("anniversary balance")
            .1;
        let expected_principal = if amortizing { 55.0 } else { 110.0 };
        support::close(
            anniversary_principal.amount(),
            expected_principal,
            support::AI_BOUND_USD,
        );
        records.push(json!({
            "amortizing": amortizing, "instrument": loan, "schedule": support::schedule(&schedule),
            "cash_queries": cash, "total_queries": total,
            "claim_provenance": "public outstanding and AI reconstruction; actual ExerciseClaims::at is captured owner-locally",
            "reconstructed_midyear_principal": support::scalar(100.0 + midyear_total - midyear_cash),
            "anniversary_principal": support::money(anniversary_principal),
            "independent_expected_anniversary_principal": expected_principal,
        }));
    }
    json!({"fixture_id": support::PIK_LOAN_ID, "fixture_source": "term_loan/pricing/call.rs::call_claim_includes_pik_once_after_scheduled_amortization",
        "records": records, "bound_usd": support::AI_BOUND_USD})
}

#[test]
fn windowed_bond_complete_price_and_accrual() {
    windowed_bond_record();
}

#[test]
fn callable_loan_complete_price_and_accrual() {
    callable_loan_record();
}

#[test]
fn immediate_par_call_has_independent_dirty_value() {
    immediate_loan_record();
}

#[test]
fn pik_loan_public_accrual_and_balances() {
    pik_loan_record();
}

#[test]
#[ignore = "opt-in untimed CCM-I3 consumer baseline capture"]
fn export_cashflow_consumer_panel() {
    let _destination = support::export_destination();
    for record in [
        windowed_bond_record(),
        callable_loan_record(),
        immediate_loan_record(),
        pik_loan_record(),
    ] {
        support::emit(record);
    }
}
