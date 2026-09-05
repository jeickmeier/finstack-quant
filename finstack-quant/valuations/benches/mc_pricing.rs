//! Monte Carlo pricing benchmarks (public API).
//!
//! Benchmarks LSMC Bermudan swaption pricing via the public pricer API.

use finstack_quant_valuations::instruments::OptionType;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
use finstack_quant_valuations::instruments::rates::hw1f::RateExoticMcConfig;
use finstack_quant_valuations::instruments::rates::swaption::{
    BermudanSchedule, BermudanSwaption, BermudanSwaptionPricer, BermudanSwaptionPricerConfig,
    PreparedHullWhiteModel,
};
use finstack_quant_valuations::pricer::Pricer;
use std::hint::black_box;
use time::Month;

fn build_swaption(as_of: Date) -> BermudanSwaption {
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let mut swaption = BermudanSwaption::new(
        "BERM-LSMC-BENCH",
        OptionType::Call,
        Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"),
        0.03,
        swap_start,
        swap_end,
        BermudanSchedule::co_terminal(first_exercise, swap_end, Tenor::semi_annual())
            .expect("valid Bermudan schedule"),
        "USD-OIS",
        "USD-OIS",
        "USD-VOL",
    )
    .expect("valid Bermudan swaption");
    swaption
        .instrument_pricing_overrides
        .model_config
        .hw1f_mean_reversion = Some(0.03);
    swaption
        .instrument_pricing_overrides
        .model_config
        .hw1f_sigma = Some(0.01);
    swaption
}

fn build_market(as_of: Date) -> MarketContext {
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([
            (0.0, 1.0),
            (0.5, 0.985),
            (1.0, 0.97),
            (2.0, 0.94),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Valid curve");

    MarketContext::new().insert(curve)
}

fn bench_bermudan_lsmc(c: &mut Criterion) {
    let mut group = c.benchmark_group("mc_bermudan_lsmc");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swaption = build_swaption(as_of);
    let market = build_market(as_of);

    {
        let num_paths = 50_000;
        group.throughput(Throughput::Elements(num_paths as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_paths),
            &num_paths,
            |b, &n| {
                let pricer =
                    BermudanSwaptionPricer::lsmc_with_config(BermudanSwaptionPricerConfig {
                        mc: RateExoticMcConfig {
                            num_paths: n,
                            seed: 42,
                            ..BermudanSwaptionPricerConfig::DEFAULT_MC
                        },
                        ..Default::default()
                    });
                b.iter(|| {
                    let result = pricer
                        .price_dyn(black_box(&swaption), black_box(&market), as_of)
                        .expect("lsmc price");
                    black_box(result.value)
                });
            },
        );
    }

    group.finish();
}

fn bench_bermudan_hw_tree(c: &mut Criterion) {
    let mut group = c.benchmark_group("mc_bermudan_hw_tree");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swaption = build_swaption(as_of);
    let market = build_market(as_of);
    let hw_params = HullWhiteCalibrationParams::default();
    let ttm = swaption.time_to_maturity(as_of).expect("ttm");
    let disc = market.get_discount("USD-OIS").expect("USD-OIS");
    let cached = PreparedHullWhiteModel::prepare(hw_params, 100, disc.as_ref(), ttm)
        .expect("pre-calibrated HW tree");

    let calibrate_each = BermudanSwaptionPricer::tree_with_config(BermudanSwaptionPricerConfig {
        tree_steps: 100,
        ..Default::default()
    });
    group.bench_function("calibrate_each_price", |b| {
        b.iter(|| {
            let result = calibrate_each
                .price_dyn(black_box(&swaption), black_box(&market), as_of)
                .expect("hw tree price");
            black_box(result.value)
        });
    });

    let reused = BermudanSwaptionPricer::tree_with_config(BermudanSwaptionPricerConfig {
        tree_steps: 100,
        prepared_model: Some(cached),
        ..Default::default()
    });
    group.bench_function("pre_calibrated_100_steps", |b| {
        b.iter(|| {
            let result = reused
                .price_dyn(black_box(&swaption), black_box(&market), as_of)
                .expect("cached hw tree price");
            black_box(result.value)
        });
    });
    group.finish();
}

criterion_group!(benches, bench_bermudan_hw_tree, bench_bermudan_lsmc);
criterion_main!(benches);
