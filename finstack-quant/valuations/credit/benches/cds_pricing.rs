//! CDS pricing benchmarks (calibration-free PV, prepared reuse, and par spread).
//!
//! Calibrated CS01 benchmarks live in `finstack-quant-calibration`'s
//! `credit_cs01_pricing` target.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{Attributes, CurveId, InstrumentId};
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{Instrument, PayReceive, PricingOptions};
use finstack_quant_valuations_core::market::conventions::CdsConvention;
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_core::results::ValuationResult;
use finstack_quant_valuations_credit::cds::CreditDefaultSwap;
use finstack_quant_valuations_credit::cds::pricing::PreparedCds;
use finstack_quant_valuations_credit::constants::isda::STANDARD_RECOVERY_SENIOR;
use finstack_quant_valuations_credit::{CdsDateRegime, PremiumLegSpec, ProtectionLegSpec};
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

#[path = "../../benches/support/mod.rs"]
mod support;

fn create_cds(tenor_years: i32) -> CreditDefaultSwap {
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let maturity = Date::from_calendar_date(2025 + tenor_years, Month::January, 1).unwrap();
    let convention = CdsConvention::IsdaNa;
    let spec = convention.get_spec().expect("CDS registry");

    let cds = CreditDefaultSwap::builder()
        .include_valuation_date_cashflows(false)
        .id(InstrumentId::new(format!("CDS-{}Y", tenor_years)))
        .notional(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
        .side(PayReceive::Pay)
        .convention(convention)
        .premium_leg(PremiumLegSpec {
            date_regime: CdsDateRegime::Standard,
            start,
            end: maturity,
            frequency: spec.frequency,
            stub: spec.stub,
            business_day_convention: spec.business_day_convention,
            calendar_id: Some(spec.calendar_id.as_str().into()),
            day_count: spec.day_count,
            coupon_bp: Decimal::from(100),
            discount_curve_id: CurveId::new("USD-OIS"),
        })
        .protection_leg(ProtectionLegSpec {
            credit_curve_id: CurveId::new("ACME-HAZARD"),
            recovery_rate: STANDARD_RECOVERY_SENIOR,
            settlement_delay: spec.settlement_days,
        })
        .instrument_pricing_overrides(InstrumentPricingOverrides::default())
        .attributes(Attributes::new())
        .build()
        .expect("valid CDS fixture");
    cds.validate().expect("valid CDS");
    cds
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (3.0, 0.94),
            (5.0, 0.88),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::Linear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let hazard = HazardCurve::builder("ACME-HAZARD")
        .base_date(base)
        .recovery_rate(STANDARD_RECOVERY_SENIOR)
        .knots([
            (1.0, 0.0100 / 0.60),
            (3.0, 0.0110 / 0.60),
            (5.0, 0.0120 / 0.60),
            (10.0, 0.0150 / 0.60),
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

fn validate_metrics(result: &ValuationResult, metrics: &[MetricId]) {
    assert_eq!(result.value.currency(), Currency::USD);
    assert!(result.value.amount().is_finite());
    assert!(result.measures.values().all(|value| value.is_finite()));
    for metric in metrics {
        assert!(result.measures.contains_key(metric), "missing {metric}");
    }
}

fn bench_cds_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_pv");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let market = create_market();
                let cds = create_cds(*tenor);
                support::bench(
                    b,
                    || {
                        cds.value(black_box(&market), black_box(as_of))
                            .expect("CDS PV")
                    },
                    |value| {
                        assert_eq!(value.currency(), Currency::USD);
                        assert!(value.amount().is_finite());
                    },
                );
            },
        );
    }
    group.finish();
}

fn bench_cds_par_spread(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_par_spread");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    for tenor in [1, 3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}Y", tenor)),
            tenor,
            |b, _| {
                let market = create_market();
                let cds = create_cds(*tenor);
                support::bench(
                    b,
                    || {
                        cds.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&[MetricId::ParSpread]),
                            PricingOptions::default(),
                        )
                        .expect("CDS par spread")
                    },
                    |result| validate_metrics(result, &[MetricId::ParSpread]),
                );
            },
        );
    }
    group.finish();
}

fn bench_cds_prepared(c: &mut Criterion) {
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("as of");
    let cds = create_cds(5);
    let hazard = market.get_hazard("ACME-HAZARD").expect("hazard");
    let prepared = PreparedCds::new(&cds, &market, as_of).expect("prepare");
    let mut group = c.benchmark_group("cds_prepared");
    group.bench_function("prepare_only_5Y", |b| {
        b.iter(|| {
            PreparedCds::new(black_box(&cds), black_box(&market), black_box(as_of))
                .expect("prepare")
        })
    });
    group.bench_function("reuse_5Y", |b| {
        b.iter(|| prepared.price(black_box(&hazard)).expect("price"))
    });
    group.bench_function("prepare_and_price_5Y", |b| {
        b.iter(|| {
            PreparedCds::new(black_box(&cds), black_box(&market), black_box(as_of))
                .expect("prepare")
                .price(black_box(&hazard))
                .expect("price")
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_cds_pv,
    bench_cds_prepared,
    bench_cds_par_spread
);
criterion_main!(benches);
