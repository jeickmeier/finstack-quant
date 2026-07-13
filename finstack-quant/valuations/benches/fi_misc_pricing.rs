//! Fixed income misc pricing benchmarks.
//!
//! Covers:
//! - [`TermLoan`]: fixed/floating coupon schedule with amortization.
//! - [`RevolvingCredit`]: deterministic draw/repay schedule, floating coupon.
//! - [`AgencyMbsPassthrough`] / [`AgencyTba`]: prepayment-adjusted cashflows.
//! - [`FIIndexTotalReturnSwap`]: carry-based TRS with yield/duration market data.
//! - [`AgencyCmo`]: sequential and PAC waterfall allocation across collateral pools.
//! - [`DollarRoll`]: front/back month settlement NPV and implied financing rate.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_cashflows::builder::specs::CouponType;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::cmo::AgencyCmo;
use finstack_quant_valuations::instruments::fixed_income::dollar_roll::DollarRoll;
use finstack_quant_valuations::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap;
use finstack_quant_valuations::instruments::fixed_income::fi_trs::TrsScheduleSpec;
use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::{
    BaseRateSpec, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
};
use finstack_quant_valuations::instruments::fixed_income::tba::AgencyTba;
use finstack_quant_valuations::instruments::fixed_income::term_loan::{
    AmortizationSpec, RateSpec, TermLoan,
};
use finstack_quant_valuations::instruments::Instrument;
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
mod test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}

fn as_of() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn term_loan(maturity: Date) -> TermLoan {
    let base = as_of();
    TermLoan::builder()
        .id("TL-BENCH".into())
        .currency(Currency::USD)
        .notional_limit(Money::new(10_000_000.0, Currency::USD))
        .issue_date(base)
        .maturity(maturity)
        .rate(RateSpec::Fixed { rate_bp: 500 })
        .frequency(Tenor::semi_annual())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::ModifiedFollowing)
        .calendar_id_opt(None)
        .stub(StubKind::None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .amortization(AmortizationSpec::None)
        .coupon_type(CouponType::Cash)
        .upfront_fee_opt(None)
        .ddtl_opt(None)
        .covenants_opt(None)
        .pricing_overrides(Default::default())
        .attributes(Default::default())
        .build()
        .unwrap()
}

fn revolving_credit_floating(maturity: Date) -> RevolvingCredit {
    RevolvingCredit::builder()
        .id("RC-BENCH".into())
        .commitment_amount(Money::new(10_000_000.0, Currency::USD))
        .drawn_amount(Money::new(5_000_000.0, Currency::USD))
        .commitment_date(as_of())
        .maturity(maturity)
        .base_rate_spec(BaseRateSpec::Floating(
            finstack_quant_cashflows::builder::FloatingRateSpec {
                index_id: "USD-SOFR-3M".into(),
                spread_bp: Decimal::try_from(100.0).unwrap(),
                gearing: Decimal::try_from(1.0).unwrap(),
                gearing_includes_spread: true,
                index_floor_bp: None,
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                index_cap_bp: None,
                overnight_index_constraints: Default::default(),
                reset_freq: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 2,
                fixing_calendar_id: None,
                overnight_compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            },
        ))
        .day_count(DayCount::Act360)
        .frequency(Tenor::quarterly())
        .fees(RevolvingCreditFees::flat(25.0, 10.0, 0.0))
        .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
        .discount_curve_id("USD-OIS".into())
        .build()
        .unwrap()
}

fn revolving_credit_market(as_of: Date) -> MarketContext {
    let disc = test_utils::flat_discount("USD-OIS", as_of, 0.05);
    let fwd = finstack_quant_core::market_data::term_structures::ForwardCurve::builder(
        "USD-SOFR-3M",
        0.25,
    )
    .base_date(as_of)
    .knots([
        (0.0, 0.04),
        (1.0, 0.045),
        (3.0, 0.048),
        (5.0, 0.05),
        (7.0, 0.052),
    ])
    .build()
    .unwrap();
    MarketContext::new().insert(disc).insert(fwd)
}

fn bench_term_loan_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("term_loan_pv");
    let as_of = as_of();
    let disc = test_utils::flat_discount("USD-OIS", as_of, 0.05);
    let market = MarketContext::new().insert(disc);

    for (label, maturity) in [
        (
            "3Y",
            Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        ),
        (
            "5Y",
            Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        ),
        (
            "7Y",
            Date::from_calendar_date(2032, Month::January, 1).unwrap(),
        ),
    ] {
        let loan = term_loan(maturity);
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| loan.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn bench_revolving_credit_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("revolving_credit_pv");
    let as_of = as_of();
    let market = revolving_credit_market(as_of);

    for (label, maturity) in [
        (
            "3Y",
            Date::from_calendar_date(2028, Month::January, 1).unwrap(),
        ),
        (
            "5Y",
            Date::from_calendar_date(2030, Month::January, 1).unwrap(),
        ),
    ] {
        let facility = revolving_credit_floating(maturity);
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| facility.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn bench_agency_mbs_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("agency_mbs_pv");
    let as_of = as_of();
    let disc = test_utils::flat_discount("USD-OIS", as_of, 0.05);
    let market = MarketContext::new().insert(disc);

    let mbs = AgencyMbsPassthrough::example().unwrap();
    group.bench_function("passthrough_example", |b| {
        b.iter(|| mbs.value(black_box(&market), black_box(as_of)));
    });

    let tba = AgencyTba::example().unwrap();
    group.bench_function("tba_example", |b| {
        b.iter(|| tba.value(black_box(&market), black_box(as_of)));
    });

    group.finish();
}

// ================================================================================================
// FI Index TRS
// ================================================================================================

/// Market context for FI TRS: discount, forward, and scalar yield/duration data.
fn fi_trs_market(as_of: Date) -> MarketContext {
    let disc = test_utils::flat_discount("USD-OIS", as_of, 0.05);
    let fwd = finstack_quant_core::market_data::term_structures::ForwardCurve::builder(
        "USD-SOFR-3M",
        0.25,
    )
    .base_date(as_of)
    .knots([(0.0, 0.04), (1.0, 0.045), (5.0, 0.05)])
    .build()
    .unwrap();
    MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_price("US-CORP-INDEX", MarketScalar::Unitless(100.0))
        .insert_price("US-CORP-YIELD", MarketScalar::Unitless(0.055))
        .insert_price("US-CORP-DURATION", MarketScalar::Unitless(5.5))
        .insert_price("US-CORP-CONVEXITY", MarketScalar::Unitless(0.30))
}

/// Scale FI TRS pricing vs tenor (1Y, 3Y, 5Y).
///
/// Measures the carry-based TRS model: per-period yield computation, financing
/// leg floating-rate projection, and discounting over the payment schedule.
fn bench_fi_trs_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("fi_trs_pv");
    let as_of = as_of();
    let market = fi_trs_market(as_of);

    for (label, years) in [("1Y", 1), ("3Y", 3), ("5Y", 5)] {
        let maturity = Date::from_calendar_date(as_of.year() + years, as_of.month(), 1).unwrap();
        let mut trs = FIIndexTotalReturnSwap::example().unwrap();
        trs.schedule = TrsScheduleSpec::from_params(
            as_of,
            maturity,
            finstack_quant_cashflows::builder::ScheduleParams {
                freq: Tenor::quarterly(),
                dc: DayCount::Act360,
                bdc: BusinessDayConvention::Following,
                calendar_id: "weekends_only".to_string(),
                stub: StubKind::None,
                end_of_month: false,
                payment_lag_days: 0,
                adjust_accrual_dates: false,
            },
        );
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| black_box(trs.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

// ================================================================================================
// CMO
// ================================================================================================

/// Scale CMO waterfall allocation vs tranche count (3, 5, 8 tranches).
///
/// Measures the sequential/PAC waterfall distribution over a monthly cashflow
/// projection horizon. Tranche count increases the inner allocation loop.
fn bench_cmo_waterfall_pv(c: &mut Criterion) {
    use finstack_quant_valuations::instruments::fixed_income::cmo::{CmoTranche, CmoWaterfall};
    use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyProgram;
    use time::macros::date;

    let mut group = c.benchmark_group("cmo_waterfall_pv");
    let as_of = as_of();
    let disc = test_utils::flat_discount("USD-OIS", as_of, 0.05);
    let market = MarketContext::new().insert(disc);

    {
        let (label, n_tranches) = ("5t", 5_usize);
        let tranches: Vec<CmoTranche> = (0..n_tranches)
            .map(|i| {
                // Keep total coupon demand under the ~4% collateral
                // pass-through so the deal passes interest-coverage
                // validation at build.
                let coupon = 0.03 + 0.001 * i as f64;
                let balance = 10_000_000.0;
                CmoTranche::sequential(
                    format!("T{}", i).as_str(),
                    Money::new(balance, Currency::USD),
                    coupon,
                    (i + 1) as u32,
                )
            })
            .collect();

        let cmo = AgencyCmo::builder()
            .id(InstrumentId::new("CMO-BENCH"))
            .deal_name("BENCH-CMO".into())
            .agency(AgencyProgram::Fnma)
            .issue_date(date!(2024 - 01 - 01))
            .waterfall(CmoWaterfall::new(tranches))
            .reference_tranche_id("T0".to_string())
            .collateral_wac(0.045)
            .collateral_wam(360)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| black_box(cmo.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

/// Compare CMO structure types: sequential, PAC/support, IO/PO.
fn bench_cmo_structure_type(c: &mut Criterion) {
    let mut group = c.benchmark_group("cmo_structure_type");
    let as_of = as_of();
    let disc = test_utils::flat_discount("USD-OIS", as_of, 0.05);
    let market = MarketContext::new().insert(disc);

    let sequential = AgencyCmo::example().unwrap();
    group.bench_function("sequential", |b| {
        b.iter(|| black_box(sequential.value(black_box(&market), black_box(as_of))).unwrap())
    });

    let pac = AgencyCmo::example_pac_support().unwrap();
    group.bench_function("pac_support", |b| {
        b.iter(|| black_box(pac.value(black_box(&market), black_box(as_of))).unwrap())
    });

    let io_po = AgencyCmo::example_io_po().unwrap();
    group.bench_function("io_po", |b| {
        b.iter(|| black_box(io_po.value(black_box(&market), black_box(as_of))).unwrap())
    });

    group.finish();
}

// ================================================================================================
// Dollar Roll
// ================================================================================================

/// Dollar roll NPV and implied-financing computation.
///
/// Benchmarks the two-leg settlement NPV (front vs. back month price
/// difference discounted to as_of) plus the implied repo rate calculation.
fn bench_dollar_roll_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("dollar_roll_pv");
    let as_of = as_of();
    let disc = test_utils::flat_discount("USD-OIS", as_of, 0.05);
    let market = MarketContext::new().insert(disc);

    for (label, drop_bp) in [
        ("tight_10bp", 0.10_f64),
        ("wide_50bp", 0.50),
        ("neg_neg10bp", -0.10),
    ] {
        let front_price = 98.50_f64;
        let back_price = front_price - drop_bp;
        let roll = DollarRoll::builder()
            .id(InstrumentId::new("ROLL-BENCH"))
            .agency(finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyProgram::Fnma)
            .coupon(0.04)
            .term(finstack_quant_valuations::instruments::fixed_income::tba::TbaTerm::ThirtyYear)
            .notional(Money::new(10_000_000.0, Currency::USD))
            .front_settlement_year(2025)
            .front_settlement_month(3)
            .back_settlement_year(2025)
            .back_settlement_month(4)
            .front_price(front_price)
            .back_price(back_price)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .build()
            .unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| black_box(roll.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_term_loan_pv,
    bench_revolving_credit_pv,
    bench_agency_mbs_pv,
    bench_fi_trs_pv,
    bench_cmo_waterfall_pv,
    bench_cmo_structure_type,
    bench_dollar_roll_pv,
);
criterion_main!(benches);
