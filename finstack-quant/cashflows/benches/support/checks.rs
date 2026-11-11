//! Untimed assertions for the cashflow benchmark fixtures.

use finstack_quant_cashflows::aggregation::PeriodAggregation;
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, Period};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;

pub(super) fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(actual.is_finite(), "non-finite benchmark output: {actual}");
    assert!(expected.is_finite());
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual {actual}, expected {expected}, tolerance {tolerance}"
    );
}

pub(super) fn rows(schedule: &CashFlowSchedule, expected_rows: usize) {
    schedule.validate().expect("valid preflight schedule");
    assert_eq!(schedule.get_flows().len(), expected_rows);
    assert!(
        schedule
            .get_flows()
            .windows(2)
            .all(|w| w[0].date <= w[1].date)
    );
    for flow in schedule.get_flows() {
        assert_eq!(flow.amount.currency(), Currency::USD);
        assert!(flow.amount.amount().is_finite());
        assert!(flow.accrual_factor.is_finite());
        assert!(flow.rate.is_none_or(f64::is_finite));
    }
}

pub(super) fn fixed_amounts(schedule: &CashFlowSchedule) {
    for flow in schedule.coupons() {
        assert_eq!(flow.kind, CFKind::Fixed);
        let accrual = flow
            .accrual
            .as_ref()
            .expect("fixed coupon accrual metadata");
        assert_eq!(accrual.day_count, DayCount::Act365F);
        assert!(accrual.start < accrual.end);
        let mut principal = schedule.get_notional().initial.amount();
        for movement in schedule.get_flows() {
            if movement.kind == CFKind::Amortization && movement.get_balance_date() <= accrual.start
            {
                principal -= movement.amount.amount();
            }
        }
        let fraction = (accrual.end - accrual.start).whole_days() as f64 / 365.0;
        close(flow.accrual_factor, fraction, 1e-12);
        close(flow.rate.expect("fixed coupon rate"), 0.06, 1e-14);
        close(flow.amount.amount(), principal * 0.06 * fraction, 1e-7);
    }
}

pub(super) fn fixed_bullet(
    schedule: &CashFlowSchedule,
    base: Date,
    years: i32,
    coupons: usize,
    adjust_payments: bool,
) {
    rows(schedule, coupons + 2);
    assert_eq!(schedule.get_meta().issue_date, Some(base));
    let maturity = base.add_months(years * 12).expect("fixture maturity");
    assert_eq!(schedule.get_meta().maturity, Some(maturity));
    assert_eq!(schedule.coupons().count(), coupons);
    assert!(schedule.get_meta().projected_fixings.is_empty());
    let months = years * 12 / coupons as i32;
    for (index, flow) in schedule.coupons().enumerate() {
        let accrual = flow.accrual.as_ref().expect("coupon accrual");
        let start = base
            .add_months(index as i32 * months)
            .expect("fixture start");
        let end = base
            .add_months((index as i32 + 1) * months)
            .expect("fixture end");
        assert_eq!((accrual.start, accrual.end), (start, end));
        let mut payment = end;
        if adjust_payments {
            while payment.is_weekend() {
                payment += time::Duration::days(1);
            }
        }
        assert_eq!(flow.date, payment);
    }
    let principal: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|f| f.kind == CFKind::Notional)
        .collect();
    assert_eq!(principal.len(), 2);
    assert_eq!(principal[0].date, base);
    close(principal[0].amount.amount(), -1_000_000.0, 0.0);
    close(principal[1].amount.amount(), 1_000_000.0, 0.0);
    assert_eq!(
        principal[1].date,
        schedule.coupons().last().expect("final coupon").date
    );
    fixed_amounts(schedule);
}

pub(super) fn floating_bullet(
    schedule: &CashFlowSchedule,
    base: Date,
    years: i32,
    coupons: usize,
    overnight: bool,
) {
    rows(schedule, coupons + 2);
    assert_eq!(schedule.get_meta().issue_date, Some(base));
    let maturity = base.add_months(years * 12).expect("fixture maturity");
    assert_eq!(schedule.get_meta().maturity, Some(maturity));
    assert_eq!(schedule.coupons().count(), coupons);
    let mut business_days = 0;
    let mut date = base;
    while date < maturity {
        business_days += usize::from(!date.is_weekend());
        date += time::Duration::days(1);
    }
    assert_eq!(
        schedule.get_meta().projected_fixings.len(),
        if overnight { business_days } else { coupons }
    );
    for fixing in &schedule.get_meta().projected_fixings {
        assert_eq!(fixing.series_id, "FIXING:USD-SOFR-3M");
        close(fixing.value.expect("projected raw fixing"), 0.045, 1e-11);
    }
    for (index, flow) in schedule.coupons().enumerate() {
        assert_eq!(flow.kind, CFKind::Floating);
        let accrual = flow.accrual.as_ref().expect("floating accrual");
        assert_eq!(accrual.day_count, DayCount::Act360);
        assert_eq!(
            accrual.start,
            base.add_months(index as i32 * 3).expect("quarter start")
        );
        assert_eq!(
            accrual.end,
            base.add_months((index as i32 + 1) * 3)
                .expect("quarter end")
        );
        let days = (accrual.end - accrual.start).whole_days() as f64;
        close(flow.accrual_factor, days / 360.0, 1e-12);
        let mut reset = accrual.start;
        if !overnight {
            for _ in 0..2 {
                reset -= time::Duration::days(1);
                while reset.is_weekend() {
                    reset -= time::Duration::days(1);
                }
            }
        }
        assert_eq!(flow.reset_date, Some(reset));
        let mut payment = accrual.end;
        while payment.is_weekend() {
            payment += time::Duration::days(1);
        }
        assert_eq!(flow.date, payment);
        let index_growth = if overnight {
            let mut growth = 1.0;
            let mut cursor = accrual.start;
            while cursor < accrual.end {
                let mut next = cursor + time::Duration::days(1);
                while next.is_weekend() {
                    next += time::Duration::days(1);
                }
                next = next.min(accrual.end);
                growth *= 1.0 + 0.045 * (next - cursor).whole_days() as f64 / 360.0;
                cursor = next;
            }
            growth - 1.0
        } else {
            0.045 * days / 360.0
        };
        close(
            flow.amount.amount(),
            1_000_000.0 * (index_growth + 0.02 * days / 360.0),
            1e-5,
        );
        close(
            flow.rate.expect("floating rate"),
            index_growth / (days / 360.0) + 0.02,
            1e-10,
        );
    }
    let principal: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|f| f.kind == CFKind::Notional)
        .collect();
    assert_eq!(principal.len(), 2);
    assert_eq!(principal[0].date, base);
    close(principal[0].amount.amount(), -1_000_000.0, 0.0);
    close(principal[1].amount.amount(), 1_000_000.0, 0.0);
    assert_eq!(
        principal[1].date,
        schedule.coupons().last().expect("final coupon").date
    );
}

pub(super) fn period_pv(
    schedule: &CashFlowSchedule,
    periods: &[Period],
    actual: &PeriodAggregation,
    market: &MarketContext,
    base: Date,
    credit: Option<f64>,
) {
    let discount = market.get_discount("USD-OIS").expect("fixture discount");
    let mut occupied = 0;
    for period in periods {
        let flows: Vec<_> = schedule
            .get_flows()
            .iter()
            .filter(|f| period.start <= f.date && f.date < period.end)
            .collect();
        if flows.is_empty() {
            assert!(!actual.contains_key(&period.id));
            continue;
        }
        occupied += 1;
        let mut expected = 0.0;
        for flow in flows {
            if flow.date <= base {
                continue;
            }
            let df = discount
                .df_between_dates(base, flow.date)
                .expect("fixture DF");
            let survival = if credit.is_some() {
                (-0.015 * (flow.date - base).whole_days() as f64 / 365.0).exp()
            } else {
                1.0
            };
            expected += flow.amount.amount() * df * survival;
            if let Some(recovery) = credit
                && flow.kind == CFKind::Notional
                && flow.amount.amount() > 0.0
            {
                expected += 1_000_000.0 * recovery * df * (1.0 - survival);
            }
        }
        let by_currency = actual.get(&period.id).expect("occupied reporting period");
        assert_eq!(by_currency.len(), 1);
        let amount = by_currency.get(&Currency::USD).expect("USD period total");
        assert_eq!(amount.currency(), Currency::USD);
        close(amount.amount(), expected, 1e-6);
    }
    assert_eq!(actual.len(), occupied);
}

pub(super) fn linear_accrued(schedule: &CashFlowSchedule, date: Date) -> f64 {
    schedule
        .coupons()
        .map(|flow| {
            let accrual = flow.accrual.as_ref().expect("fixture accrual");
            assert_eq!(accrual.day_count, DayCount::Act365F);
            if accrual.start <= date && date < flow.date {
                let elapsed = (date.min(accrual.end) - accrual.start).whole_days() as f64;
                flow.amount.amount() * elapsed / (accrual.end - accrual.start).whole_days() as f64
            } else {
                0.0
            }
        })
        .sum()
}

pub(super) fn amortization_path(schedule: &CashFlowSchedule, n: usize, base: Date) {
    rows(schedule, n);
    let path = schedule
        .outstanding_by_date()
        .expect("amortization balances");
    assert_eq!(path.len(), n);
    for (index, ((date, balance), flow)) in path.iter().zip(schedule.get_flows()).enumerate() {
        assert_eq!(*date, base + time::Duration::days((index as i64 + 1) * 90));
        assert_eq!(flow.kind, CFKind::Amortization);
        assert_eq!(balance.currency(), Currency::USD);
        close(flow.amount.amount(), 1_000_000.0 / n as f64, 1e-8);
        close(
            balance.amount(),
            1_000_000.0 * (1.0 - (index + 1) as f64 / n as f64),
            1e-6,
        );
    }
}
