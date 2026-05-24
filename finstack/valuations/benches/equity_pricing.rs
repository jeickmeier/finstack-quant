//! Equity non-option instrument pricing benchmarks.
//!
//! Covers:
//! - [`Equity`]: spot equity PV.
//! - [`EquityTotalReturnSwap`]: floating financing vs equity total return.
//! - [`EquityIndexFuture`]: cost-of-carry future pricing.
//! - [`VarianceSwap`]: fair-strike via Carr-Madan replication.
//! - [`VolatilityIndexFuture`]: VIX-style forward contract.
//! - [`VolatilityIndexOption`]: options on volatility indices.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_cashflows::builder::ScheduleParams;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::market_data::surfaces::VolSurface;
use finstack_core::market_data::term_structures::{
    DiscountCurve, ForwardCurve, VolatilityIndexCurve,
};
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::equity::equity_index_future::{
    EquityFutureSpecs, EquityIndexFuture,
};
use finstack_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap;
use finstack_valuations::instruments::equity::variance_swap::{
    PayReceive, RealizedVarMethod, VarianceSwap,
};
use finstack_valuations::instruments::equity::vol_index_future::{
    VolIndexContractSpecs, VolatilityIndexFuture,
};
use finstack_valuations::instruments::equity::vol_index_option::{
    VolIndexOptionSpecs, VolatilityIndexOption,
};
use finstack_valuations::instruments::equity::Equity;
use finstack_valuations::instruments::rates::ir_future::Position;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::instruments::{Attributes, ExerciseStyle, OptionType};
use finstack_valuations::instruments::{
    EquityUnderlyingParams, FinancingLegSpec, TrsScheduleSpec, TrsSide,
};
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

/// Shared market context for all equity benchmarks.
///
/// Provides USD-OIS / SOFR-3M curves, SPX spot and vol grid, VIX forward curve,
/// and VIX vol-of-vol surface for volatility index benchmarks.
fn create_equity_market() -> MarketContext {
    let base = base_date();

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.25, 0.995),
            (0.5, 0.990),
            (1.0, 0.980),
            (2.0, 0.960),
            (3.0, 0.940),
            (5.0, 0.900),
            (7.0, 0.860),
            (10.0, 0.800),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(base)
        .knots([
            (0.0, 0.02),
            (0.25, 0.021),
            (0.5, 0.022),
            (1.0, 0.023),
            (2.0, 0.024),
            (5.0, 0.025),
            (7.0, 0.026),
            (10.0, 0.027),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let vol_surface = VolSurface::from_grid(
        "SPX",
        &[0.25, 0.5, 1.0, 2.0, 3.0, 5.0],
        &[4500.0, 4800.0, 5000.0, 5200.0, 5500.0],
        &[0.28; 30],
    )
    .unwrap();

    // VIX forward curve: spot at 18.5, mean-reverting to ~22 at 1Y
    let vix_curve = VolatilityIndexCurve::builder("VIX")
        .base_date(base)
        .spot_level(18.5)
        .knots([(0.0, 18.5), (0.25, 20.0), (0.5, 21.0), (1.0, 22.0)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Flat VIX vol-of-vol surface (for vol index option pricing)
    let vix_volvol = VolSurface::from_grid(
        "VIX-VOLVOL",
        &[0.25, 0.5, 1.0],
        &[15.0, 18.0, 21.0, 24.0, 27.0],
        &[0.80; 15],
    )
    .unwrap();

    MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert(vix_curve)
        .insert_surface(vol_surface)
        .insert_surface(vix_volvol)
        .insert_price(
            "SPX",
            MarketScalar::Price(Money::new(5000.0, Currency::USD)),
        )
        .insert_price("SPX-SPOT", MarketScalar::Unitless(5000.0))
        // TRS tests use `SPX-DIV-YIELD`; variance forward variance uses `SPX-DIVYIELD`.
        .insert_price("SPX-DIV-YIELD", MarketScalar::Unitless(0.015))
        .insert_price("SPX-DIVYIELD", MarketScalar::Unitless(0.015))
}

fn sample_equity() -> Equity {
    Equity::new("SPX-EQ", "SPX", Currency::USD).with_shares(100.0)
}

fn equity_trs(tenor_years: i32) -> EquityTotalReturnSwap {
    let base = base_date();
    let start = base + time::Duration::days(2);
    let end = start + time::Duration::days(365 * tenor_years as i64);

    let notional = Money::new(10_000_000.0, Currency::USD);
    let underlying = EquityUnderlyingParams::new("SPX-TRS", "SPX-SPOT", notional.currency())
        .with_contract_size(1.0)
        .with_dividend_yield(CurveId::new("SPX-DIV-YIELD"));

    let financing = FinancingLegSpec::new(
        "USD-OIS",
        "USD-SOFR-3M",
        Decimal::try_from(25.0).expect("spread bp"),
        DayCount::Act360,
    );

    let schedule = TrsScheduleSpec::from_params(start, end, ScheduleParams::quarterly_act360());

    EquityTotalReturnSwap::builder()
        .id(format!("EQ-TRS-{tenor_years}Y").into())
        .notional(notional)
        .underlying(underlying)
        .financing(financing)
        .schedule(schedule)
        .side(TrsSide::ReceiveTotalReturn)
        .build()
        .unwrap()
}

fn equity_index_future(expiry: Date, last_trade: Date, label: &str) -> EquityIndexFuture {
    EquityIndexFuture::builder()
        .id(InstrumentId::new(label))
        .underlying_ticker("SPX".to_string())
        .notional(Money::new(2_250_000.0, Currency::USD))
        .expiry(expiry)
        .last_trading_date(last_trade)
        .entry_price_opt(Some(4500.0))
        .quoted_price_opt(Some(4550.0))
        .position(Position::Long)
        .contract_specs(EquityFutureSpecs::sp500_emini())
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

fn variance_swap(months: i64) -> VarianceSwap {
    let start = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let maturity = start + time::Duration::days(months * 30);

    VarianceSwap::builder()
        .id(InstrumentId::new(format!("VAR-{months}M")))
        .underlying_ticker("SPX".to_string())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .strike_variance(0.04)
        .start_date(start)
        .maturity(maturity)
        .observation_freq(Tenor::daily())
        .realized_var_method(RealizedVarMethod::CloseToClose)
        .side(PayReceive::Pay)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

fn bench_equity_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("equity_pv");
    let market = create_equity_market();
    let as_of = base_date();
    let equity = sample_equity();

    group.bench_function("spx_spot", |b| {
        b.iter(|| equity.value(black_box(&market), black_box(as_of)));
    });
    group.finish();
}

fn bench_equity_trs_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("equity_trs_pv");
    let market = create_equity_market();
    let as_of = base_date();

    for years in [1, 3, 5] {
        let trs = equity_trs(years);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{years}Y")),
            &years,
            |b, _| {
                b.iter(|| trs.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

fn bench_equity_index_future_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("equity_index_future_pv");
    let market = create_equity_market();
    let as_of = base_date();

    let near_expiry = Date::from_calendar_date(2025, Month::June, 20).unwrap();
    let near_last = Date::from_calendar_date(2025, Month::June, 19).unwrap();
    let far_expiry = Date::from_calendar_date(2027, Month::June, 18).unwrap();
    let far_last = Date::from_calendar_date(2027, Month::June, 17).unwrap();

    let near = equity_index_future(near_expiry, near_last, "ES-NEAR");
    let far = equity_index_future(far_expiry, far_last, "ES-FAR");

    group.bench_function("near_expiry", |b| {
        b.iter(|| near.value(black_box(&market), black_box(as_of)));
    });
    group.bench_function("far_expiry", |b| {
        b.iter(|| far.value(black_box(&market), black_box(as_of)));
    });
    group.finish();
}

fn bench_variance_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("variance_swap_pv");
    let market = create_equity_market();
    let as_of = base_date();

    for months in [3, 6, 12] {
        let swap = variance_swap(months);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{months}M")),
            &months,
            |b, _| {
                b.iter(|| swap.value(black_box(&market), black_box(as_of)));
            },
        );
    }
    group.finish();
}

// ================================================================================================
// Volatility index benchmarks
// ================================================================================================

/// Benchmark VIX future pricing vs tenor (1M, 3M, 6M).
///
/// Measures the vol index forward curve interpolation and discount factor
/// lookup required to mark a VIX futures position to market.
fn bench_vol_index_future_pv(c: &mut Criterion) {
    let market = create_equity_market();
    let as_of = base_date();

    let mut group = c.benchmark_group("vol_index_future/tenor");
    for (label, months) in [("1M", 1_i64), ("3M", 3), ("6M", 6)] {
        let expiry = as_of + time::Duration::days(months * 30);
        let fut = VolatilityIndexFuture::builder()
            .id(InstrumentId::new(format!("VIX-FUT-{label}")))
            .notional(Money::new(100_000.0, Currency::USD))
            .expiry(expiry)
            .settlement_date(expiry)
            .quoted_price(18.5)
            .position(Position::Long)
            .contract_specs(VolIndexContractSpecs::vix())
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .attributes(Attributes::new())
            .build()
            .unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| black_box(fut.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

/// Benchmark VIX call/put option pricing via Black-76 on VIX forward.
///
/// Measures vol-of-vol surface lookup, Black-76 evaluation, and discount
/// factor interpolation for options on a volatility index.
fn bench_vol_index_option_pv(c: &mut Criterion) {
    let market = create_equity_market();
    let as_of = base_date();

    let mut group = c.benchmark_group("vol_index_option/type");
    for (label, opt_type) in [("call", OptionType::Call), ("put", OptionType::Put)] {
        let expiry = as_of + time::Duration::days(90);
        let opt = VolatilityIndexOption::builder()
            .id(InstrumentId::new(format!("VIX-OPT-{label}")))
            .notional(Money::new(10_000.0, Currency::USD))
            .strike(20.0)
            .option_type(opt_type)
            .exercise_style(ExerciseStyle::European)
            .expiry(expiry)
            .contract_specs(VolIndexOptionSpecs::vix())
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .vol_index_curve_id(CurveId::new("VIX"))
            .vol_of_vol_surface_id(CurveId::new("VIX-VOLVOL"))
            .attributes(Attributes::new())
            .build()
            .unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            b.iter(|| black_box(opt.value(black_box(&market), black_box(as_of))).unwrap())
        });
    }
    group.finish();
}

criterion_group!(
    equity_pricing,
    bench_equity_pv,
    bench_equity_trs_pv,
    bench_equity_index_future_pv,
    bench_variance_swap_pv,
    bench_vol_index_future_pv,
    bench_vol_index_option_pv,
);
criterion_main!(equity_pricing);
