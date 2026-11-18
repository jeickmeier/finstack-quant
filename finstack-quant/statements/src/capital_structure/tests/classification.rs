//! Period admission and cashflow classification (STATEMENTS-CAPITAL-002).

use crate::capital_structure::calculate_period_flows;
use crate::support::ScheduleInstrument;
use finstack_quant_cashflows::builder::{CashFlowMeta, CashFlowSchedule, Notional};
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowParams};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Period, build_periods};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use time::macros::date;

fn usd(amount: f64) -> Money {
    Money::new(amount, Currency::USD).expect("valid money")
}

/// A cash row in the schedule's holder perspective, with an optional
/// principal effect on its own date.
fn row(kind: CFKind, date: Date, cash: f64, principal: Option<(f64, Date)>) -> CashFlow {
    let mut params = CashFlowParams::new(date, None, usd(cash), kind, 0.0, None);
    if let Some((delta, on)) = principal {
        params = params
            .with_principal_delta(usd(delta))
            .with_principal_date(on);
    }
    params.build().expect("valid cashflow")
}

fn debt(flows: Vec<CashFlow>) -> ScheduleInstrument {
    ScheduleInstrument::new(CashFlowSchedule::from_flows(
        flows,
        Notional::par(100.0, Currency::USD).expect("valid principal"),
        DayCount::Act365F,
        CashFlowMeta {
            issue_date: Some(date!(2025 - 01 - 01)),
            ..Default::default()
        },
    ))
}

fn months(range: &str) -> Vec<Period> {
    build_periods(range, None).expect("periods").periods
}

/// The record's example: a USD 100 liability extinguished on March 31 is a
/// USD 100 face reduction in March; the USD 102 payment on April 1 is
/// company cash of USD -102 in April.
#[test]
fn face_reduction_and_repayment_cash_keep_their_own_periods() {
    let instrument = debt(vec![row(
        CFKind::Notional,
        date!(2025 - 04 - 01),
        102.0,
        Some((-100.0, date!(2025 - 03 - 31))),
    )]);
    let periods = months("2025M3..2025M4");
    let market = MarketContext::new();
    let (march, march_close, _) = calculate_period_flows(
        &instrument,
        &periods[0],
        usd(100.0),
        &market,
        date!(2025 - 01 - 01),
        None,
    )
    .expect("march");
    assert_eq!(march.principal_payment, usd(0.0));
    assert_eq!(march_close, usd(0.0));
    let (april, april_close, _) = calculate_period_flows(
        &instrument,
        &periods[1],
        march_close,
        &market,
        date!(2025 - 01 - 01),
        None,
    )
    .expect("april");
    assert_eq!(april.principal_payment, usd(102.0));
    assert_eq!(april_close, usd(0.0));
}

/// The record's example: a liability of 100 with an explicit reduction of 40
/// has principal of 60; a separately identified distress settlement of 15
/// paid by the company is not scheduled principal.
#[test]
fn distress_cash_is_reported_outside_scheduled_debt_service() {
    let instrument = debt(vec![
        row(
            CFKind::DefaultedNotional,
            date!(2025 - 02 - 10),
            0.0,
            Some((-40.0, date!(2025 - 02 - 10))),
        ),
        row(CFKind::Recovery, date!(2025 - 02 - 20), 15.0, None),
    ]);
    let period = months("2025M2..2025M2").remove(0);
    let (flows, close, _) = calculate_period_flows(
        &instrument,
        &period,
        usd(100.0),
        &MarketContext::new(),
        date!(2025 - 01 - 01),
        None,
    )
    .expect("february");
    assert_eq!(flows.principal_payment, usd(0.0));
    assert_eq!(flows.fees, usd(0.0));
    assert_eq!(flows.interest_expense_cash, usd(0.0));
    assert_eq!(close, usd(60.0));
    // The settlement is reported on its own, as company cash of -15.
    let distress = crate::capital_structure::period_flows::period_distress_cash(
        "TL",
        true,
        crate::capital_structure::SuppliedSchedule::holder(&instrument.schedule),
        &period,
    )
    .expect("distress");
    assert_eq!(distress.len(), 1);
    assert_eq!(distress[0].amount, usd(-15.0));
    assert_eq!(distress[0].payment_date, date!(2025 - 02 - 20));
}

/// Distress cash keeps its instrument, date and the company's sign: the
/// borrower pays a settlement of 15 (company cash -15) and a hedge
/// counterparty's protection payment of 7 is a company receipt.
#[test]
fn distress_rows_are_dated_and_signed_for_the_company() {
    use crate::capital_structure::SuppliedSchedule;
    use crate::capital_structure::period_flows::period_distress_cash;
    let instrument = debt(vec![
        row(CFKind::Recovery, date!(2025 - 02 - 20), 15.0, None),
        row(CFKind::ProtectionPayment, date!(2025 - 03 - 05), 7.0, None),
    ]);
    let february = months("2025M2..2025M2").remove(0);
    let borrower = period_distress_cash(
        "TL",
        true,
        SuppliedSchedule::holder(&instrument.schedule),
        &february,
    )
    .expect("distress");
    assert_eq!(borrower.len(), 1, "March cash is not February's");
    assert_eq!(
        (
            borrower[0].instrument_id.as_str(),
            borrower[0].payment_date,
            borrower[0].kind,
            borrower[0].amount,
        ),
        ("TL", date!(2025 - 02 - 20), CFKind::Recovery, usd(-15.0))
    );
    let march = months("2025M3..2025M3").remove(0);
    let hedge = period_distress_cash(
        "HEDGE",
        false,
        SuppliedSchedule::holder(&instrument.schedule),
        &march,
    )
    .expect("distress");
    assert_eq!(hedge[0].amount, usd(7.0));
}

/// A period is validated before any schedule is read, and a borrowing
/// schedule cannot arrive in an unstated or company perspective.
#[test]
fn periods_and_perspectives_are_admitted_before_schedules_are_read() {
    use crate::capital_structure::{SchedulePerspective, SuppliedSchedule};
    let instrument = debt(vec![]);
    let mut reversed = months("2025M2..2025M2").remove(0);
    std::mem::swap(&mut reversed.start, &mut reversed.end);
    let mut empty = months("2025M2..2025M2").remove(0);
    empty.end = empty.start;
    let market = MarketContext::new();
    for period in [&reversed, &empty] {
        let error = calculate_period_flows(
            &instrument,
            period,
            usd(100.0),
            &market,
            date!(2025 - 01 - 01),
            None,
        )
        .expect_err("invalid period");
        assert!(error.to_string().contains("empty or reversed"), "{error}");
    }
    let period = months("2025M2..2025M2").remove(0);
    let error = calculate_period_flows(
        &instrument,
        &period,
        usd(100.0),
        &market,
        date!(2025 - 01 - 01),
        Some(SuppliedSchedule {
            schedule: &instrument.schedule,
            perspective: SchedulePerspective::Company,
        }),
    )
    .expect_err("company-perspective borrowing schedule");
    assert!(error.to_string().contains("holder perspective"), "{error}");
    // The native currency is established for a zero opening too.
    let error = calculate_period_flows(
        &instrument,
        &period,
        Money::new(0.0, Currency::EUR).expect("valid money"),
        &market,
        date!(2025 - 01 - 01),
        None,
    )
    .expect_err("foreign zero opening");
    assert!(
        matches!(error, crate::error::Error::CurrencyMismatch(..)),
        "{error}"
    );
}

/// Company cash is signed once: fees and repayments the company pays are
/// claims; a row of those kinds that is a company receipt, a margin row and
/// an inflation coupon are each classified explicitly, never by magnitude.
#[test]
fn every_kind_is_classified_with_its_sign() {
    let period = months("2025M2..2025M2").remove(0);
    let flows = |rows: Vec<CashFlow>| {
        calculate_period_flows(
            &debt(rows),
            &period,
            usd(100.0),
            &MarketContext::new(),
            date!(2025 - 01 - 01),
            None,
        )
    };
    let on = date!(2025 - 02 - 10);
    // The lender receives a 2 fee and a 3 inflation-linked coupon.
    let (paid, _, _) = flows(vec![
        row(CFKind::Fee, on, 2.0, None),
        row(CFKind::InflationCoupon, on, 3.0, None),
    ])
    .expect("payable rows");
    assert_eq!(paid.fees, usd(2.0));
    assert_eq!(paid.interest_expense_cash, usd(3.0));
    // A fee the lender pays back is a company receipt, not a fee claim.
    let rebate = flows(vec![row(CFKind::Fee, on, -2.0, None)]).expect_err("rebate");
    assert!(rebate.to_string().contains("receipt"), "{rebate}");
    // Margin cash has no capital-structure classification.
    let margin =
        flows(vec![row(CFKind::VariationMarginPay, on, -1.0, None)]).expect_err("margin row");
    assert!(
        margin
            .to_string()
            .contains("no capital-structure classification"),
        "{margin}"
    );
    // A PIK correction reduces the capitalized total with its sign.
    let (pik, _, _) = flows(vec![
        row(CFKind::Pik, on, 0.0, Some((5.0, on))),
        row(CFKind::Pik, on, 0.0, Some((-1.0, on))),
    ])
    .expect("pik rows");
    assert_eq!(pik.interest_expense_pik, usd(4.0));
}
