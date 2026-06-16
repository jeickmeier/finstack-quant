//! Cashflow generation and validation tests.

use crate::deposit::common::*;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;

#[test]
fn test_cashflow_generation_two_flows() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .start_date(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let flows = dep.dated_cashflows(&ctx, base).unwrap();

    // Validate - should have exactly 2 flows
    assert_eq!(flows.len(), 2, "Expected 2 cashflows");

    // First flow (payment at start)
    assert_eq!(flows[0].0, base);
    assert_eq!(flows[0].1.currency(), Currency::USD);
    assert!(
        flows[0].1.amount() < 0.0,
        "First flow should be negative (payment)"
    );

    // Second flow (receipt at end)
    assert_eq!(flows[1].0, date(2025, 7, 1));
    assert_eq!(flows[1].1.currency(), Currency::USD);
    assert!(
        flows[1].1.amount() > 0.0,
        "Second flow should be positive (receipt)"
    );
}

#[test]
fn test_cashflow_redemption_amount() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let notional = 1_000_000.0;
    let rate = 0.04;

    let dep = DepositBuilder::new(base)
        .notional(Money::new(notional, Currency::USD))
        .start_date(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(rate)
        .build();

    // Execute
    let flows = dep.dated_cashflows(&ctx, base).unwrap();

    // Calculate expected redemption
    let yf = dep
        .day_count
        .year_fraction(
            dep.start_date,
            dep.maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();
    let expected_redemption = notional * (1.0 + rate * yf);

    // Validate
    assert!((flows[1].1.amount() - expected_redemption).abs() < 1.0);
}

#[test]
fn test_cashflow_conservation_of_value() {
    // Setup - sum of discounted cashflows should equal PV
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .start_date(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    // Execute - compare against the pricing-view trade NPV (`npv_raw`), which
    // discounts every scheduled flow including the T+0 initial exchange. The
    // holder-view `value` excludes flows on `as_of` and would differ by the
    // initial notional.
    let flows = dep.dated_cashflows(&ctx, base).unwrap();
    let npv = dep.npv_raw(&ctx, base).unwrap();

    // Manually discount flows
    let disc = ctx.get_discount("USD-OIS").unwrap();
    let mut manual_pv: f64 = 0.0;
    for (date, amount) in flows {
        let df = disc
            .df_on_date_curve(date)
            .expect("try_df_on_date_curve should succeed");
        manual_pv += amount.amount() * df;
    }

    // Validate - manual calculation should match npv_raw() within machine epsilon
    // Both calculations discount the same cashflows using the same curve,
    // so residual should be at floating-point precision, not basis points.
    assert!(
        (manual_pv - npv).abs() < 1e-6,
        "Cashflow conservation violated: manual_pv={}, npv={}, diff={}",
        manual_pv,
        npv,
        (manual_pv - npv).abs()
    );
}

#[test]
fn test_cashflow_with_zero_rate() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let notional = 1_000_000.0;

    let dep = DepositBuilder::new(base)
        .notional(Money::new(notional, Currency::USD))
        .start_date(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.0)
        .build();

    // Execute
    let flows = dep.dated_cashflows(&ctx, base).unwrap();

    // Validate - with zero rate, redemption = notional
    assert!((flows[1].1.amount() - notional).abs() < 1e-9);
}

#[test]
fn test_cashflow_dates_ordered() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .start_date(base)
        .maturity(date(2025, 7, 1))
        .build();

    // Execute
    let flows = dep.dated_cashflows(&ctx, base).unwrap();

    // Validate - dates should be in order
    assert!(flows[0].0 < flows[1].0);
}

#[test]
fn test_cashflow_notional_scales() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_1m = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::USD))
        .quote_rate(0.03)
        .build();

    let dep_2m = DepositBuilder::new(base)
        .notional(Money::new(2_000_000.0, Currency::USD))
        .quote_rate(0.03)
        .build();

    // Execute
    let flows_1m = dep_1m.dated_cashflows(&ctx, base).unwrap();
    let flows_2m = dep_2m.dated_cashflows(&ctx, base).unwrap();

    // Validate - cashflows should scale linearly
    assert!((flows_2m[0].1.amount() / flows_1m[0].1.amount() - 2.0).abs() < 1e-10);
    assert!((flows_2m[1].1.amount() / flows_1m[1].1.amount() - 2.0).abs() < 1e-6);
}

#[test]
fn test_cashflow_currency_consistency() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .discount_curve_id("EUR-OIS")
        .build();

    // Execute
    let flows = dep.dated_cashflows(&ctx, base).unwrap();

    // Validate - all flows should be in EUR
    for (_, amount) in flows {
        assert_eq!(amount.currency(), Currency::EUR);
    }
}
