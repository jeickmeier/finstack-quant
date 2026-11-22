//! Linear rates instrument pricing benchmarks.

#![allow(clippy::unwrap_used)]

#[path = "../../benches/support/mod.rs"]
mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, Date, DayCount, DayCountContext, StubKind, Tenor,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{MarketScalar, ScalarTimeSeries};
use finstack_quant_core::market_data::surfaces::{
    VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_models::types::ExerciseStyle;
use finstack_quant_valuations_core::instruments::PricingOptions;
use finstack_quant_valuations_core::instruments::{Instrument, PayReceive, SettlementType};
use finstack_quant_valuations_core::instruments::{ListedFutureTerms, Position};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::pricer::ModelKey;
use finstack_quant_valuations_rates::FloatLegSpec;
use finstack_quant_valuations_rates::basis_swap::BasisSwap;
use finstack_quant_valuations_rates::cap_floor::{CapFloor, RateOptionType};
use finstack_quant_valuations_rates::deposit::Deposit;
use finstack_quant_valuations_rates::fra::ForwardRateAgreement;
use finstack_quant_valuations_rates::ir_future::{FutureContractSpecs, InterestRateFuture};
use finstack_quant_valuations_rates::repo::{CollateralSpec, Repo};
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
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let fwd_3m = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (0.125, 0.995004705688054),
            (0.25, 0.990018895175281),
            (0.375, 0.985042676354874),
            (0.5, 0.980076156130075),
            (0.625, 0.975119440412489),
            (0.75, 0.970172634120479),
            (0.875, 0.965235841177649),
            (1.0, 0.960309164511413),
            (1.125, 0.955392706051647),
            (1.25, 0.950486566729423),
            (1.375, 0.945590846475839),
            (1.5, 0.940705644220913),
            (1.625, 0.935831057892582),
            (1.75, 0.93096718441577),
            (1.875, 0.926114119711543),
            (2.0, 0.921271958696349),
            (2.1875, 0.914029366746096),
            (2.375, 0.906811831866448),
            (2.5625, 0.89961966166733),
            (2.75, 0.892453158659783),
            (2.9375, 0.885312620254042),
            (3.125, 0.878198338758511),
            (3.3125, 0.871110601379638),
            (3.5, 0.864049690222695),
            (3.6875, 0.857015882293436),
            (3.875, 0.850009449500634),
            (4.0625, 0.843030658659494),
            (4.25, 0.83607977149593),
            (4.4375, 0.829157044651693),
            (4.625, 0.822262729690352),
            (4.8125, 0.815397073104109),
            (5.0, 0.808560316321452),
            (5.3125, 0.797246154248255),
            (5.625, 0.786044252301273),
            (5.9375, 0.774954336604495),
            (6.25, 0.763976117592417),
            (6.5625, 0.753109290298775),
            (6.875, 0.742353534645681),
            (7.1875, 0.731708515733044),
            (7.5, 0.721173884128187),
            (7.8125, 0.71074927615553),
            (8.125, 0.700434314186274),
            (8.4375, 0.690228606927944),
            (8.75, 0.680131749713727),
            (9.0625, 0.670143324791486),
            (9.375, 0.660262901612357),
            (9.6875, 0.650490037118847),
            (10.0, 0.640824276032319),
            (11.25, 0.603458428395665),
            (12.5, 0.568182561960748),
            (13.75, 0.534885205108605),
            (15.0, 0.503460510444308),
            (16.25, 0.47380798940494),
            (17.5, 0.445832258377),
            (18.75, 0.419442795878017),
            (20.0, 0.394553710371601),
            (21.25, 0.371083518299286),
            (22.5, 0.348954931926299),
            (23.75, 0.328094656611859),
            (25.0, 0.308433197127742),
            (26.25, 0.289904672661664),
            (27.5, 0.27244664015452),
            (28.75, 0.25599992563267),
            (30.0, 0.240508463208342),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let fwd_1m = ForwardCurve::builder("USD-SOFR-1M")
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (0.125, 0.995129089050063),
            (0.25, 0.990266430839744),
            (0.375, 0.985412136627979),
            (0.5, 0.98056631673808),
            (0.625, 0.975729080555446),
            (0.75, 0.97090053652535),
            (0.875, 0.96608079215082),
            (1.0, 0.961269953990598),
            (1.125, 0.956468127657184),
            (1.25, 0.951675417814965),
            (1.375, 0.946891928178425),
            (1.5, 0.94211776151044),
            (1.625, 0.937353019620651),
            (1.75, 0.93259780336393),
            (1.875, 0.927852212638915),
            (2.0, 0.923116346386636),
            (2.1875, 0.916030994467999),
            (2.375, 0.90896806948576),
            (2.5625, 0.90192789320895),
            (2.75, 0.894910782530104),
            (2.9375, 0.887917049458652),
            (3.125, 0.880947001115197),
            (3.3125, 0.874000939726705),
            (3.5, 0.867079162622589),
            (3.6875, 0.860181962231675),
            (3.875, 0.853309626080054),
            (4.0625, 0.846462436789815),
            (4.25, 0.83964067207863),
            (4.4375, 0.832844604760224),
            (4.625, 0.826074502745679),
            (4.8125, 0.819330629045595),
            (5.0, 0.812613241773098),
            (5.3125, 0.801492794593498),
            (5.625, 0.790478210035502),
            (5.9375, 0.779569315137611),
            (6.25, 0.768765920870817),
            (6.5625, 0.758067822404332),
            (6.875, 0.747474799372235),
            (7.1875, 0.736986616140945),
            (7.5, 0.726603022077397),
            (7.8125, 0.716323751817831),
            (8.125, 0.706148525537082),
            (8.4375, 0.696077049218283),
            (8.75, 0.686109014922863),
            (9.0625, 0.676244101060762),
            (9.375, 0.666481972660743),
            (9.6875, 0.656822281640725),
            (10.0, 0.647264667078035),
            (11.25, 0.61028566692597),
            (12.5, 0.575329418782605),
            (13.75, 0.542290672593492),
            (15.0, 0.511069341670116),
            (16.25, 0.481570266927862),
            (17.5, 0.453702990941912),
            (18.75, 0.427381541461501),
            (20.0, 0.402524224033636),
            (21.25, 0.379053423397887),
            (22.5, 0.356895413324134),
            (23.75, 0.335980174575277),
            (25.0, 0.316241220686766),
            (26.25, 0.297615431264564),
            (27.5, 0.280042892512572),
            (28.75, 0.263466744709875),
            (30.0, 0.247833036367229),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .expiries(&[first_expiry, 0.25, 1.0, 5.0, 10.0])
        .secondary_values(&[0.01, 0.03, 0.05, 0.07, 0.10])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .row(&[vol, vol, vol, vol, vol])
        .build()
        .unwrap();

    MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd_3m)
        .expect("insert market curve")
        .try_insert(fwd_1m)
        .expect("insert market curve")
        .insert_surface(cap_vol)
        .insert_price(
            "TREASURY_BOND_PRICE",
            MarketScalar::price(Money::new(1.02, Currency::USD).expect("valid money fixture")),
        )
}

fn create_cap_market() -> MarketContext {
    let fixings = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![(date_support::date(2024, 12, 30), 0.04)],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
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
            index_id: "USD-SOFR-3M".into(),
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
            index_id: "USD-SOFR-1M".into(),
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
