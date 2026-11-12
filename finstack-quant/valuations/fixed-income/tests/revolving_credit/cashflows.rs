//! Revolving credit cashflow generation tests.

use finstack_quant_cashflows::builder::CashFlowMeta;
use finstack_quant_cashflows::builder::RateSpec;
use finstack_quant_cashflows::{
    CashflowProvider, ScheduleBuildOpts, schedule_from_classified_flows,
};
use finstack_quant_core::cashflow::{CFKind, CashFlowParams};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_fixed_income::revolving_credit::{
    DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use time::macros::date;

use finstack_quant_valuations_test_support::test_helpers::flat_discount_curve;

#[test]
fn test_interest_on_drawn_amounts() {
    // Arrange
    let facility = RevolvingCredit::builder()
        .id("RC-CF-INTEREST".into())
        .commitment(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .drawn(Money::new(5_000_000.0, Currency::USD).expect("valid money fixture"))
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01))
        .rate(RateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 0.0).unwrap())
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .unwrap();

    // Act
    let market =
        MarketContext::new().insert(flat_discount_curve(0.03, date!(2025 - 01 - 01), "USD-OIS"));
    let cashflows = facility
        .cashflow_schedule(&market, date!(2025 - 01 - 01))
        .unwrap();

    // Assert
    assert!(!cashflows.get_flows().is_empty());
    // Should include quarterly interest payments
}

#[test]
fn test_commitment_fee_on_undrawn() {
    // Arrange
    let facility = RevolvingCredit::builder()
        .id("RC-CF-COMMIT".into())
        .commitment(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .drawn(Money::new(2_000_000.0, Currency::USD).expect("valid money fixture")) // 80% undrawn
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01))
        .rate(RateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(50.0, 10.0, 0.0).unwrap()) // High commitment fee
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .unwrap();

    // Act
    let market =
        MarketContext::new().insert(flat_discount_curve(0.03, date!(2025 - 01 - 01), "USD-OIS"));
    let cashflows = facility
        .cashflow_schedule(&market, date!(2025 - 01 - 01))
        .unwrap();

    // Assert
    assert!(!cashflows.get_flows().is_empty());
    // Should include commitment fees on undrawn portion
}

#[test]
fn test_utilization_fee_at_threshold() {
    // Arrange
    let facility = RevolvingCredit::builder()
        .id("RC-CF-UTIL".into())
        .commitment(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .drawn(Money::new(8_000_000.0, Currency::USD).expect("valid money fixture")) // 80% utilization
        .issue_date(date!(2025 - 01 - 01))
        .maturity(date!(2026 - 01 - 01))
        .rate(RateSpec::Fixed { rate: 0.05 })
        .day_count(DayCount::Act360)
        .yield_basis(finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 15.0).unwrap()) // Utilization fee above threshold
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .recovery_rate(0.0)
        .build()
        .unwrap();

    // Act
    let market =
        MarketContext::new().insert(flat_discount_curve(0.03, date!(2025 - 01 - 01), "USD-OIS"));
    let cashflows = facility
        .cashflow_schedule(&market, date!(2025 - 01 - 01))
        .unwrap();

    // Assert
    assert!(!cashflows.get_flows().is_empty());
}

#[test]
fn same_date_flows_use_the_canonical_cashflow_order() {
    let payment_date = date!(2025 - 07 - 01);
    // A cash principal row moves the balance by minus its cash.
    let flow = |kind: CFKind, amount| {
        let cash = Money::new(amount, Currency::USD).expect("valid money fixture");
        let row = CashFlowParams::new(payment_date, None, cash, kind, 0.0, None);
        if kind.is_principal_like() && kind.is_cash_settlement() {
            row.with_principal_delta(cash.checked_neg())
                .with_principal_date(payment_date)
        } else {
            row
        }
        .build()
        .expect("valid cashflow")
    };
    let flows = vec![
        flow(CFKind::Notional, 5.0),
        CashFlowParams::new(
            payment_date,
            None,
            Money::zero(Currency::USD),
            CFKind::Pik,
            0.0,
            None,
        )
        .with_principal_delta(Money::new(10.0, Currency::USD).expect("valid PIK delta"))
        .with_principal_date(payment_date)
        .build()
        .expect("valid cashflow"),
        flow(CFKind::Fee, 1.0),
        flow(CFKind::Notional, -30.0),
        flow(CFKind::Amortization, 20.0),
    ];

    let schedule = schedule_from_classified_flows(
        flows,
        DayCount::Act360,
        ScheduleBuildOpts {
            notional_hint: Some(Money::new(100.0, Currency::USD).expect("valid money fixture")),
            meta: CashFlowMeta {
                issue_date: Some(date!(2025 - 01 - 01)),
                ..CashFlowMeta::default()
            },
        },
    );
    assert_eq!(
        schedule
            .get_flows()
            .iter()
            .map(|cashflow| cashflow.get_kind())
            .collect::<Vec<_>>(),
        vec![
            CFKind::Fee,
            CFKind::Amortization,
            CFKind::Pik,
            CFKind::Notional,
            CFKind::Notional,
        ]
    );
    assert_eq!(
        schedule.get_flows()[3].get_amount().amount(),
        -30.0,
        "draw sorts before repayment"
    );
    assert_eq!(schedule.get_flows()[4].get_amount().amount(), 5.0);
    let outstanding = schedule.outstanding_by_date().unwrap();
    assert_eq!(
        outstanding,
        vec![(
            payment_date,
            Money::new(115.0, Currency::USD).expect("valid money fixture")
        )]
    );
}
