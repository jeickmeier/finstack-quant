//! Linear rates instrument pricing benchmarks.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::rates::basis_swap::{BasisSwap, BasisSwapLeg};
use finstack_quant_valuations::instruments::rates::cap_floor::{CapFloor, RateOptionType};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::rates::fra::ForwardRateAgreement;
use finstack_quant_valuations::instruments::rates::ir_future::{
    FutureContractSpecs, InterestRateFuture, Position,
};
use finstack_quant_valuations::instruments::rates::repo::{CollateralSpec, Repo};
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::instruments::{
    ExerciseStyle, Instrument, PayReceive, SettlementType,
};
use finstack_quant_valuations::metrics::MetricId;
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

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

/// USD-OIS discount, SOFR 3M/1M forwards, and a flat cap volatility surface.
fn create_rates_market() -> MarketContext {
    let base = base_date();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (0.25, 0.9875),
            (0.5, 0.975),
            (1.0, 0.95),
            (2.0, 0.90),
            (5.0, 0.78),
            (10.0, 0.60),
            (30.0, 0.35),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let fwd_3m = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 0.04),
            (1.0, 0.041),
            (2.0, 0.042),
            (5.0, 0.045),
            (10.0, 0.048),
            (30.0, 0.05),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let fwd_1m = ForwardCurve::builder("USD-SOFR-1M", 1.0 / 12.0)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 0.039),
            (1.0, 0.040),
            (2.0, 0.041),
            (5.0, 0.044),
            (10.0, 0.047),
            (30.0, 0.049),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let vol = 0.30_f64;
    let cap_vol = VolSurface::builder("USD-CAP-VOL")
        .expiries(&[0.25, 1.0, 5.0, 10.0])
        .strikes(&[0.01, 0.03, 0.05, 0.07, 0.10])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert(fwd_3m)
        .insert(fwd_1m)
        .insert_surface(cap_vol)
        .insert_price(
            "TREASURY_BOND_PRICE",
            MarketScalar::Price(Money::new(1.02, Currency::USD)),
        )
}

fn deposit(id: &str, maturity: Date) -> Deposit {
    Deposit::builder()
        .id(InstrumentId::new(id))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(base_date())
        .maturity(maturity)
        .day_count(DayCount::Act360)
        .quote_rate_opt(Some(Decimal::try_from(0.04).unwrap()))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .unwrap()
}

fn fra(id: &str, start: Date, end: Date) -> ForwardRateAgreement {
    ForwardRateAgreement {
        id: InstrumentId::new(id),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(start),
        start_date: start,
        maturity: end,
        fixed_rate: Decimal::try_from(0.042).unwrap(),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        side: PayReceive::Receive,
        pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn basis_swap(id: &str, end: Date) -> BasisSwap {
    const CAL: &str = "usny";
    let start = base_date();
    BasisSwap::new(
        id,
        Money::new(10_000_000.0, Currency::USD),
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CAL.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
        BasisSwapLeg {
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CAL.to_string()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
        },
    )
    .unwrap()
}

fn interest_rate_cap(id: &str, maturity: Date) -> CapFloor {
    CapFloor {
        id: InstrumentId::new(id),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.04).unwrap(),
        start_date: base_date(),
        maturity,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        vol_surface_id: CurveId::new("USD-CAP-VOL"),
        vol_type: Default::default(),
        vol_shift: 0.0,
        pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn term_repo() -> Repo {
    let collateral = CollateralSpec::new("TREASURY_BOND", 1_000_000.0, "TREASURY_BOND_PRICE");
    Repo::term(
        "REPO-BENCH",
        Money::new(1_000_000.0, Currency::USD),
        collateral,
        0.05,
        test_utils::date(2025, 1, 15),
        test_utils::date(2025, 4, 15),
        "USD-OIS",
    )
    .unwrap()
}

fn ir_future() -> InterestRateFuture {
    let start = test_utils::date(2025, 7, 1);
    let end = test_utils::date(2025, 10, 1);
    InterestRateFuture {
        id: InstrumentId::new("IRF-BENCH"),
        notional: Money::new(1_000_000.0, Currency::USD),
        expiry: start,
        fixing_date: Some(start),
        period_start: Some(start),
        period_end: Some(end),
        quoted_price: 97.50,
        day_count: DayCount::Act360,
        position: Position::Long,
        contract_specs: FutureContractSpecs {
            convexity_adjustment: Some(0.0),
            ..FutureContractSpecs::default()
        },
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        vol_surface_id: None,
        pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn bench_deposit_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("deposit_pv");
    let market = create_rates_market();
    let as_of = base_date();
    let maturities = [
        ("1M", test_utils::date(2025, 2, 1)),
        ("3M", test_utils::date(2025, 4, 1)),
        ("6M", test_utils::date(2025, 7, 1)),
        ("1Y", test_utils::date(2026, 1, 1)),
    ];
    for (label, mat) in maturities {
        let dep = deposit(&format!("DEP-{label}"), mat);
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| dep.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn bench_fra_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("fra_pv");
    let market = create_rates_market();
    let as_of = base_date();
    let cases = [
        (
            "3x6",
            test_utils::date(2025, 4, 1),
            test_utils::date(2025, 7, 1),
        ),
        (
            "6x9",
            test_utils::date(2025, 7, 1),
            test_utils::date(2025, 10, 1),
        ),
        (
            "6x12",
            test_utils::date(2025, 7, 1),
            test_utils::date(2026, 1, 1),
        ),
    ];
    for (label, start, end) in cases {
        let f = fra(&format!("FRA-{label}"), start, end);
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| f.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn bench_basis_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("basis_swap_pv");
    let market = create_rates_market();
    let as_of = base_date();
    let cases = [
        ("2Y", test_utils::date(2027, 1, 1)),
        ("5Y", test_utils::date(2030, 1, 1)),
        ("10Y", test_utils::date(2035, 1, 1)),
    ];
    for (label, end) in cases {
        let swap = basis_swap(&format!("BASIS-{label}"), end);
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| swap.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn bench_cap_floor_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cap_floor_pv");
    let market = create_rates_market();
    let as_of = base_date();
    for (label, mat) in [
        ("2Y", test_utils::date(2027, 1, 1)),
        ("5Y", test_utils::date(2030, 1, 1)),
    ] {
        let cap = interest_rate_cap(&format!("CAP-{label}"), mat);
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| cap.value(black_box(&market), black_box(as_of)));
        });
    }
    group.finish();
}

fn bench_cap_floor_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("cap_floor_greeks");
    let market = create_rates_market();
    let as_of = base_date();
    let cap = interest_rate_cap("CAP-5Y-GREEKS", test_utils::date(2030, 1, 1));
    let metrics = [
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Dv01,
    ];
    group.bench_function("5Y_cap", |b| {
        b.iter(|| {
            cap.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&metrics),
                PricingOptions::default(),
            )
        });
    });
    group.finish();
}

fn bench_repo_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_pv");
    let market = create_rates_market();
    let as_of = test_utils::date(2025, 1, 10);
    let repo = term_repo();
    group.bench_function("term_repo", |b| {
        b.iter(|| repo.value(black_box(&market), black_box(as_of)));
    });
    group.finish();
}

fn bench_ir_future_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_future_pv");
    let market = create_rates_market();
    let as_of = base_date();
    let fut = ir_future();
    group.bench_function("quarterly_future", |b| {
        b.iter(|| fut.value(black_box(&market), black_box(as_of)));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_deposit_pv,
    bench_fra_pv,
    bench_basis_swap_pv,
    bench_cap_floor_pv,
    bench_cap_floor_greeks,
    bench_repo_pv,
    bench_ir_future_pv
);
criterion_main!(benches);
