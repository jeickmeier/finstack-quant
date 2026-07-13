//! Convertible bond pricing benchmarks.
//!
//! Measures performance of hybrid debt-equity valuation:
//! - NPV calculation with binomial and trinomial trees
//! - Tree convergence (different step counts)
//! - Greeks calculation (delta, gamma, vega, rho, theta)
//! - Parity and conversion premium
//! - Callable/puttable features
//! - Different moneyness levels (ITM, ATM, OTM)
//! - Volatility sensitivity
//!
//! Market Standards Review (Week 5)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_cashflows::builder::specs::{CouponType, FixedCouponSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    calculate_convertible_greeks, price_convertible_bond, ConvertibleTreeType,
};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, ConvertibleBond, DividendAdjustment,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use std::hint::black_box;
use time::Month;

// Standard test parameters
const NOTIONAL: f64 = 1000.0;
const CONVERSION_RATIO: f64 = 10.0;
const COUPON_RATE: f64 = 0.05;
const SPOT_PRICE: f64 = 150.0;
const SPOT_ATM: f64 = 100.0;
const VOL_STANDARD: f64 = 0.25;
const DIV_YIELD: f64 = 0.02;

fn issue_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn maturity_5y() -> Date {
    Date::from_calendar_date(2030, Month::January, 1).unwrap()
}

fn maturity_3y() -> Date {
    Date::from_calendar_date(2028, Month::January, 1).unwrap()
}

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn create_market_context(spot: f64, vol: f64, div_yield: f64) -> MarketContext {
    let base = base_date();

    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([(0.0, 1.0), (10.0, 0.741)]) // ~3% rate
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    MarketContext::new()
        .insert(discount_curve)
        .insert_price("AAPL", MarketScalar::Unitless(spot))
        .insert_price("AAPL-VOL", MarketScalar::Unitless(vol))
        .insert_price("AAPL-DIVYIELD", MarketScalar::Unitless(div_yield))
}

fn create_standard_convertible() -> ConvertibleBond {
    let conversion_spec = ConversionSpec {
        ratio: Some(CONVERSION_RATIO),
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    let fixed_coupon = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: rust_decimal::Decimal::from_f64_retain(COUPON_RATE).unwrap_or_default(),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::semi_annual(),

            dc: DayCount::Act365F,

            bdc: BusinessDayConvention::Following,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
        },
    };

    ConvertibleBond {
        id: "CONVERTIBLE".to_string().into(),
        notional: Money::new(NOTIONAL, Currency::USD),
        issue_date: issue_date(),
        maturity: maturity_5y(),
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: Some(fixed_coupon),
        floating_coupon: None,
        settlement_days: None,
        recovery_rate: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

fn create_callable_convertible() -> ConvertibleBond {
    let mut bond = create_standard_convertible();
    let mut call_put = CallPutSchedule::default();

    call_put.calls.push(CallPut {
        start_date: maturity_3y(),
        end_date: maturity_3y(),
        price_pct_of_par: 105.0,
        make_whole: None,
    });

    bond.call_put = Some(call_put);
    bond
}

fn create_zero_coupon_convertible() -> ConvertibleBond {
    let conversion_spec = ConversionSpec {
        ratio: Some(CONVERSION_RATIO),
        price: None,
        policy: ConversionPolicy::Voluntary,
        anti_dilution: AntiDilutionPolicy::None,
        dividend_adjustment: DividendAdjustment::None,
        dilution_events: Vec::new(),
    };

    ConvertibleBond {
        id: "ZERO_COUPON".to_string().into(),
        notional: Money::new(NOTIONAL, Currency::USD),
        issue_date: issue_date(),
        maturity: maturity_5y(),
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        conversion: conversion_spec,
        underlying_equity_id: Some("AAPL".to_string()),
        call_put: None,
        soft_call_trigger: None,
        fixed_coupon: None,
        floating_coupon: None,
        settlement_days: None,
        recovery_rate: None,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    }
}

// ============================================================================
// NPV Benchmarks - Tree Type Comparison
// ============================================================================

fn bench_npv_binomial(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_npv_binomial");
    let bond = create_standard_convertible();
    let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);

    {
        let steps = &100;
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}steps", steps)),
            steps,
            |b, &s| {
                b.iter(|| {
                    price_convertible_bond(
                        black_box(&bond),
                        black_box(&market),
                        black_box(ConvertibleTreeType::Binomial(s)),
                        base_date(),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_npv_trinomial(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_npv_trinomial");
    let bond = create_standard_convertible();
    let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);

    {
        let steps = &100;
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}steps", steps)),
            steps,
            |b, &s| {
                b.iter(|| {
                    price_convertible_bond(
                        black_box(&bond),
                        black_box(&market),
                        black_box(ConvertibleTreeType::Trinomial(s)),
                        base_date(),
                    )
                });
            },
        );
    }
    group.finish();
}

// ============================================================================
// NPV Benchmarks - Moneyness
// ============================================================================

fn bench_npv_by_moneyness(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_npv_moneyness");
    let bond = create_standard_convertible();

    {
        let (label, spot) = &("ATM", SPOT_ATM);
        let market = create_market_context(*spot, VOL_STANDARD, DIV_YIELD);
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                price_convertible_bond(
                    black_box(&bond),
                    black_box(&market),
                    black_box(ConvertibleTreeType::Binomial(50)),
                    base_date(),
                )
            });
        });
    }
    group.finish();
}

// ============================================================================
// NPV Benchmarks - Features
// ============================================================================

fn bench_npv_features(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_npv_features");
    let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);

    // Standard convertible
    let standard = create_standard_convertible();
    group.bench_function("standard", |b| {
        b.iter(|| {
            price_convertible_bond(
                black_box(&standard),
                black_box(&market),
                black_box(ConvertibleTreeType::Binomial(50)),
                base_date(),
            )
        });
    });

    // Callable convertible
    let callable = create_callable_convertible();
    group.bench_function("callable", |b| {
        b.iter(|| {
            price_convertible_bond(
                black_box(&callable),
                black_box(&market),
                black_box(ConvertibleTreeType::Binomial(50)),
                base_date(),
            )
        });
    });

    // Zero coupon
    let zero_coupon = create_zero_coupon_convertible();
    group.bench_function("zero_coupon", |b| {
        b.iter(|| {
            price_convertible_bond(
                black_box(&zero_coupon),
                black_box(&market),
                black_box(ConvertibleTreeType::Binomial(50)),
                base_date(),
            )
        });
    });

    group.finish();
}

// ============================================================================
// NPV Benchmarks - Volatility Sensitivity
// ============================================================================

fn bench_npv_volatility(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_npv_volatility");
    let bond = create_standard_convertible();

    {
        let (label, vol) = &("std", VOL_STANDARD);
        let market = create_market_context(SPOT_PRICE, *vol, DIV_YIELD);
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                price_convertible_bond(
                    black_box(&bond),
                    black_box(&market),
                    black_box(ConvertibleTreeType::Binomial(50)),
                    base_date(),
                )
            });
        });
    }
    group.finish();
}

// ============================================================================
// Greeks Benchmarks
// ============================================================================

fn bench_greeks_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_greeks");
    let bond = create_standard_convertible();
    let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);

    {
        let steps = &100;
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}steps", steps)),
            steps,
            |b, &s| {
                b.iter(|| {
                    calculate_convertible_greeks(
                        black_box(&bond),
                        black_box(&market),
                        black_box(ConvertibleTreeType::Binomial(s)),
                        black_box(Some(0.01)),
                        base_date(),
                    )
                });
            },
        );
    }
    group.finish();
}

fn bench_greeks_by_moneyness(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_greeks_moneyness");
    let bond = create_standard_convertible();

    {
        let (label, spot) = &("ATM", SPOT_ATM);
        let market = create_market_context(*spot, VOL_STANDARD, DIV_YIELD);
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| {
                calculate_convertible_greeks(
                    black_box(&bond),
                    black_box(&market),
                    black_box(ConvertibleTreeType::Binomial(50)),
                    black_box(Some(0.01)),
                    base_date(),
                )
            });
        });
    }
    group.finish();
}

// ============================================================================
// Metrics Benchmarks
// ============================================================================

fn bench_metrics_suite(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_metrics");
    let bond = create_standard_convertible();
    let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);
    let as_of = base_date();

    let metrics = vec![
        MetricId::Delta,
        MetricId::Gamma,
        MetricId::Vega,
        MetricId::Rho,
        MetricId::Theta,
    ];

    group.bench_function("full_suite", |b| {
        b.iter(|| {
            bond.price_with_metrics(
                black_box(&market),
                black_box(as_of),
                black_box(&metrics),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        });
    });

    group.finish();
}

// ============================================================================
// Parity Benchmarks
// ============================================================================

fn bench_parity_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_parity");
    let bond = create_standard_convertible();

    {
        let (label, spot) = &("ATM", SPOT_ATM);
        let market = create_market_context(*spot, VOL_STANDARD, DIV_YIELD);
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            b.iter(|| bond.parity(black_box(&market)));
        });
    }
    group.finish();
}

// ============================================================================
// Convergence Benchmarks
// ============================================================================

fn bench_tree_convergence(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_convergence");
    let bond = create_standard_convertible();
    let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);

    {
        let steps = &100;
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}steps", steps)),
            steps,
            |b, &s| {
                b.iter(|| {
                    price_convertible_bond(
                        black_box(&bond),
                        black_box(&market),
                        black_box(ConvertibleTreeType::Binomial(s)),
                        base_date(),
                    )
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_npv_binomial,
    bench_npv_trinomial,
    bench_npv_by_moneyness,
    bench_npv_features,
    bench_npv_volatility,
    bench_greeks_calculation,
    bench_greeks_by_moneyness,
    bench_metrics_suite,
    bench_parity_calculation,
    bench_tree_convergence
);
criterion_main!(benches);
