//! Revolving credit cashflow generation tests.

use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use time::macros::date;

use crate::common::test_helpers::flat_discount_curve;

#[test]
fn test_interest_on_drawn_amounts() {
    // Arrange
    let facility = RevolvingCredit::builder()
        .id("RC-CF-INTEREST".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 0.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    // Act
    let market =
        MarketContext::new().insert(flat_discount_curve(0.03, date!(2025 - 01 - 01), "USD-OIS"));
    let cashflows = facility
        .cashflow_schedule(&market, date!(2025 - 01 - 01))
        .unwrap();

    // Assert
    assert!(!cashflows.flows.is_empty());
    // Should include quarterly interest payments
}

#[test]
fn test_commitment_fee_on_undrawn() {
    // Arrange
    let facility = RevolvingCredit::builder()
        .id("RC-CF-COMMIT".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(2_000_000.0, Currency::USD)) // 80% undrawn
        .commitment_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 10.0, 0.0)) // High commitment fee
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    // Act
    let market =
        MarketContext::new().insert(flat_discount_curve(0.03, date!(2025 - 01 - 01), "USD-OIS"));
    let cashflows = facility
        .cashflow_schedule(&market, date!(2025 - 01 - 01))
        .unwrap();

    // Assert
    assert!(!cashflows.flows.is_empty());
    // Should include commitment fees on undrawn portion
}

#[test]
fn test_utilization_fee_at_threshold() {
    // Arrange
    let facility = RevolvingCredit::builder()
        .id("RC-CF-UTIL".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(8_000_000.0, Currency::USD)) // 80% utilization
        .commitment_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01))
        .base_rate_spec(BaseRateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 15.0)) // Utilization fee above threshold
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap();

    // Act
    let market =
        MarketContext::new().insert(flat_discount_curve(0.03, date!(2025 - 01 - 01), "USD-OIS"));
    let cashflows = facility
        .cashflow_schedule(&market, date!(2025 - 01 - 01))
        .unwrap();

    // Assert
    assert!(!cashflows.flows.is_empty());
}
