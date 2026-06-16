//! CDS Option pricing benchmarks.
//!
//! Measures performance of CDS option operations:
//! - Present value (NPV) calculation
//! - Greeks (delta, gamma, vega, theta)
//! - Implied volatility calculation
//!
//! Tests across multiple tenors and option types (calls/puts).

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve, Seniority};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::{
    CDSOption, CDSOptionParams, ProtectionStartConvention,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::{CreditParams, OptionType};
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

fn create_cds_option(
    option_type: OptionType,
    expiry_months: i32,
    cds_tenor_years: i32,
) -> CDSOption {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let expiry = base + time::Duration::days((expiry_months * 30) as i64);
    let cds_maturity = base + time::Duration::days((cds_tenor_years * 365) as i64);

    let option_params = CDSOptionParams {
        strike: Decimal::new(1, 2), // 0.01 = 100bp
        option_type,
        expiry,
        cds_maturity,
        notional: Money::new(10_000_000.0, Currency::USD),
        underlying_is_index: false,
        index_factor: None,
        underlying_cds_coupon: None,
        protection_start_convention: ProtectionStartConvention::Forward,
    };

    let credit_params = CreditParams {
        reference_entity: "ACME".to_string(),
        recovery_rate: 0.40,
        credit_curve_id: "ACME-HAZARD".into(),
    };

    CDSOption::new(
        format!("CDS_OPT_{}M_{}Y", expiry_months, cds_tenor_years),
        &option_params,
        &credit_params,
        "USD-OIS",
        "CDS-VOL",
    )
    .expect("valid CDS option setup")
}

fn create_index_cds_option() -> CDSOption {
    let mut option = create_cds_option(OptionType::Call, 1, 5);
    option.underlying_is_index = true;
    option.index_factor = Some(1.0);
    option.underlying_cds_coupon = Some(Decimal::new(1, 2));
    option.protection_start_convention = ProtectionStartConvention::Spot;
    option.knockout = false;
    option
}

fn create_market() -> MarketContext {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Discount curve
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.5, 0.975),
            (1.0, 0.95),
            (2.0, 0.91),
            (3.0, 0.87),
            (5.0, 0.78),
            (10.0, 0.60),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    // Hazard curve
    let hazard = HazardCurve::builder("ACME-HAZARD")
        .issuer("ACME")
        .seniority(Seniority::Senior)
        .currency(Currency::USD)
        .recovery_rate(0.40)
        .base_date(base)
        .knots([
            (0.0, 0.012), // 120bp hazard
            (0.5, 0.013),
            (1.0, 0.014),
            (2.0, 0.016),
            (3.0, 0.018),
            (5.0, 0.022),
            (10.0, 0.030),
        ])
        .build()
        .unwrap();

    // Volatility surface (flat 30% vol for simplicity)
    let vol_surface = VolSurface::from_grid(
        "CDS-VOL",
        &[0.25, 0.5, 1.0, 2.0, 3.0],         // Expiries in years
        &[0.005, 0.0075, 0.01, 0.015, 0.02], // Strikes as decimal rates
        &[0.30; 25],                         // Flat 30% vol
    )
    .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert(hazard)
        .insert_surface(vol_surface)
}

fn bench_cds_option_npv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_option_npv");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Benchmark across different tenors
    let test_cases = [
        (3, 5),   // 3M expiry, 5Y CDS
        (6, 5),   // 6M expiry, 5Y CDS
        (12, 5),  // 1Y expiry, 5Y CDS
        (12, 10), // 1Y expiry, 10Y CDS
    ];

    for &(expiry_months, cds_tenor) in &test_cases {
        let call = create_cds_option(OptionType::Call, expiry_months, cds_tenor);
        group.bench_with_input(
            BenchmarkId::new("call", format!("{}M_{}Y", expiry_months, cds_tenor)),
            &(expiry_months, cds_tenor),
            |b, _| {
                b.iter(|| call.value(black_box(&market), black_box(as_of)));
            },
        );

        let put = create_cds_option(OptionType::Put, expiry_months, cds_tenor);
        group.bench_with_input(
            BenchmarkId::new("put", format!("{}M_{}Y", expiry_months, cds_tenor)),
            &(expiry_months, cds_tenor),
            |b, _| {
                b.iter(|| put.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_cds_option_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_option_greeks");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let option = create_cds_option(OptionType::Call, 6, 5);

    group.bench_function("delta", |b| {
        b.iter(|| option.delta(black_box(&market), black_box(as_of)));
    });

    group.bench_function("gamma", |b| {
        b.iter(|| option.gamma(black_box(&market), black_box(as_of)));
    });

    group.bench_function("vega", |b| {
        b.iter(|| option.vega(black_box(&market), black_box(as_of)));
    });

    group.bench_function("theta", |b| {
        b.iter(|| option.theta(black_box(&market), black_box(as_of)));
    });

    group.finish();
}

fn bench_cds_option_all_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_option_all_greeks");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let option = create_cds_option(OptionType::Call, 6, 5);

    group.bench_function("all_greeks_sequential", |b| {
        b.iter(|| {
            let _delta = option.delta(black_box(&market), black_box(as_of));
            let _gamma = option.gamma(black_box(&market), black_box(as_of));
            let _vega = option.vega(black_box(&market), black_box(as_of));
            let _theta = option.theta(black_box(&market), black_box(as_of));
        });
    });

    group.finish();
}

fn bench_cds_option_implied_vol(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_option_implied_vol");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let option = create_cds_option(OptionType::Call, 6, 5);

    // Get a target price from NPV
    let target_price = option.value(&market, as_of).unwrap().amount();

    group.bench_function("implied_vol_solver", |b| {
        b.iter(|| {
            option.implied_vol(
                black_box(&market),
                black_box(as_of),
                black_box(target_price),
                black_box(Some(0.25)),
            )
        });
    });

    group.finish();
}

fn bench_cds_index_option_npv(c: &mut Criterion) {
    let mut group = c.benchmark_group("cds_index_option_npv");
    let market = create_market();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let option = create_index_cds_option();

    group.bench_function("index_payer_1M_5Y", |b| {
        b.iter(|| option.value(black_box(&market), black_box(as_of)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_cds_option_npv,
    bench_cds_option_greeks,
    bench_cds_option_all_greeks,
    bench_cds_option_implied_vol,
    bench_cds_index_option_npv
);
criterion_main!(benches);
