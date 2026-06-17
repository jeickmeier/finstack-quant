//! Cross-currency swap (XCCY) pricing benchmarks.
//!
//! The XCCY swap pricer iterates over `n_periods` payment dates on both legs,
//! performing forward-rate projection, discount-factor lookup, and FX conversion
//! per cashflow. With 4Q/year coupon frequency on a 10Y tenor, that is 40 cash
//! flows per leg × 2 legs + 2 principal exchanges — comparable to a long IRS but
//! with two full curve lookups plus FxMatrix queries in the hot loop.
//!
//! No existing `rates_pricing.rs` bench covers XccySwap.
//!
//! Scenarios:
//! - Representative 5Y tenor (coupon count ≈ 20 per leg)
//! - Notional exchange: None / Final / Initial+Final
//! - Reporting in USD (one leg local, one converted via FxMatrix)

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::instruments::rates::xccy_swap::{
    LegSide, NotionalExchange, XccySwap, XccySwapLeg,
};
use finstack_quant_valuations::instruments::Instrument;
use rust_decimal::Decimal;
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

// ---------------------------------------------------------------------------
// Market context — 2 discount + 2 forward curves + FxMatrix
// ---------------------------------------------------------------------------

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 2).unwrap()
}

fn create_market() -> MarketContext {
    let base = base_date();

    let usd_disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (5.0, 0.80), (10.0, 0.63), (20.0, 0.40)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let eur_disc = DiscountCurve::builder("EUR-OIS")
        .base_date(base)
        .knots(vec![(0.0, 1.0), (5.0, 0.86), (10.0, 0.74), (20.0, 0.54)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let usd_fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots(vec![
            (0.0, 0.045),
            (5.0, 0.043),
            (10.0, 0.042),
            (20.0, 0.041),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let eur_fwd = ForwardCurve::builder("EUR-EURIBOR-3M", 0.25)
        .base_date(base)
        .knots(vec![
            (0.0, 0.035),
            (5.0, 0.034),
            (10.0, 0.033),
            (20.0, 0.032),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let provider = Arc::new(SimpleFxProvider::new());
    provider
        .set_quote(Currency::EUR, Currency::USD, 1.08)
        .expect("valid fx");
    let fx = FxMatrix::new(provider);

    MarketContext::new()
        .insert(usd_disc)
        .insert(eur_disc)
        .insert(usd_fwd)
        .insert(eur_fwd)
        .insert_fx(fx)
}

// ---------------------------------------------------------------------------
// Instrument builder
// ---------------------------------------------------------------------------

fn make_xccy(base: Date, maturity: Date, exchange: NotionalExchange) -> XccySwap {
    let usd_notional = 10_000_000.0_f64;
    let eur_notional = usd_notional / 1.08;

    let leg_usd = XccySwapLeg {
        currency: Currency::USD,
        notional: Money::new(usd_notional, Currency::USD),
        side: LegSide::Receive,
        forward_curve_id: CurveId::from("USD-SOFR-3M"),
        discount_curve_id: CurveId::from("USD-OIS"),
        start: base,
        end: maturity,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        calendar_id: None,
        reset_lag_days: None,
        allow_calendar_fallback: true,
    };

    let leg_eur = XccySwapLeg {
        currency: Currency::EUR,
        notional: Money::new(eur_notional, Currency::EUR),
        side: LegSide::Pay,
        forward_curve_id: CurveId::from("EUR-EURIBOR-3M"),
        discount_curve_id: CurveId::from("EUR-OIS"),
        start: base,
        end: maturity,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        calendar_id: None,
        reset_lag_days: None,
        allow_calendar_fallback: true,
    };

    XccySwap::new("XCCY-BENCH", leg_usd, leg_eur, Currency::USD).with_notional_exchange(exchange)
}

// ---------------------------------------------------------------------------
// Benchmark: tenor scaling (InitialAndFinal exchange)
// ---------------------------------------------------------------------------

fn bench_xccy_tenor(c: &mut Criterion) {
    let mut group = c.benchmark_group("xccy_tenor");
    let base = base_date();
    let market = create_market();

    let label = "5Y";
    let years = 5;
    let maturity = Date::from_calendar_date(2025 + years, Month::January, 2).unwrap();
    let swap = make_xccy(base, maturity, NotionalExchange::InitialAndFinal);
    let n_periods = years as u64 * 4 * 2; // 4 Q/yr × 2 legs

    group.throughput(Throughput::Elements(n_periods));
    group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
        b.iter(|| {
            black_box(&swap)
                .value(black_box(&market), black_box(base))
                .unwrap()
                .amount()
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: notional exchange convention (5Y swap)
// ---------------------------------------------------------------------------

fn bench_xccy_notional_exchange(c: &mut Criterion) {
    let mut group = c.benchmark_group("xccy_notional_exchange");
    let base = base_date();
    let maturity = Date::from_calendar_date(2030, Month::January, 2).unwrap();
    let market = create_market();

    for (label, exchange) in [
        ("none", NotionalExchange::None),
        ("final", NotionalExchange::Final),
        ("initial_and_final", NotionalExchange::InitialAndFinal),
    ] {
        let swap = make_xccy(base, maturity, exchange);
        group.bench_function(label, |b| {
            b.iter(|| {
                black_box(&swap)
                    .value(black_box(&market), black_box(base))
                    .unwrap()
                    .amount()
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_xccy_tenor, bench_xccy_notional_exchange);
criterion_main!(benches);
