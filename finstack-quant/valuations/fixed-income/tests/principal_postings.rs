//! Term loans and revolvers recompile their contractual schedule from their
//! own terms after a principal change settled outside the schedule.

use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_cashflows::builder::{AmortizationSpec, CashFlowSchedule, CouponType, RateSpec};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_fixed_income::loan_terms::PrincipalPosting;
use finstack_quant_valuations_fixed_income::revolving_credit::{
    DrawRepayEvent, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations_fixed_income::term_loan::TermLoan;
use rust_decimal_macros::dec;
use time::macros::date;

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("finite amount")
}

fn posting(effective_date: Date, delta: f64) -> PrincipalPosting {
    PrincipalPosting {
        effective_date,
        delta: usd(delta),
    }
}

fn total(schedule: &CashFlowSchedule, kinds: &[CFKind], on: Date) -> f64 {
    schedule
        .get_flows()
        .iter()
        .filter(|flow| kinds.contains(&flow.get_kind()) && flow.get_date() == on)
        .map(|flow| flow.get_amount().amount())
        .sum()
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected:.10}, got {actual:.10}"
    );
}

/// A 10% loan of 100 on 30/360 with one semiannual coupon per half year.
fn semiannual_loan(coupon_type: CouponType) -> TermLoan {
    let mut loan = TermLoan::example().expect("term loan");
    loan.id = "LOAN".into();
    loan.notional_limit = usd(100.0);
    loan.issue_date = date!(2025 - 01 - 01);
    loan.maturity = date!(2026 - 01 - 01);
    loan.rate = RateSpec::Fixed { rate: 0.10 };
    loan.frequency = Tenor::semi_annual();
    loan.day_count = DayCount::Thirty360;
    loan.business_day_convention = BusinessDayConvention::Unadjusted;
    loan.amortization = AmortizationSpec::None;
    loan.coupon_type = coupon_type;
    loan
}

/// The record's example: 10% on 100 for an exact quarter and on 60 for the
/// next exact quarter earns 2.50 and 1.50 inside one coupon.
#[test]
fn a_term_loan_coupon_accrues_on_the_dated_principal() {
    let loan = semiannual_loan(CouponType::Cash);
    let market = MarketContext::new();
    let rebuilt = loan
        .cashflow_schedule_with_postings(&market, &[posting(date!(2025 - 04 - 01), -40.0)])
        .expect("rebuilt schedule");

    assert_close(
        total(&rebuilt, &[CFKind::Fixed], date!(2025 - 07 - 01)),
        4.0,
    );
    // The second coupon earns on 60 throughout, and 60 is redeemed.
    assert_close(
        total(&rebuilt, &[CFKind::Fixed], date!(2026 - 01 - 01)),
        3.0,
    );
    assert_close(
        total(&rebuilt, &[CFKind::Notional], date!(2026 - 01 - 01)),
        60.0,
    );

    // The posting is a zero-cash principal row: its cash settled elsewhere.
    let row = rebuilt
        .get_flows()
        .iter()
        .find(|flow| flow.get_kind() == CFKind::PrePayment)
        .expect("posting row");
    assert_eq!(row.get_date(), date!(2025 - 04 - 01));
    assert_eq!(row.get_amount(), usd(0.0));
    assert_eq!(row.get_principal_delta(), Some(usd(-40.0)));

    // With no postings the loan's ordinary schedule is returned.
    let plain = loan
        .cashflow_schedule_with_postings(&market, &[])
        .expect("plain schedule");
    let raw = loan
        .raw_cashflow_schedule(&market, loan.issue_date)
        .expect("raw schedule");
    assert_eq!(plain.get_flows(), raw.get_flows());
    assert_close(total(&plain, &[CFKind::Fixed], date!(2025 - 07 - 01)), 5.0);
}

/// The contractual 75% cash and 25% PIK split applies once to the rebuilt
/// coupon: 3 of cash and 1 capitalized, which the next coupon earns on.
#[test]
fn the_cash_pik_split_applies_once_to_the_rebuilt_coupon() {
    let loan = semiannual_loan(CouponType::Split {
        cash_fraction: dec!(0.75),
        pik_fraction: dec!(0.25),
    });
    let rebuilt = loan
        .cashflow_schedule_with_postings(
            &MarketContext::new(),
            &[posting(date!(2025 - 04 - 01), -40.0)],
        )
        .expect("rebuilt schedule");
    let capitalized = |on: Date| -> f64 {
        rebuilt
            .get_flows()
            .iter()
            .filter(|flow| flow.get_kind() == CFKind::Pik && flow.get_date() == on)
            .filter_map(|flow| flow.get_principal_delta())
            .map(|delta| delta.amount())
            .sum()
    };
    let july = date!(2025 - 07 - 01);
    assert_close(total(&rebuilt, &[CFKind::Fixed], july), 3.0);
    assert_close(capitalized(july), 1.0);
    // 61 earns 10% for the second half year: 3.05 in total.
    let january = date!(2026 - 01 - 01);
    let second = total(&rebuilt, &[CFKind::Fixed], january) + capitalized(january);
    assert_close(second, 3.05);
}

/// Later amortization and the maturity redemption are sized on what remains,
/// on an actual-day basis with the posting inside a coupon.
#[test]
fn later_term_loan_principal_is_sized_on_what_remains() {
    let mut loan = semiannual_loan(CouponType::Cash);
    loan.frequency = Tenor::quarterly();
    loan.day_count = DayCount::Act365F;
    loan.amortization = AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.30 };
    let effective = date!(2025 - 02 - 15);
    let rebuilt = loan
        .cashflow_schedule_with_postings(&MarketContext::new(), &[posting(effective, -50.0)])
        .expect("rebuilt schedule");

    // 45 days on 100 and 45 days on 50 in the first quarter.
    assert_close(
        total(&rebuilt, &[CFKind::Fixed], date!(2025 - 04 - 01)),
        0.10 * (100.0 * 45.0 + 50.0 * 45.0) / 365.0,
    );
    // 30 is repaid on the first date, and only the 20 left on the second.
    assert_close(
        total(&rebuilt, &[CFKind::Amortization], date!(2025 - 04 - 01)),
        30.0,
    );
    assert_close(
        total(&rebuilt, &[CFKind::Amortization], date!(2025 - 07 - 01)),
        20.0,
    );
    let path = rebuilt.outstanding_by_date().expect("principal path");
    assert_close(path.last().expect("final balance").1.amount(), 0.0);
    assert!(path.iter().all(|(_, balance)| balance.amount() >= -1e-9));
}

#[test]
fn a_term_loan_rejects_postings_it_cannot_apply() {
    let loan = semiannual_loan(CouponType::Cash);
    let market = MarketContext::new();
    let rebuild = |posting: PrincipalPosting| {
        loan.cashflow_schedule_with_postings(&market, &[posting])
            .expect_err("rejected posting")
            .to_string()
    };
    let error = rebuild(posting(date!(2025 - 04 - 01), -140.0));
    assert!(error.contains("exceeds"), "{error}");
    let error = rebuild(posting(date!(2025 - 01 - 01), -10.0));
    assert!(error.contains("outside the life"), "{error}");
    let error = rebuild(PrincipalPosting {
        effective_date: date!(2025 - 04 - 01),
        delta: Money::new(-10.0, Currency::EUR).expect("amount"),
    });
    assert!(error.contains("EUR"), "{error}");
}

/// A 200 commitment with 100 drawn at 8%, a 50 bp commitment fee on the
/// undrawn amount and a 25 bp usage fee on the drawn amount, on ACT/360.
fn revolver(events: Vec<DrawRepayEvent>) -> RevolvingCredit {
    let mut facility = RevolvingCredit::example().expect("revolver");
    facility.id = "RCF".into();
    facility.commitment = usd(200.0);
    facility.drawn = usd(100.0);
    facility.issue_date = date!(2025 - 01 - 01);
    facility.maturity = date!(2026 - 01 - 01);
    facility.rate = RateSpec::Fixed { rate: 0.08 };
    facility.day_count = DayCount::Act360;
    facility.frequency = Tenor::quarterly();
    facility.fees = RevolvingCreditFees::flat(50.0, 25.0, 0.0).expect("fees");
    facility.draw_repay_spec = DrawRepaySpec::Deterministic(events);
    facility
}

fn quarter_rows(schedule: &CashFlowSchedule, on: Date) -> (f64, f64, f64) {
    (
        total(schedule, &[CFKind::Fixed], on),
        total(schedule, &[CFKind::CommitmentFee], on),
        total(schedule, &[CFKind::UsageFee], on),
    )
}

/// A swept revolver earns interest and usage fees on the lower drawn balance
/// and commitment fees on the higher undrawn amount, from the effective date.
#[test]
fn a_swept_revolver_rebuilds_interest_and_balance_fees() {
    let facility = revolver(Vec::new());
    let market = MarketContext::new();
    let as_of = facility.issue_date;
    let plain = facility
        .raw_cashflow_schedule(&market, as_of)
        .expect("plain schedule");
    let first = plain
        .get_flows()
        .iter()
        .find(|flow| flow.get_kind() == CFKind::Fixed)
        .expect("first coupon")
        .get_date();
    let (interest, commitment_fee, usage_fee) = quarter_rows(&plain, first);
    // 90 days from 1 January to 1 April.
    assert_close(interest, 100.0 * 0.08 * 90.0 / 360.0);
    assert_close(commitment_fee, 100.0 * 0.005 * 90.0 / 360.0);
    assert_close(usage_fee, 100.0 * 0.0025 * 90.0 / 360.0);

    // 40 swept at the close of 14 February: 45 days at 100, then 45 at 60.
    let rebuilt = facility
        .cashflow_schedule_with_postings(&market, as_of, &[posting(date!(2025 - 02 - 15), -40.0)])
        .expect("rebuilt schedule");
    let (interest, commitment_fee, usage_fee) = quarter_rows(&rebuilt, first);
    assert_close(interest, 0.08 * (100.0 * 45.0 + 60.0 * 45.0) / 360.0);
    assert_close(
        commitment_fee,
        0.005 * (100.0 * 45.0 + 140.0 * 45.0) / 360.0,
    );
    assert_close(usage_fee, 0.0025 * (100.0 * 45.0 + 60.0 * 45.0) / 360.0);

    // The posting is a zero-cash principal row and 60 is repaid at maturity.
    let row = rebuilt
        .get_flows()
        .iter()
        .find(|flow| flow.get_kind() == CFKind::PrePayment)
        .expect("posting row");
    assert_eq!(row.get_amount(), usd(0.0));
    assert_eq!(row.get_principal_delta(), Some(usd(-40.0)));
    let terminal: f64 = rebuilt
        .get_flows()
        .iter()
        .filter(|flow| flow.get_kind() == CFKind::Notional)
        .map(|flow| flow.get_amount().amount())
        .sum();
    assert_close(terminal, 60.0);
}

/// The facility's terms survive a zero balance: a contractual redraw after a
/// full sweep earns the facility's rate again.
#[test]
fn a_revolver_keeps_its_terms_through_a_zero_balance_and_a_redraw() {
    let facility = revolver(vec![DrawRepayEvent {
        date: date!(2025 - 07 - 01),
        amount: usd(80.0),
        is_draw: true,
    }]);
    let market = MarketContext::new();
    let rebuilt = facility
        .cashflow_schedule_with_postings(
            &market,
            facility.issue_date,
            &[posting(date!(2025 - 04 - 01), -100.0)],
        )
        .expect("rebuilt schedule");
    let coupons: Vec<(Date, f64)> = rebuilt
        .get_flows()
        .iter()
        .filter(|flow| flow.get_kind() == CFKind::Fixed)
        .map(|flow| (flow.get_date(), flow.get_amount().amount()))
        .collect();
    // Nothing is drawn in the second quarter, so it pays no interest; the
    // third quarter (92 days) earns 8% on the 80 redrawn.
    assert_eq!(coupons.len(), 3, "{coupons:?}");
    assert_close(coupons[0].1, 100.0 * 0.08 * 90.0 / 360.0);
    assert_close(coupons[1].1, 80.0 * 0.08 * 92.0 / 360.0);
    assert_close(coupons[2].1, 80.0 * 0.08 * 92.0 / 360.0);
}

#[test]
fn a_revolver_rejects_postings_it_cannot_apply() {
    let facility = revolver(Vec::new());
    let market = MarketContext::new();
    let rebuild = |as_of: Date, posting: PrincipalPosting| {
        facility
            .cashflow_schedule_with_postings(&market, as_of, &[posting])
            .expect_err("rejected posting")
            .to_string()
    };
    let issue = facility.issue_date;
    // More than is drawn, and a capitalization above the commitment.
    let error = rebuild(issue, posting(date!(2025 - 04 - 01), -140.0));
    assert!(error.contains("exceeds current balance"), "{error}");
    let error = rebuild(issue, posting(date!(2025 - 04 - 01), 140.0));
    assert!(error.contains("exceed commitment"), "{error}");
    // `drawn` is the balance at the anchor, so an earlier posting cannot be
    // replayed on top of it.
    let error = rebuild(date!(2025 - 05 - 01), posting(date!(2025 - 04 - 01), -40.0));
    assert!(
        error.contains("on or before the simulation anchor"),
        "{error}"
    );
    let error = rebuild(
        issue,
        PrincipalPosting {
            effective_date: date!(2025 - 04 - 01),
            delta: Money::new(-10.0, Currency::EUR).expect("amount"),
        },
    );
    assert!(error.contains("EUR"), "{error}");
}
