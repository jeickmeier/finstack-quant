//! Opening principal reconciliation and contractual accrued cash interest
//! (STATEMENTS-CAPITAL-003).
#![allow(clippy::expect_used)]

use finstack_quant_cashflows::builder::{
    AmortizationSpec, CashFlowMeta, CashFlowSchedule, CouponType, Notional, RateSpec,
};
use finstack_quant_core::cashflow::{CFKind, CashFlowParams};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, PeriodId, StubKind, Tenor, build_periods,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, Rate};
use finstack_quant_statements::builder::ModelBuilder;
use finstack_quant_statements::capital_structure::{
    PaymentPriority, WaterfallSpec, calculate_period_flows, scheduled_opening_principal,
};
use finstack_quant_statements::evaluator::Evaluator;
use finstack_quant_statements::types::{AmountOrScalar, FinancialStatementInstrument};
use finstack_quant_valuations::instruments::BondConvention;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use indexmap::IndexMap;
use std::sync::Arc;
use time::Month;

use crate::support::{ScheduleInstrument, aggregate_period_flows};

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid date")
}

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("valid money")
}

/// A semi-annual ACT/ACT ISMA bond accrues its coupon by actual days over
/// the actual days of the coupon period. The accrual needs the coupon
/// frequency, which only the instrument's own earning terms supply.
#[test]
fn an_isma_bond_accrues_its_contractual_coupon_mid_period() {
    let bond = Bond::with_convention(
        "UST",
        usd(1_000_000.0),
        Rate::from_decimal(0.04).expect("rate"),
        date(2025, Month::January, 15),
        date(2030, Month::January, 15),
        BondConvention::UsTreasury,
        "USD-TREASURY",
    )
    .expect("treasury bond");
    assert_eq!(bond.cashflow_spec.day_count(), DayCount::ActActIsma);

    let mut instruments: IndexMap<String, Arc<dyn Instrument>> = IndexMap::new();
    instruments.insert("UST".into(), Arc::new(bond));
    let periods = build_periods("2025Q1..Q3", None).expect("periods").periods;
    let cashflows = aggregate_period_flows(
        &instruments,
        &periods,
        &MarketContext::new(),
        date(2025, Month::January, 15),
    )
    .expect("an ISMA schedule accrues on its instrument's terms");
    let accrued = |quarter: u8| {
        cashflows
            .get_component(
                "accrued_interest",
                "UST",
                &PeriodId::quarter(2025, quarter).expect("quarter"),
            )
            .expect("accrued")
    };

    // The first coupon of 20,000 runs 15 January to 15 July, 181 days. The
    // Q1 close is 31 March, 75 days in; the Q2 close is 30 June, 166 days in.
    let coupon = 20_000.0;
    assert!(
        (accrued(1) - coupon * 75.0 / 181.0).abs() < 0.01,
        "{}",
        accrued(1)
    );
    assert!(
        (accrued(2) - coupon * 166.0 / 181.0).abs() < 0.01,
        "{}",
        accrued(2)
    );
    // The coupon is paid on 15 July: Q3 holds only the next period's accrual
    // (15 July to 30 September, 77 of 184 days).
    assert!(
        (accrued(3) - coupon * 77.0 / 184.0).abs() < 0.01,
        "{}",
        accrued(3)
    );
}

fn term_loan() -> FinancialStatementInstrument {
    FinancialStatementInstrument::TermLoan(
        TermLoan::builder()
            .id("TL".into())
            .currency(Currency::USD)
            .notional_limit(usd(10_000_000.0))
            .issue_date(date(2025, Month::January, 1))
            .maturity(date(2030, Month::January, 1))
            .rate(RateSpec::Fixed { rate: 0.05 })
            .frequency(Tenor::quarterly())
            .quote_compounding(finstack_quant_core::math::Compounding::QUARTERLY)
            .day_count(DayCount::Act360)
            .yield_basis(finstack_quant_valuations::metrics::returns::YieldBasis::Act360)
            .business_day_convention(BusinessDayConvention::ModifiedFollowing)
            .calendar_id_opt(None)
            .stub(StubKind::None)
            .discount_curve_id(CurveId::from("USD-OIS"))
            .amortization(AmortizationSpec::None)
            .coupon_type(CouponType::Cash)
            .upfront_fee_opt(None)
            .ddtl_opt(None)
            .covenants_opt(None)
            .instrument_pricing_overrides(Default::default())
            .attributes(Default::default())
            .build()
            .expect("valid term loan"),
    )
}

/// Interest earned and not yet due is accrued. On its due date an unpaid
/// coupon stops being accrued and becomes arrears, which stay separate
/// until they are paid.
#[test]
fn an_unpaid_coupon_moves_from_accrued_to_arrears_at_its_due_date() {
    let month = |m: u8| PeriodId::month(2025, m).expect("month");
    let mut builder = ModelBuilder::new("arrears");
    builder.periods("2025M1..M5", None).expect("periods");
    // No cash until May.
    let cash: Vec<(PeriodId, AmountOrScalar)> = (1..=5)
        .map(|m| {
            let amount = if m == 5 { 1_000_000.0 } else { 0.0 };
            (month(m), AmountOrScalar::scalar(amount))
        })
        .collect();
    builder.value("cash", &cash);
    builder
        .compute("accrued", "cs.accrued_interest.TL")
        .expect("formula");
    builder
        .compute("arrears", "cs.interest_arrears.TL")
        .expect("formula");
    builder
        .compute("paid", "cs.interest_expense_cash.TL")
        .expect("formula");
    builder.add_debt("TL", term_loan());
    builder.waterfall(WaterfallSpec {
        priority_of_payments: vec![
            PaymentPriority::Fees,
            PaymentPriority::Interest,
            PaymentPriority::Amortization,
            PaymentPriority::Equity,
        ],
        available_cash_node: "cash".into(),
        ..Default::default()
    });
    let model = builder.build().expect("model");
    let results = Evaluator::new().evaluate(&model).expect("evaluation");
    let get = |node: &str, m: u8| results.get(node, &month(m)).expect("value");
    let close = |actual: f64, expected: f64| {
        assert!((actual - expected).abs() < 0.01, "{actual} vs {expected}");
    };
    let daily = 10_000_000.0 * 0.05 / 360.0;

    // March closes the day before the first coupon is due: 89 days earned,
    // nothing overdue.
    close(get("accrued", 3), daily * 89.0);
    close(get("arrears", 3), 0.0);
    // The 90-day coupon falls due on 1 April with no cash to pay it. It is
    // arrears, and the accrual holds only April's own 29 days.
    close(get("paid", 4), 0.0);
    close(get("arrears", 4), daily * 90.0);
    close(get("accrued", 4), daily * 29.0);
    // May pays the carried coupon: no arrears remain and the accrual is the
    // 60 days earned since 1 April.
    close(get("paid", 5), daily * 90.0);
    close(get("arrears", 5), 0.0);
    close(get("accrued", 5), daily * 60.0);
}

/// Newly issued and redrawn debt closes on the dated schedule: every
/// funding, repayment and capitalization moves the balance on its
/// balance-effective date, whether its cash settles early or late.
#[test]
fn new_funding_and_redraws_close_on_balance_dates_not_cash_dates() {
    let principal_row = |cash_date: Date, balance_date: Date, cash: f64, delta: f64| {
        CashFlowParams::new(cash_date, None, usd(cash), CFKind::Notional, 0.0, None)
            .with_principal_delta(usd(delta))
            .with_principal_date(balance_date)
            .build()
            .expect("valid principal row")
    };
    let issue = date(2025, Month::January, 15);
    let flows = vec![
        // Issued on 15 January for cash the same day.
        principal_row(issue, issue, -100.0, 100.0),
        // Repaid on 20 February; the cash settles late, on 5 April.
        principal_row(
            date(2025, Month::April, 5),
            date(2025, Month::February, 20),
            100.0,
            -100.0,
        ),
        // Redrawn on 2 April; the cash arrived early, on 28 March.
        principal_row(
            date(2025, Month::March, 28),
            date(2025, Month::April, 2),
            -60.0,
            60.0,
        ),
        // Interest capitalized on 31 March.
        CashFlowParams::new(
            date(2025, Month::March, 31),
            None,
            Money::zero(Currency::USD),
            CFKind::Pik,
            0.0,
            None,
        )
        .with_principal_delta(usd(5.0))
        .with_principal_date(date(2025, Month::March, 31))
        .build()
        .expect("valid capitalization"),
    ];
    let schedule = CashFlowSchedule::from_flows(
        flows,
        // The dated draw establishes the principal.
        Notional::par(0.0, Currency::USD).expect("notional"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(issue),
            ..Default::default()
        },
    );
    let instrument = ScheduleInstrument::new(schedule.clone());
    let periods = build_periods("2025Q1..Q2", None).expect("periods").periods;
    let market = MarketContext::new();
    let flows_of = |index: usize, opening: Money| {
        calculate_period_flows(&instrument, &periods[index], opening, &market, issue, None)
    };

    // Q1: issued 100, repaid 100 on its balance date, capitalized 5. The late
    // repayment cash and the early redraw cash do not move the balance.
    let opening = scheduled_opening_principal(&schedule, periods[0].start).expect("opening");
    assert_eq!(opening.amount(), 0.0);
    let (q1, q1_close, q1_funding, _) = flows_of(0, opening).expect("Q1");
    assert_eq!(q1_close.amount(), 5.0);
    assert_eq!(q1_funding.amount(), 100.0);
    assert_eq!(q1.principal_payment.amount(), 0.0);
    assert_eq!(q1.interest_expense_pik.amount(), 5.0);

    // Q2 opens where Q1 closed: the redraw is effective on 2 April and the
    // repayment cash of 100 settles on 5 April without moving the balance.
    assert_eq!(
        scheduled_opening_principal(&schedule, periods[1].start)
            .expect("opening")
            .amount(),
        5.0
    );
    let (q2, q2_close, q2_funding, _) = flows_of(1, q1_close).expect("Q2");
    assert_eq!(q2_close.amount(), 65.0);
    assert_eq!(q2_funding.amount(), 60.0);
    assert_eq!(q2.principal_payment.amount(), 100.0);

    // A ledger that disagrees with the schedule at the boundary is an error.
    let error = flows_of(1, usd(105.0))
        .expect_err("the ledger must agree with the schedule")
        .to_string();
    for needle in ["SCHEDULE", "2025Q2", "USD", "105", "carries 5 USD"] {
        assert!(error.contains(needle), "{needle}: {error}");
    }
}
