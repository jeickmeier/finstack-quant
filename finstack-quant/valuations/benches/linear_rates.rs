//! Linear rates instrument pricing benchmarks.

#![allow(clippy::unwrap_used)]

mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{MarketScalar, ScalarTimeSeries};
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::types::ExerciseStyle;
use finstack_quant_valuations::instruments::FloatLegSpec;
use finstack_quant_valuations::instruments::PricingOptions;
use finstack_quant_valuations::instruments::rates::basis_swap::BasisSwap;
use finstack_quant_valuations::instruments::rates::cap_floor::{CapFloor, RateOptionType};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::rates::fra::ForwardRateAgreement;
use finstack_quant_valuations::instruments::rates::ir_future::{
    FutureContractSpecs, InterestRateFuture,
};
use finstack_quant_valuations::instruments::rates::repo::{CollateralSpec, Repo};
use finstack_quant_valuations::instruments::{Instrument, PayReceive, SettlementType};
use finstack_quant_valuations::instruments::{ListedFutureTerms, Position};
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::ModelKey;
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

use finstack_quant_valuations_test_support::date as date_support;

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
    // The next live caplet fixes before one quarter of an ACT/365F year
    // because the reference index preserves its two-business-day reset lag.
    let first_expiry = DayCount::Act365F
        .year_fraction(
            base,
            base + time::Duration::days(1),
            DayCountContext::default(),
        )
        .expect("cap surface expiry year fraction");
    let cap_vol = VolSurface::builder("USD-CAP-VOL")
        .expiries(&[first_expiry, 0.25, 1.0, 5.0, 10.0])
        .strikes(&[0.01, 0.03, 0.05, 0.07, 0.10])
        .row(&[vol, vol, vol, vol, vol])
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
            MarketScalar::Price(Money::new(1.02, Currency::USD).expect("valid money fixture")),
        )
}

fn create_cap_market() -> MarketContext {
    let fixings = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![(date_support::date(2024, 12, 30), 0.04)],
        None,
    )
    .expect("valid historical cap fixing");
    create_rates_market().insert_series(fixings)
}

fn assert_cap_fixing_required(cap: &CapFloor, as_of: Date) {
    let error = cap
        .value(&create_rates_market(), as_of)
        .expect_err("fixed-but-unpaid caplet must require its historical fixing")
        .to_string();
    assert!(error.contains("FIXING:USD-SOFR-3M"));
    assert!(error.contains("2024-12-30"));
}

fn deposit(id: &str, maturity: Date) -> Deposit {
    Deposit::builder()
        .id(InstrumentId::new(id))
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .start_date(base_date())
        .maturity(maturity)
        .day_count(DayCount::Act360)
        .fixed_rate_opt(Some(Decimal::try_from(0.04).unwrap()))
        .discount_curve_id(CurveId::new("USD-OIS"))
        .build()
        .unwrap()
}

fn fra(id: &str, start: Date, end: Date) -> ForwardRateAgreement {
    ForwardRateAgreement {
        id: InstrumentId::new(id),
        notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        fixing_date: Some(start),
        start_date: start,
        maturity: end,
        fixed_rate: Decimal::try_from(0.042).unwrap(),
        day_count: DayCount::Act360,
        reset_lag_days: 2,
        fixing_calendar_id: None,
        fixing_business_day_convention: None,
        observed_fixing: None,
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        side: PayReceive::Receive,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn basis_swap(id: &str, end: Date) -> BasisSwap {
    const CAL: &str = "usny";
    let start = base_date();
    BasisSwap::new(
        id,
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            forward_curve_id: CurveId::new("USD-SOFR-3M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CAL.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
        FloatLegSpec {
            end_of_month: false,
            fixing_calendar_id: None,
            forward_curve_id: CurveId::new("USD-SOFR-1M"),
            discount_curve_id: CurveId::new("USD-OIS"),
            start,
            end,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some(CAL.into()),
            stub: StubKind::ShortFront,
            spread_bp: Decimal::ZERO,
            payment_lag_days: 0,
            reset_lag_days: 0,
            compounding: Default::default(),
        },
    )
    .unwrap()
}

fn interest_rate_cap(id: &str, maturity: Date) -> CapFloor {
    CapFloor {
        id: InstrumentId::new(id),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        strike: Decimal::try_from(0.04).unwrap(),
        start_date: base_date(),
        maturity,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        vol_surface_id: CurveId::new("USD-CAP-VOL"),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        premium: None,
        spread_bp: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn term_repo() -> Repo {
    let collateral = CollateralSpec::new("TREASURY_BOND", 1_000_000.0, "TREASURY_BOND_PRICE");
    Repo::term(
        "REPO-BENCH",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        collateral,
        0.05,
        date_support::date(2025, 1, 15),
        date_support::date(2025, 4, 15),
        "USD-OIS",
    )
    .unwrap()
}

fn ir_future() -> InterestRateFuture {
    let start = date_support::date(2025, 7, 1);
    let end = date_support::date(2025, 10, 1);
    InterestRateFuture {
        id: InstrumentId::new("IRF-BENCH"),
        terms: ListedFutureTerms::new(
            1.0,
            2_500.0,
            Currency::USD,
            97.50,
            start,
            start,
            Position::Long,
        )
        .expect("valid listed terms"),
        fixing_date: Some(start),
        period_start: Some(start),
        period_end: Some(end),
        day_count: DayCount::Act360,
        contract_specs: FutureContractSpecs {
            convexity_adjustment: Some(0.0),
            ..FutureContractSpecs::default()
        },
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        compounding: FloatingLegCompounding::Simple,
        index_id: None,
        fixing_calendar_id: None,
        vol_surface_id: None,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn bench_deposit_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("deposit_pv");
    let as_of = base_date();
    let maturities = [("3M", date_support::date(2025, 4, 1))];
    for (label, mat) in maturities {
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            let market = create_rates_market();
            let dep = deposit(&format!("DEP-{label}"), mat);
            assert_eq!(dep.default_model(), ModelKey::Discounting);
            support::bench(
                b,
                || {
                    dep.value(black_box(&market), black_box(as_of))
                        .expect("deposit pricing must succeed")
                },
                |value| {
                    assert!(value.amount().is_finite());
                    assert_eq!(value.currency(), Currency::USD);
                },
            );
        });
    }
    group.finish();
}

fn bench_fra_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("fra_pv");
    let as_of = base_date();
    let cases = [(
        "6x9",
        date_support::date(2025, 7, 1),
        date_support::date(2025, 10, 1),
    )];
    for (label, start, end) in cases {
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            let market = create_rates_market();
            let f = fra(&format!("FRA-{label}"), start, end);
            assert_eq!(f.default_model(), ModelKey::Discounting);
            support::bench(
                b,
                || {
                    f.value(black_box(&market), black_box(as_of))
                        .expect("FRA pricing must succeed")
                },
                |value| {
                    assert!(value.amount().is_finite());
                    assert_eq!(value.currency(), Currency::USD);
                },
            );
        });
    }
    group.finish();
}

fn bench_basis_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("basis_swap_pv");
    let as_of = base_date();
    let cases = [("5Y", date_support::date(2030, 1, 1))];
    for (label, end) in cases {
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            let market = create_rates_market();
            let swap = basis_swap(&format!("BASIS-{label}"), end);
            assert_eq!(swap.default_model(), ModelKey::Discounting);
            support::bench(
                b,
                || {
                    swap.value(black_box(&market), black_box(as_of))
                        .expect("basis swap pricing must succeed")
                },
                |value| {
                    assert!(value.amount().is_finite());
                    assert_eq!(value.currency(), Currency::USD);
                },
            );
        });
    }
    group.finish();
}

fn bench_cap_floor_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cap_floor_pv");
    let as_of = base_date();
    {
        let (label, mat) = ("5Y", date_support::date(2030, 1, 1));
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            let market = create_cap_market();
            let cap = interest_rate_cap(&format!("CAP-{label}"), mat);
            assert_eq!(cap.default_model(), ModelKey::Black76);
            assert_cap_fixing_required(&cap, as_of);
            support::bench(
                b,
                || {
                    cap.value(black_box(&market), black_box(as_of))
                        .expect("cap pricing must succeed")
                },
                |value| {
                    assert!(value.amount().is_finite());
                    assert_eq!(value.currency(), Currency::USD);
                },
            );
        });
    }
    group.finish();
}

fn bench_cap_floor_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("cap_floor_greeks");
    let as_of = base_date();
    let metrics = [
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Dv01,
    ];
    group.bench_function("5Y_cap", |b| {
        let market = create_cap_market();
        let cap = interest_rate_cap("CAP-5Y-GREEKS", date_support::date(2030, 1, 1));
        assert_eq!(cap.default_model(), ModelKey::Black76);
        assert_cap_fixing_required(&cap, as_of);
        support::bench(
            b,
            || {
                cap.price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&metrics),
                    PricingOptions::default(),
                )
                .expect("cap Greek pricing must succeed")
            },
            |result| {
                assert!(result.value.amount().is_finite());
                assert_eq!(result.value.currency(), Currency::USD);
                for metric in &metrics {
                    assert!(
                        result
                            .measures
                            .get(metric)
                            .expect("requested cap Greek must be present")
                            .is_finite()
                    );
                }
            },
        );
    });
    group.finish();
}

fn bench_repo_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("repo_pv");
    let as_of = date_support::date(2025, 1, 10);
    group.bench_function("term_repo", |b| {
        let market = create_rates_market();
        let repo = term_repo();
        assert_eq!(repo.default_model(), ModelKey::Discounting);
        support::bench(
            b,
            || {
                repo.value(black_box(&market), black_box(as_of))
                    .expect("repo pricing must succeed")
            },
            |value| {
                assert!(value.amount().is_finite());
                assert_eq!(value.currency(), Currency::USD);
            },
        );
    });
    group.finish();
}

fn bench_ir_future_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_future_pv");
    let as_of = base_date();
    group.bench_function("quarterly_future", |b| {
        let market = create_rates_market();
        let fut = ir_future();
        assert_eq!(fut.default_model(), ModelKey::Discounting);
        assert_eq!(fut.compounding, FloatingLegCompounding::Simple);
        support::bench(
            b,
            || {
                fut.value(black_box(&market), black_box(as_of))
                    .expect("IR future pricing must succeed")
            },
            |value| {
                assert!(value.amount().is_finite());
                assert_eq!(value.currency(), Currency::USD);
            },
        );
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
