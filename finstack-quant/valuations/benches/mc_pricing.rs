//! Monte Carlo pricing benchmarks (public API).
//!
//! Benchmarks LSMC Bermudan swaption pricing via the public pricer API.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::swaption::{
    BermudanSchedule, BermudanSwaption, BermudanSwaptionPricer, BermudanSwaptionPricerConfig,
};
use finstack_quant_valuations::pricer::Pricer;
use std::hint::black_box;
use time::Month;

fn build_swaption(as_of: Date) -> BermudanSwaption {
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    BermudanSwaption::new_payer(
        "BERM-LSMC-BENCH",
        Money::new(10_000_000.0, Currency::USD),
        0.03,
        swap_start,
        swap_end,
        BermudanSchedule::co_terminal(first_exercise, swap_end, Tenor::semi_annual())
            .expect("valid Bermudan schedule"),
        "USD-OIS",
        "USD-OIS",
        "USD-VOL",
    )
    .expect("valid Bermudan swaption")
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
                        mc_paths: n,
                        mc_seed: 42,
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

criterion_group!(benches, bench_bermudan_lsmc);
criterion_main!(benches);
