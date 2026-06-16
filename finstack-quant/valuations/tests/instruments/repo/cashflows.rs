//! Tests for repo cashflow schedule generation.

use super::fixtures::*;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::repo::Repo;
use finstack_quant_valuations::instruments::Instrument;

#[test]
fn test_cashflow_schedule_structure() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "CF_STRUCTURE",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let cashflows = repo.dated_cashflows(&context, date(2025, 1, 10)).unwrap();

    // Should have exactly 2 cashflows: initial outflow and final inflow
    assert_eq!(cashflows.len(), 2);
}

#[test]
fn test_full_schedule_marks_initial_exchange_as_notional() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "CF_KIND",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let schedule = repo
        .cashflow_schedule(&context, date(2025, 1, 10))
        .expect("repo full schedule");

    assert_eq!(schedule.flows.len(), 2);
    assert_eq!(schedule.flows[0].kind, CFKind::Notional);
}

#[test]
fn test_initial_cashflow_negative() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "CF_INITIAL",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let cashflows = repo.dated_cashflows(&context, date(2025, 1, 10)).unwrap();

    let (start_date, cash_outflow) = &cashflows[0];

    assert_eq!(*start_date, date(2025, 1, 15));
    assert_eq!(
        cash_outflow.amount(),
        -1_000_000.0,
        "Initial flow should be negative (outflow)"
    );
    assert_eq!(cash_outflow.currency(), Currency::USD);
}

#[test]
fn test_final_cashflow_includes_interest() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "CF_FINAL",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let cashflows = repo.dated_cashflows(&context, date(2025, 1, 10)).unwrap();

    let (maturity_date, cash_inflow) = &cashflows[1];

    assert_eq!(*maturity_date, date(2025, 4, 15));
    assert!(
        cash_inflow.amount() > 1_000_000.0,
        "Final flow should include interest"
    );
    assert_eq!(cash_inflow.currency(), Currency::USD);

    // Verify it matches total repayment
    let expected_repayment = repo.total_repayment().unwrap();
    assert_money_approx_eq(*cash_inflow, expected_repayment, 0.01);
}

#[test]
fn test_cashflow_dates_match_repo_dates() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    // Use business days (Wed/Mon) to avoid adjustment
    // Note: Repo::term sets target2 calendar + Following BDC
    let start = date(2025, 1, 15); // Wednesday - business day
    let maturity = date(2025, 8, 1); // Friday - business day

    let repo = Repo::term(
        "CF_DATES",
        Money::new(2_000_000.0, Currency::USD),
        collateral,
        0.045,
        start,
        maturity,
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    // Get the expected adjusted dates
    let (adj_start, adj_maturity) = repo.adjusted_dates().unwrap();

    let cashflows = repo.dated_cashflows(&context, date(2025, 1, 10)).unwrap();

    assert_eq!(
        cashflows[0].0, adj_start,
        "First cashflow should be at adjusted start date"
    );
    assert_eq!(
        cashflows[1].0, adj_maturity,
        "Second cashflow should be at adjusted maturity"
    );
}

#[test]
fn test_cashflow_net_present_value() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "CF_NPV",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let cashflows = repo.dated_cashflows(&context, date(2025, 1, 10)).unwrap();

    // Net cashflow should equal interest amount (ignoring time value)
    let net_undiscounted = cashflows[0].1.checked_add(cashflows[1].1).unwrap();
    let interest = repo.interest_amount().unwrap();

    assert_money_approx_eq(net_undiscounted, interest, 0.01);
}

#[test]
fn test_value_matches_discounted_provider_flows() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();
    let as_of = date(2025, 1, 10);

    let repo = Repo::term(
        "CF_VALUE_PATH",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let pv = repo.value(&context, as_of).expect("repo value");
    let discount = context.get_discount("USD-OIS").expect("discount curve");
    let provider_flows = repo
        .dated_cashflows(&context, as_of)
        .expect("provider flows should build");
    let discounted_total = provider_flows
        .into_iter()
        .try_fold(Money::new(0.0, Currency::USD), |acc, (date, amount)| {
            let df = discount.df_between_dates(as_of, date)?;
            acc.checked_add(amount * df)
        })
        .expect("discounting provider flows should succeed");

    assert_money_approx_eq(pv, discounted_total, 0.01);
}

#[test]
fn test_dated_flows_exclude_settled_start_leg_mid_life() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "CF_MIDLIFE",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let cashflows = repo
        .dated_cashflows(&context, date(2025, 2, 1))
        .expect("mid-life dated flows should build");

    assert_eq!(cashflows.len(), 1);
    assert_eq!(cashflows[0].0, date(2025, 4, 15));
    assert!(cashflows[0].1.amount() > 0.0);
}

#[test]
fn test_zero_rate_cashflows() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "CF_ZERO",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.0, // Zero rate
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let cashflows = repo.dated_cashflows(&context, date(2025, 1, 10)).unwrap();

    // Final cashflow should equal principal (no interest)
    assert_eq!(cashflows[1].1.amount(), 1_000_000.0);
}

#[test]
fn test_overnight_repo_cashflows() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::overnight(
        "CF_OVERNIGHT",
        Money::new(5_000_000.0, Currency::USD),
        collateral,
        0.05,
        date(2025, 1, 15),
        "usny",    // Calendar ID for business day adjustment
        "USD-OIS", // Discount curve ID
    )
    .unwrap();

    let cashflows = repo.dated_cashflows(&context, date(2025, 1, 14)).unwrap();

    assert_eq!(cashflows.len(), 2);
    assert_eq!(cashflows[0].0, date(2025, 1, 15));
    assert!(cashflows[1].0 > date(2025, 1, 15), "Maturity after start");

    // Small interest for overnight
    let interest = cashflows[1].1.amount() - 5_000_000.0;
    assert!(interest > 0.0);
    assert!(interest < 1000.0, "Overnight interest should be small");
}

#[test]
fn test_cashflows_currency_consistency() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "CF_CURRENCY",
        Money::new(1_000_000.0, Currency::EUR),
        collateral,
        0.035,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let cashflows = repo.dated_cashflows(&context, date(2025, 1, 10)).unwrap();

    // All cashflows should be in EUR
    assert_eq!(cashflows[0].1.currency(), Currency::EUR);
    assert_eq!(cashflows[1].1.currency(), Currency::EUR);
}

#[test]
fn test_large_notional_cashflows() {
    let context = create_standard_market_context();
    let collateral = treasury_collateral();

    let repo = Repo::term(
        "CF_LARGE",
        Money::new(100_000_000.0, Currency::USD), // 100 million
        collateral,
        0.05,
        date(2025, 1, 15),
        date(2025, 4, 15),
        "USD-OIS",
    )
    .expect("Repo construction should succeed");

    let cashflows = repo.dated_cashflows(&context, date(2025, 1, 10)).unwrap();

    assert_eq!(cashflows[0].1.amount(), -100_000_000.0);
    assert!(cashflows[1].1.amount() > 100_000_000.0);

    // Interest should be proportionally scaled
    let interest = repo.interest_amount().unwrap();
    assert!(interest.amount() > 1_000_000.0);
}
