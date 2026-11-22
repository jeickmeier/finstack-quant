//! Equity non-option instrument pricing benchmarks.
//!
//! Covers:
//! - [`Equity`]: spot equity PV.
//! - [`EquityTotalReturnSwap`]: floating financing vs equity total return.
//! - [`EquityFuture`]: cost-of-carry future pricing.
//! - [`VarianceSwap`]: fair-strike via Carr-Madan replication.
//! - [`VolatilityIndexFuture`]: VIX-style forward contract.
//! - [`EquityFutureOption`]: options on listed equity/volatility-index futures.

#![allow(clippy::unwrap_used)]

#[path = "../../benches/support/mod.rs"]
mod support;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_cashflows::builder::ScheduleParams;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::{
    VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, ForwardCurve, PriceCurve, PriceCurveKind,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::math::stats::RealizedVarMethod;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::{CurveId, InstrumentId, PriceId};
use finstack_quant_models::types::{ExerciseStyle, OptionType};
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_core::instruments::Position;
use finstack_quant_valuations_core::instruments::pricing_overrides::InstrumentPricingOverrides;
use finstack_quant_valuations_core::instruments::{FinancingLegSpec, TrsScheduleSpec};
use finstack_quant_valuations_core::instruments::{
    FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement, FutureOptionTerms,
    ListedFutureTerms,
};
use finstack_quant_valuations_equity::EquityUnderlyingParams;
use finstack_quant_valuations_equity::equity_future::EquityFuture;
use finstack_quant_valuations_equity::equity_future_option::EquityFutureOption;
use finstack_quant_valuations_equity::equity_trs::EquityTotalReturnSwap;
use finstack_quant_valuations_equity::spot::Equity;
use finstack_quant_valuations_equity::variance_swap::VarianceSwap;
use finstack_quant_valuations_equity::vol_index_future::VolatilityIndexFuture;
use rust_decimal::Decimal;
use std::hint::black_box;
use time::Month;

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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("USD-SOFR-3M")
        .base_date(base)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.125, 0.997471950911942),
            (0.25, 0.994888110405994),
            (0.375, 0.992248945555301),
            (0.5, 0.989554932567899),
            (0.625, 0.98682197561651),
            (0.75, 0.984065814019865),
            (0.875, 0.981286684604636),
            (1.0, 0.978484825682528),
            (1.125, 0.975668099393593),
            (1.25, 0.972844280694656),
            (1.375, 0.970013478228807),
            (1.5, 0.967175800650738),
            (1.625, 0.964331356619928),
            (1.75, 0.961480254793845),
            (1.875, 0.958622603821178),
            (2.0, 0.955758512335073),
            (2.1875, 0.951461686598083),
            (2.375, 0.947173078450782),
            (2.5625, 0.942892751140432),
            (2.75, 0.938620767338384),
            (2.9375, 0.934357189139748),
            (3.125, 0.9301020780631),
            (3.3125, 0.925855495050225),
            (3.5, 0.921617500465892),
            (3.6875, 0.917388154097666),
            (3.875, 0.913167515155755),
            (4.0625, 0.908955642272887),
            (4.25, 0.904752593504231),
            (4.4375, 0.900558426327339),
            (4.625, 0.896373197642133),
            (4.8125, 0.892196963770921),
            (5.0, 0.888029780458446),
            (5.125, 0.885255560927911),
            (5.25, 0.882483113678219),
            (5.375, 0.879712476410418),
            (5.5, 0.876943686664984),
            (5.625, 0.87417678182115),
            (5.75, 0.871411799096252),
            (5.875, 0.868648775545072),
            (6.0, 0.865887748059205),
            (6.125, 0.863128753366417),
            (6.25, 0.860371828030025),
            (6.375, 0.857617008448277),
            (6.5, 0.854864330853747),
            (6.625, 0.852113831312733),
            (6.75, 0.849365545724664),
            (6.875, 0.84661950982152),
            (7.0, 0.843875759167255),
            (7.1875, 0.839766955659775),
            (7.375, 0.835668364736334),
            (7.5625, 0.831580032378845),
            (7.75, 0.827502003984762),
            (7.9375, 0.823434324367821),
            (8.125, 0.819377037758803),
            (8.3125, 0.815330187806336),
            (8.5, 0.811293817577717),
            (8.6875, 0.807267969559776),
            (8.875, 0.803252685659755),
            (9.0625, 0.799248007206233),
            (9.25, 0.795253974950065),
            (9.4375, 0.79127062906536),
            (9.625, 0.787298009150485),
            (9.8125, 0.783336154229096),
            (10.0, 0.779385102751198),
        ])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let vol_surface = VolSurface::from_grid(
        "SPX",
        &[0.25, 0.5, 1.0, 2.0, 3.0, 5.0],
        &[4500.0, 4800.0, 5000.0, 5200.0, 5500.0],
        &[0.28; 30],
        VolGridOpts {
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type: VolQuoteType::BlackLognormal,
            interpolation_mode: VolInterpolationMode::Vol,
        },
        None,
    )
    .unwrap();

    // VIX forward curve: spot at 18.5, mean-reverting to ~22 at 1Y
    let vix_curve = PriceCurve::builder("VIX")
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Flat)
        .kind(PriceCurveKind::VolIndex)
        .base_date(base)
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
        VolGridOpts {
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type: VolQuoteType::BlackLognormal,
            interpolation_mode: VolInterpolationMode::Vol,
        },
        None,
    )
    .unwrap();

    MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(fwd)
        .expect("insert market curve")
        .try_insert(vix_curve)
        .expect("insert market curve")
        .insert_surface(vol_surface)
        .insert_surface(vix_volvol)
        .insert_price(
            "SPX",
            MarketScalar::price(Money::new(5000.0, Currency::USD).expect("valid money fixture")),
        )
        .insert_price(
            "SPX-SPOT",
            MarketScalar::unitless(5000.0).expect("valid scalar"),
        )
        // TRS tests use `SPX-DIV-YIELD`; variance forward variance uses `SPX-DIVYIELD`.
        .insert_price(
            "SPX-DIV-YIELD",
            MarketScalar::unitless(0.015).expect("valid scalar"),
        )
        .insert_price(
            "SPX-DIVYIELD",
            MarketScalar::unitless(0.015).expect("valid scalar"),
        )
}

fn sample_equity() -> Equity {
    Equity::new("SPX-EQ", "SPX", Currency::USD)
        .with_quantity(100.0)
        .with_spot_id("SPX")
        .with_div_yield_id("SPX-DIVYIELD")
}

fn equity_trs(tenor_years: i32) -> EquityTotalReturnSwap {
    let base = base_date();
    let start = base + time::Duration::days(2);
    let end = start + time::Duration::days(365 * tenor_years as i64);

    let notional = Money::new(10_000_000.0, Currency::USD).expect("valid money fixture");
    let underlying = EquityUnderlyingParams::new("SPX-TRS", "SPX-SPOT", notional.currency())
        .with_div_yield_id(PriceId::new("SPX-DIV-YIELD"));

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
        .financing_leg(financing)
        .schedule(schedule)
        .side(PayReceive::Receive)
        .dividend_settlement(
            finstack_quant_valuations_equity::equity_trs::TrsDividendSettlement::OnDividendDate,
        )
        .build()
        .unwrap()
}

fn equity_future(settlement: Date, last_trade: Date, label: &str) -> EquityFuture {
    EquityFuture::builder()
        .id(InstrumentId::new(label))
        .underlying_ticker("SPX".to_string())
        .underlying_currency(Currency::USD)
        .terms(
            ListedFutureTerms::new(
                10.0,
                50.0,
                Currency::USD,
                4500.0,
                last_trade,
                settlement,
                Position::Long,
            )
            .unwrap(),
        )
        .discount_curve_id(CurveId::new("USD-OIS"))
        .spot_id("SPX-SPOT".into())
        .div_yield_id_opt(Some("SPX-DIVYIELD".into()))
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
        .spot_id("SPX".into())
        .vol_surface_id("SPX".into())
        .div_yield_id_opt(Some("SPX-DIVYIELD".into()))
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .strike_variance(0.04)
        .start_date(start)
        .maturity(maturity)
        .observation_frequency(Tenor::daily())
        .observation_calendar_id("USNY".into())
        .realized_var_method(RealizedVarMethod::CloseToClose)
        .price_series_policy(
            finstack_quant_valuations_equity::variance_swap::EquityPriceSeriesPolicy::Adjusted,
        )
        .side(PayReceive::Pay)
        .discount_curve_id(CurveId::new("USD-OIS"))
        .day_count(DayCount::Act365F)
        .attributes(Attributes::new())
        .build()
        .unwrap()
}

fn bench_equity_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("equity_pv");
    group.bench_function("spx_spot", |b| {
        let market = create_equity_market();
        let as_of = base_date();
        let equity = sample_equity();
        support::bench(
            b,
            || {
                equity
                    .value(black_box(&market), black_box(as_of))
                    .expect("spot equity PV")
            },
            |pv| {
                assert!(pv.amount().is_finite());
                assert_eq!(pv.currency(), Currency::USD);
                assert_eq!(pv.amount(), 100.0 * 5000.0);
            },
        );
    });
    group.finish();
}

fn bench_equity_trs_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("equity_trs_pv");
    {
        let years = 3;
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{years}Y")),
            &years,
            |b, _| {
                let market = create_equity_market();
                let as_of = base_date();
                let trs = equity_trs(years);
                assert!(as_of < trs.schedule.start);
                assert_eq!(
                    (trs.schedule.end - trs.schedule.start).whole_days(),
                    365 * i64::from(years)
                );
                support::bench(
                    b,
                    || {
                        trs.value(black_box(&market), black_box(as_of))
                            .expect("equity total-return-swap PV")
                    },
                    |pv| {
                        assert!(pv.amount().is_finite());
                        assert_eq!(pv.currency(), Currency::USD);
                    },
                );
            },
        );
    }
    group.finish();
}

fn bench_equity_future_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("equity_future_pv");
    group.bench_function("near_expiry", |b| {
        let market = create_equity_market();
        let as_of = base_date();
        let near_settlement = Date::from_calendar_date(2025, Month::June, 20).unwrap();
        let near_last = Date::from_calendar_date(2025, Month::June, 19).unwrap();
        let near = equity_future(near_settlement, near_last, "ES-NEAR");
        assert!(as_of < near_last);
        support::bench(
            b,
            || {
                near.value(black_box(&market), black_box(as_of))
                    .expect("cost-of-carry equity future PV")
            },
            |pv| {
                assert!(pv.amount().is_finite());
                assert_eq!(pv.currency(), Currency::USD);
            },
        );
    });
    group.finish();
}

fn bench_variance_swap_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("variance_swap_pv");
    {
        let months = 6;
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{months}M")),
            &months,
            |b, _| {
                let market = create_equity_market();
                let as_of = base_date();
                let swap = variance_swap(months);
                // A missing override selects Carr-Madan surface replication.
                assert!(
                    swap.instrument_pricing_overrides
                        .market_quotes
                        .implied_volatility
                        .is_none()
                );
                assert!(as_of < swap.start_date);
                support::bench(
                    b,
                    || {
                        swap.value(black_box(&market), black_box(as_of))
                            .expect("Carr-Madan equity variance-swap PV")
                    },
                    |pv| {
                        assert!(pv.amount().is_finite());
                        assert_eq!(pv.currency(), Currency::USD);
                    },
                );
            },
        );
    }
    group.finish();
}

// Volatility index benchmarks

/// Benchmark three-month VIX future pricing.
///
/// Measures the vol index forward curve interpolation and discount factor
/// lookup required to mark a VIX futures position to market.
fn bench_vol_index_future_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("vol_index_future/tenor");
    {
        let (label, months) = ("3M", 3_i64);
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            let market = create_equity_market();
            let as_of = base_date();
            let expiry = as_of + time::Duration::days(months * 30);
            let fut = VolatilityIndexFuture::builder()
                .id(InstrumentId::new(format!("VIX-FUT-{label}")))
                .terms(
                    ListedFutureTerms::new(
                        100_000.0 / (1_000.0 * 18.5),
                        1_000.0,
                        Currency::USD,
                        18.5,
                        expiry,
                        expiry,
                        Position::Long,
                    )
                    .unwrap(),
                )
                .vol_index_curve_id(CurveId::new("VIX"))
                .attributes(Attributes::new())
                .build()
                .unwrap();

            assert!(as_of < expiry);
            support::bench(
                b,
                || {
                    fut.value(black_box(&market), black_box(as_of))
                        .expect("volatility-index future PV")
                },
                |pv| {
                    assert!(pv.amount().is_finite());
                    assert_eq!(pv.currency(), Currency::USD);
                },
            );
        });
    }
    group.finish();
}

/// Benchmark a listed volatility-future call via Black-76.
///
/// Measures Black-76 evaluation with an explicit flat vol-of-vol quote and
/// discount factor interpolation for an option on a volatility index.
fn bench_equity_future_option_pv(c: &mut Criterion) {
    let mut group = c.benchmark_group("equity_future_option/type");
    {
        let (label, opt_type) = ("call", OptionType::Call);
        group.bench_with_input(BenchmarkId::from_parameter(label), &label, |b, _| {
            let market = create_equity_market();
            let as_of = base_date();
            let expiry = as_of + time::Duration::days(90);
            let terms = FutureOptionTerms::builder()
                .underlying("VIX-FUT".to_string())
                .futures_price(20.0)
                .strike(20.0)
                .contracts(100.0)
                .multiplier(100.0)
                .currency(Currency::USD)
                .option_type(opt_type)
                .position(Position::Long)
                .exercise_style(ExerciseStyle::European)
                .expiry(expiry)
                .settlement(FutureOptionSettlement::Cash {
                    payment_date: expiry,
                })
                .model(FutureOptionModel::Black76)
                .premium_style(FutureOptionPremiumStyle::PremiumPaid)
                .day_count(DayCount::Act365F)
                .discount_curve_id(CurveId::new("USD-OIS"))
                .build()
                .unwrap();
            let opt = EquityFutureOption::builder()
                .id(InstrumentId::new(format!("VIX-OPT-{label}")))
                .terms(terms)
                .instrument_pricing_overrides(
                    InstrumentPricingOverrides::default().with_implied_volatility(0.80),
                )
                .attributes(Attributes::new())
                .build()
                .unwrap();

            assert_eq!(opt.terms.exercise_style, ExerciseStyle::European);
            assert_eq!(opt.terms.model, FutureOptionModel::Black76);
            assert_eq!(
                opt.instrument_pricing_overrides
                    .market_quotes
                    .implied_volatility,
                Some(0.80)
            );
            assert!(as_of < opt.terms.expiry);
            support::bench(
                b,
                || {
                    opt.value(black_box(&market), black_box(as_of))
                        .expect("Black-76 equity future-option PV")
                },
                |pv| {
                    assert!(pv.amount().is_finite());
                    assert_eq!(pv.currency(), Currency::USD);
                },
            );
        });
    }
    group.finish();
}

criterion_group!(
    equity_pricing,
    bench_equity_pv,
    bench_equity_trs_pv,
    bench_equity_future_pv,
    bench_variance_swap_pv,
    bench_vol_index_future_pv,
    bench_equity_future_option_pv,
);
criterion_main!(equity_pricing);
