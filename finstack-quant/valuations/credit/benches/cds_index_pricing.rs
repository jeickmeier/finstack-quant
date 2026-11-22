//! CDS Index pricing benchmarks (calibration-free PV, par spread, and risky PV01).
//!
//! Calibrated CS01 and CS01-including metrics benchmarks live in
//! `finstack-quant-calibration`'s `credit_cs01_pricing` target.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::instruments::{Instrument, PayReceive};
use finstack_quant_valuations_credit::CreditParams;
use finstack_quant_valuations_credit::cds::CdsTradeParams;
use finstack_quant_valuations_credit::cds_index::{
    CdsIndex, CdsIndexConstituent, CdsIndexParams, IndexPricing,
};
use std::hint::black_box;
use time::Month;

#[path = "../../benches/support/mod.rs"]
mod support;

fn create_cds_index_single_curve(tenor_years: i32) -> CdsIndex {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();

    let preset = CdsIndexParams::cdx_na_ig(42, 1, 100.0);

    CdsIndex::from_preset(
        &preset,
        CdsTradeParams {
            id: format!("CDX-{}Y", tenor_years).into(),
            notional: Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
            side: PayReceive::Pay,
            include_valuation_date_cashflows: false,
            start,
            end,
            recovery_rate: 0.40,
            discount_curve_id: CurveId::new("USD-OIS"),
            credit_curve_id: CurveId::new("CDX-HAZARD"),
        },
    )
    .expect("valid benchmark parameters")
}

fn create_cds_index_constituents(tenor_years: i32, num_names: usize) -> CdsIndex {
    let mut index = create_cds_index_single_curve(tenor_years);

    let weight = 1.0 / (num_names as f64);
    let mut constituents = Vec::with_capacity(num_names);

    for i in 0..num_names {
        constituents.push(CdsIndexConstituent {
            credit: CreditParams {
                reference_entity: format!("ISSUER-{:03}", i + 1),
                credit_curve_id: format!("HAZARD-{:03}", i + 1).into(),
                recovery_rate: 0.40,
            },
            weight,
            defaulted: false,
        });
    }

    index.constituents = constituents;
    index.pricing = IndexPricing::Constituents;
    index
}

fn create_market_single_curve() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.96),
            (5.0, 0.88),
            (10.0, 0.70),
        ])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("CDX-HAZARD")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([
            (1.0, 0.0120 / 0.60),
            (3.0, 0.0150 / 0.60),
            (5.0, 0.0180 / 0.60),
            (10.0, 0.0210 / 0.60),
        ])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .build()
        .unwrap();

    MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(hazard)
        .expect("insert market curve")
}

fn create_market_constituents(num_names: usize) -> MarketContext {
    let mut market = create_market_single_curve();
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for i in 0..num_names {
        let scale = 1.0 + ((i as f64) / (num_names as f64) - 0.5) * 0.2;
        let hazard = HazardCurve::builder(format!("HAZARD-{:03}", i + 1))
            .base_date(base)
            .knots([
                (1.0, 0.02 * scale),
                (3.0, 0.025 * scale),
                (5.0, 0.03 * scale),
                (10.0, 0.035 * scale),
            ])
            .recovery_rate(0.40)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .build()
            .unwrap();
        market = market.try_insert(hazard).expect("insert market curve");
    }

    market
}

fn validate_price(value: &Money) {
    assert_eq!(value.currency(), Currency::USD);
    assert!(value.amount().is_finite());
}

fn bench_cds_index_pv_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_pv_single");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let index = create_cds_index_single_curve(*tenor);
                let market = create_market_single_curve();
                assert_eq!(index.pricing, IndexPricing::SingleCurve);
                support::bench(
                    b,
                    || {
                        index
                            .value(black_box(&market), black_box(as_of))
                            .expect("CDS index value")
                    },
                    validate_price,
                );
            },
        );
    }
    group.finish();
}

fn bench_cds_index_pv_constituents(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_pv_constituents");
    let tenor = 5;
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let num_names = &50;
    group.bench_with_input(
        BenchmarkId::from_parameter(format!("{}names", num_names)),
        num_names,
        |b, _| {
            let index = create_cds_index_constituents(tenor, *num_names);
            let market = create_market_constituents(*num_names);
            assert_eq!(index.pricing, IndexPricing::Constituents);
            assert_eq!(index.constituents.len(), *num_names);
            support::bench(
                b,
                || {
                    index
                        .value(black_box(&market), black_box(as_of))
                        .expect("CDS index value")
                },
                validate_price,
            );
        },
    );
    group.finish();
}

fn bench_cds_index_par_spread(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_par_spread");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let index = create_cds_index_single_curve(*tenor);
                let market = create_market_single_curve();
                assert_eq!(index.pricing, IndexPricing::SingleCurve);
                support::bench(
                    b,
                    || {
                        index
                            .par_spread(black_box(&market), black_box(as_of))
                            .expect("CDS index par_spread")
                    },
                    |value| assert!(value.is_finite()),
                );
            },
        );
    }
    group.finish();
}

fn bench_cds_index_risky_pv01(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_risky_pv01");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let index = create_cds_index_single_curve(*tenor);
                let market = create_market_single_curve();
                assert_eq!(index.pricing, IndexPricing::SingleCurve);
                support::bench(
                    b,
                    || {
                        index
                            .risky_pv01(black_box(&market), black_box(as_of))
                            .expect("CDS index risky_pv01")
                    },
                    |value| assert!(value.is_finite()),
                );
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cds_index_pv_single,
    bench_cds_index_pv_constituents,
    bench_cds_index_par_spread,
    bench_cds_index_risky_pv01
);
criterion_main!(benches);
