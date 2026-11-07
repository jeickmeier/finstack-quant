//! Convertible bond pricing benchmarks.
//!
//! Measures performance of hybrid debt-equity valuation:
//! - NPV calculation with the binomial tree
//! - Tree convergence (different step counts)
//! - Greeks calculation (delta, gamma, vega, rho, theta)
//! - Parity and conversion premium
//! - Callable/puttable features
//! - Different moneyness levels (ITM, ATM, OTM)
//! - Volatility sensitivity
//!
//! Market Standards Review (Week 5)

mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::builder::{CouponType, FixedCouponSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::GreekBumps;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::bond::{CallPut, CallPutSchedule};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    ConvertibleBond, ConvertibleGreeks,
};
use finstack_quant_valuations::instruments::fixed_income::convertible::{
    calculate_convertible_greeks, price_convertible_bond,
};
use finstack_quant_valuations::instruments::{
    AntiDilutionPolicy, ConversionPolicy, ConversionSpec, DividendAdjustment,
};
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

/// Set the convertible lattice size (`model_config.tree_steps`).
fn with_tree_steps(mut bond: ConvertibleBond, steps: usize) -> ConvertibleBond {
    bond.instrument_pricing_overrides.model_config.tree_steps = Some(steps);
    bond
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
            frequency: Tenor::semi_annual(),

            day_count: DayCount::Act365F,

            business_day_convention: BusinessDayConvention::Following,

            calendar_id: "weekends_only".into(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    };

    ConvertibleBond {
        id: "CONVERTIBLE".to_string().into(),
        notional: Money::new(NOTIONAL, Currency::USD).expect("valid money fixture"),
        issue_date: issue_date(),
        maturity: maturity_5y(),
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        conversion: conversion_spec,
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL-VOL".into(),
        div_yield_id: Some("AAPL-DIVYIELD".into()),
        call_put: None,
        soft_call_trigger: None,
        resolved_settlement: None,
        cashflow_spec:
            finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec::Fixed(
                fixed_coupon,
            ),
        settlement_days: None,
        recovery_rate: None,
        instrument_pricing_overrides:
            finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn create_callable_convertible() -> ConvertibleBond {
    let mut bond = create_standard_convertible();
    let mut call_put = CallPutSchedule::default();

    call_put.calls.push(CallPut {
        start: maturity_3y(),
        end: maturity_3y(),
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
        notional: Money::new(NOTIONAL, Currency::USD).expect("valid money fixture"),
        issue_date: issue_date(),
        maturity: maturity_5y(),
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        conversion: conversion_spec,
        spot_id: "AAPL".into(),
        vol_surface_id: "AAPL-VOL".into(),
        div_yield_id: Some("AAPL-DIVYIELD".into()),
        call_put: None,
        soft_call_trigger: None,
        resolved_settlement: None,
        cashflow_spec:
            finstack_quant_valuations::instruments::fixed_income::bond::CashflowSpec::default(), // zero coupon
        settlement_days: None,
        recovery_rate: None,
        instrument_pricing_overrides:
            finstack_quant_valuations::instruments::pricing_overrides::InstrumentPricingOverrides::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

// NPV Benchmarks - Tree Type Comparison

fn validate_money(value: &Money) {
    assert!(value.amount().is_finite());
    assert_eq!(value.currency(), Currency::USD);
}

fn validate_greeks(greeks: &ConvertibleGreeks) {
    for value in [
        greeks.price,
        greeks.delta,
        greeks.gamma,
        greeks.vega,
        greeks.rho,
        greeks.theta,
    ] {
        assert!(value.is_finite());
    }
}

fn bench_npv_binomial(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_npv_binomial");

    {
        let steps = &100;
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}steps", steps)),
            steps,
            |b, &s| {
                let bond = with_tree_steps(create_standard_convertible(), s);
                let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);
                support::bench(
                    b,
                    || {
                        price_convertible_bond(black_box(&bond), black_box(&market), base_date())
                            .expect("convertible binomial price")
                    },
                    validate_money,
                );
            },
        );
    }
    group.finish();
}

// NPV Benchmarks - Moneyness

fn bench_npv_by_moneyness(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_npv_moneyness");

    {
        let (label, spot) = &("ATM", SPOT_ATM);
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            let bond = with_tree_steps(create_standard_convertible(), 50);
            let market = create_market_context(*spot, VOL_STANDARD, DIV_YIELD);
            support::bench(
                b,
                || {
                    price_convertible_bond(black_box(&bond), black_box(&market), base_date())
                        .expect("convertible moneyness price")
                },
                validate_money,
            );
        });
    }
    group.finish();
}

// NPV Benchmarks - Features

fn bench_npv_features(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_npv_features");

    // Standard convertible
    group.bench_function("standard", |b| {
        let standard = with_tree_steps(create_standard_convertible(), 50);
        let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);
        support::bench(
            b,
            || {
                price_convertible_bond(black_box(&standard), black_box(&market), base_date())
                    .expect("standard convertible price")
            },
            validate_money,
        );
    });

    // Callable convertible
    group.bench_function("callable", |b| {
        let callable = with_tree_steps(create_callable_convertible(), 50);
        let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);
        assert!(
            !callable
                .call_put
                .as_ref()
                .expect("callable schedule")
                .calls
                .is_empty()
        );
        support::bench(
            b,
            || {
                price_convertible_bond(black_box(&callable), black_box(&market), base_date())
                    .expect("callable convertible price")
            },
            validate_money,
        );
    });

    // Zero coupon
    group.bench_function("zero_coupon", |b| {
        let zero_coupon = with_tree_steps(create_zero_coupon_convertible(), 50);
        let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);
        support::bench(
            b,
            || {
                price_convertible_bond(black_box(&zero_coupon), black_box(&market), base_date())
                    .expect("zero-coupon convertible price")
            },
            validate_money,
        );
    });

    group.finish();
}

// NPV Benchmarks - Volatility Sensitivity

fn bench_npv_volatility(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_npv_volatility");

    {
        let (label, vol) = &("std", VOL_STANDARD);
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            let bond = with_tree_steps(create_standard_convertible(), 50);
            let market = create_market_context(SPOT_PRICE, *vol, DIV_YIELD);
            support::bench(
                b,
                || {
                    price_convertible_bond(black_box(&bond), black_box(&market), base_date())
                        .expect("convertible volatility price")
                },
                validate_money,
            );
        });
    }
    group.finish();
}

// Greeks Benchmarks

fn bench_greeks_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_greeks");

    {
        let steps = &100;
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}steps", steps)),
            steps,
            |b, &s| {
                let bond = with_tree_steps(create_standard_convertible(), s);
                let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);
                assert_eq!(bond.notional.currency(), Currency::USD);
                support::bench(
                    b,
                    || {
                        calculate_convertible_greeks(
                            black_box(&bond),
                            black_box(&market),
                            black_box(GreekBumps::default()),
                            base_date(),
                        )
                        .expect("convertible Greeks")
                    },
                    validate_greeks,
                );
            },
        );
    }
    group.finish();
}

fn bench_greeks_by_moneyness(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_greeks_moneyness");

    {
        let (label, spot) = &("ATM", SPOT_ATM);
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            let bond = with_tree_steps(create_standard_convertible(), 50);
            let market = create_market_context(*spot, VOL_STANDARD, DIV_YIELD);
            assert_eq!(bond.notional.currency(), Currency::USD);
            support::bench(
                b,
                || {
                    calculate_convertible_greeks(
                        black_box(&bond),
                        black_box(&market),
                        black_box(GreekBumps::default()),
                        base_date(),
                    )
                    .expect("convertible moneyness Greeks")
                },
                validate_greeks,
            );
        });
    }
    group.finish();
}

// Metrics Benchmarks

fn bench_metrics_suite(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_metrics");

    group.bench_function("full_suite", |b| {
        let bond = create_standard_convertible();
        let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);
        let as_of = base_date();
        let metrics = [
            MetricId::Delta,
            MetricId::Gamma,
            MetricId::Vega,
            MetricId::Rho,
            MetricId::Theta,
        ];
        support::bench(
            b,
            || {
                bond.price_with_metrics(
                    black_box(&market),
                    black_box(as_of),
                    black_box(&metrics),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("convertible metrics")
            },
            |result| {
                validate_money(&result.value);
                for metric in &metrics {
                    assert!(
                        result
                            .measures
                            .get(metric)
                            .expect("requested convertible metric")
                            .is_finite()
                    );
                }
            },
        );
    });

    group.finish();
}

// Parity Benchmarks

fn bench_parity_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_parity");

    {
        let (label, spot) = &("ATM", SPOT_ATM);
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            let bond = create_standard_convertible();
            let market = create_market_context(*spot, VOL_STANDARD, DIV_YIELD);
            support::bench(
                b,
                || bond.parity(black_box(&market)).expect("convertible parity"),
                |parity| {
                    assert!(parity.is_finite());
                    assert!((parity - spot * CONVERSION_RATIO / NOTIONAL).abs() < 1e-12);
                },
            );
        });
    }
    group.finish();
}

// Convergence Benchmarks

fn bench_tree_convergence(c: &mut Criterion) {
    let mut group = c.benchmark_group("convertible_convergence");

    {
        let steps = &100;
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}steps", steps)),
            steps,
            |b, &s| {
                let bond = with_tree_steps(create_standard_convertible(), s);
                let market = create_market_context(SPOT_PRICE, VOL_STANDARD, DIV_YIELD);
                support::bench(
                    b,
                    || {
                        price_convertible_bond(black_box(&bond), black_box(&market), base_date())
                            .expect("convertible convergence price")
                    },
                    validate_money,
                );
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_npv_binomial,
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
