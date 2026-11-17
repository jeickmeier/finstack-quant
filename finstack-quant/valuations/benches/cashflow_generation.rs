//! Cashflow generation benchmarks.
//!
//! Measures performance of:
//! - Fixed-rate schedule building
//! - Bond and floating/fixed swap cashflow generation
//! - Compensated aggregation of dated cashflows
//!
//! Market Standards Review (Week 5)

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_cashflows::aggregation::aggregate_cashflows_checked;
use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, FixedCouponSpec};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../rates/tests/support/rates.rs"]
mod rates_support;
use rust_decimal_macros::dec;
use std::hint::black_box;
use time::Month;
mod support;

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (5.0, 0.88),
            (10.0, 0.70),
            (30.0, 0.40),
        ])
        .interp(InterpStyle::Linear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.994996932244156),
            (0.25, 0.98998795756821),
            (0.375, 0.984973417976586),
            (0.5, 0.979953654267085),
            (0.625, 0.9749290059945),
            (0.75, 0.969899811434621),
            (0.875, 0.964866407548615),
            (1.0, 0.959829129947799),
            (1.25, 0.949781389200023),
            (1.5, 0.939794776900668),
            (1.75, 0.929869581606853),
            (2.0, 0.920006080244019),
            (2.25, 0.910204538177125),
            (2.5, 0.900465209283782),
            (2.75, 0.890788336029286),
            (3.0, 0.881174149543515),
            (3.25, 0.871622869699645),
            (3.5, 0.862134705194628),
            (3.75, 0.852709853631428),
            (4.0, 0.843348501602928),
            (4.25, 0.8340508247775),
            (4.5, 0.824816987986182),
            (4.75, 0.815647145311418),
            (5.0, 0.806541440177327),
            (5.3125, 0.795239995999149),
            (5.625, 0.784020342031173),
            (5.9375, 0.772883500109301),
            (6.25, 0.761830453516174),
            (6.5625, 0.750862147147463),
            (6.875, 0.739979487694401),
            (7.1875, 0.729183343842207),
            (7.5, 0.718474546484074),
            (7.8125, 0.707853888950355),
            (8.125, 0.697322127252603),
            (8.4375, 0.686879980342104),
            (8.75, 0.676528130382526),
            (9.0625, 0.666267223036341),
            (9.375, 0.656097867764614),
            (9.6875, 0.646020638139812),
            (10.0, 0.63603607217124),
            (11.25, 0.597383906679029),
            (12.5, 0.560861519355261),
            (13.75, 0.526366356993483),
            (15.0, 0.49379985288271),
            (16.25, 0.463067330834553),
            (17.5, 0.434077907770486),
            (18.75, 0.406744395179397),
            (20.0, 0.380983199739337),
            (21.25, 0.356714223381208),
            (22.5, 0.333860763056229),
            (23.75, 0.312349410453224),
            (25.0, 0.292109951896311),
            (26.25, 0.273075268638354),
            (27.5, 0.255181237750667),
            (28.75, 0.238366633794927),
            (30.0, 0.222573031449104),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    MarketContext::new().insert(disc).insert(fwd)
}

fn validate_dated_cashflows(flows: &[(Date, Money)], as_of: Date) {
    assert!(!flows.is_empty());
    assert!(flows.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    for (date, amount) in flows {
        assert!(*date >= as_of);
        assert_eq!(amount.currency(), Currency::USD);
        assert!(amount.amount().is_finite());
    }
}

fn bench_bond_cashflow_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("bond_cashflow_generation");
    let tenor = 5;
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{}Y", tenor)),
        &tenor,
        |b, _| {
            let market = create_market();
            let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
            let maturity = Date::from_calendar_date(2025 + tenor, Month::January, 1).unwrap();
            let bond = Bond::fixed(
                format!("BOND-{}Y", tenor),
                Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
                finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
                issue,
                maturity,
                StubKind::ShortFront,
                "USD-OIS",
            )
            .expect("valid benchmark bond");
            support::bench(
                b,
                || {
                    bond.dated_cashflows(black_box(&market), black_box(issue))
                        .expect("bond cashflow generation")
                },
                |flows| {
                    validate_dated_cashflows(flows, issue);
                    assert!(
                        flows
                            .iter()
                            .any(|(_, amount)| amount.amount() >= 1_000_000.0)
                    );
                    assert!(
                        flows
                            .iter()
                            .any(|(_, amount)| amount.amount() > 0.0
                                && amount.amount() < 1_000_000.0)
                    );
                },
            );
        },
    );
    group.finish();
}

fn bench_swap_cashflow_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("swap_cashflow_generation");
    let tenor = 5;
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{}Y", tenor)),
        &tenor,
        |b, _| {
            let market = create_market();
            let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
            let end = Date::from_calendar_date(2025 + tenor, Month::January, 1).unwrap();
            let swap = rates_support::usd_irs_swap(
                InstrumentId::new(format!("IRS-{}Y", tenor)),
                Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
                0.04,
                start,
                end,
                finstack_quant_valuations::instruments::PayReceive::Pay,
            )
            .expect("valid benchmark swap");
            support::bench(
                b,
                || {
                    swap.dated_cashflows(black_box(&market), black_box(start))
                        .expect("swap cashflow generation")
                },
                |flows| {
                    validate_dated_cashflows(flows, start);
                    assert_eq!(
                        flows.len(),
                        30,
                        "ten semiannual and twenty quarterly coupons"
                    );
                    assert!(flows.iter().any(|(_, amount)| amount.amount() < 0.0));
                    assert!(flows.iter().any(|(_, amount)| amount.amount() > 0.0));
                },
            );
        },
    );
    group.finish();
}

fn bench_schedule_builder_fixed(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_builder_fixed");
    let tenor = 5;
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{}Y", tenor)),
        &tenor,
        |b, _| {
            let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
            let maturity = Date::from_calendar_date(2025 + tenor, Month::January, 1).unwrap();
            let fixed_spec = FixedCouponSpec {
                coupon_type: CouponType::Cash,
                rate: dec!(0.05),
                schedule: finstack_quant_cashflows::builder::ScheduleParams {
                    frequency: Tenor::semi_annual(),
                    day_count: DayCount::Thirty360,
                    business_day_convention: BusinessDayConvention::Following,
                    calendar_id: "weekends_only".into(),
                    stub: StubKind::None,
                    end_of_month: false,
                    payment_lag_days: 0,
                    adjust_accrual_dates: false,
                    roll_rule: finstack_quant_cashflows::builder::RollRule::None,
                },
            };
            let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
            support::bench(
                b,
                || {
                    let mut builder = CashFlowSchedule::builder();
                    let _ = builder
                        .principal(black_box(init), black_box(issue), black_box(maturity))
                        .fixed_cf(black_box(fixed_spec.clone()));
                    builder.build(None).expect("fixed cashflow schedule")
                },
                |schedule| {
                    schedule.validate().expect("valid generated schedule");
                    assert_eq!(schedule.get_day_count(), DayCount::Thirty360);
                    let flows = schedule.get_flows();
                    assert_eq!(
                        flows
                            .iter()
                            .filter(|flow| flow.get_kind() == CFKind::Fixed)
                            .count(),
                        10
                    );
                    assert!(
                        flows
                            .windows(2)
                            .all(|pair| pair[0].get_date() <= pair[1].get_date())
                    );
                    for flow in flows {
                        assert_eq!(flow.get_amount().currency(), Currency::USD);
                        assert!(flow.get_amount().amount().is_finite());
                    }
                },
            );
        },
    );
    group.finish();
}

fn bench_compensated_summation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compensated_summation");
    let num_flows = 50;
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{}flows", num_flows)),
        &num_flows,
        |b, _| {
            let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
            let flows: Vec<(Date, Money)> = (0..num_flows)
                .map(|i| {
                    let date = base + time::Duration::days(i * 6 * 30);
                    (
                        date,
                        Money::new(1000.0, Currency::USD).expect("valid money fixture"),
                    )
                })
                .collect();
            support::bench(
                b,
                || {
                    aggregate_cashflows_checked(black_box(&flows), Currency::USD)
                        .expect("cashflow aggregation")
                },
                |total| {
                    assert_eq!(total.currency(), Currency::USD);
                    assert_eq!(total.amount(), 50_000.0);
                },
            );
        },
    );
    group.finish();
}

criterion_group!(
    benches,
    bench_bond_cashflow_generation,
    bench_swap_cashflow_generation,
    bench_schedule_builder_fixed,
    bench_compensated_summation
);
criterion_main!(benches);
