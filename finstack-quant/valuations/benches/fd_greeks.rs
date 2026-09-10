//! Finite-difference Greeks benchmarks.
//!
//! Isolates the per-instrument cost of requesting the full five-metric FD
//! set (delta, gamma, vega, vanna, volga) against a cheap closed-form
//! baseline:
//! - Equity option closed-form Greeks (baseline)
//! - Barrier option FD Greeks (analytical pricer repriced per bump)
//! - Asian option FD Greeks (Monte Carlo pricer repriced per bump)

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{BarrierType, CurveId, InstrumentId, PriceId};
use finstack_quant_valuations::instruments::exotics::asian_option::AsianOption;
use finstack_quant_valuations::instruments::exotics::barrier_option::BarrierOption;
use finstack_quant_valuations::instruments::{Instrument, OptionType, PricingOptions};
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../tests/support/equity_fx_options.rs"]
mod option_support;
use std::hint::black_box;
use time::Month;

fn as_of() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn discount_curve(id: &str) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(as_of())
        .knots([
            (0.0, 1.0),
            (0.5, 0.98),
            (1.0, 0.96),
            (2.0, 0.92),
            (5.0, 0.80),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

fn flat_vol_surface(id: &str) -> VolSurface {
    VolSurface::from_grid(
        id,
        &[0.25, 0.5, 1.0, 2.0],
        &[80.0, 90.0, 100.0, 110.0, 120.0],
        &[0.25; 20],
    )
    .unwrap()
}

/// Single market covering every fixture's curve/surface/price IDs.
fn create_market() -> MarketContext {
    MarketContext::new()
        .insert(discount_curve("USD-OIS"))
        .insert(discount_curve("USD_DISC"))
        .insert_surface(flat_vol_surface("EQUITY-VOL"))
        .insert_surface(flat_vol_surface("SPX_VOL"))
        .insert_price(
            "EQUITY-SPOT",
            MarketScalar::Price(Money::new(100.0, Currency::USD).expect("valid money fixture")),
        )
        .insert_price("EQUITY-DIVYIELD", MarketScalar::Unitless(0.02))
        .insert_price(
            "SPX",
            MarketScalar::Price(Money::new(100.0, Currency::USD).expect("valid money fixture")),
        )
        .insert_price("SPX_DIV", MarketScalar::Unitless(0.02))
}

fn create_equity_option(
) -> finstack_quant_valuations::instruments::equity::equity_option::EquityOption {
    let expiry = as_of() + time::Duration::days(365);
    option_support::equity_option_european_call("GREEKS-EQ-CALL", "EQUITY", 100.0, expiry, 100.0)
        .expect("equity option should build in benchmarks")
}

fn create_barrier_option() -> BarrierOption {
    BarrierOption {
        expiry_fixing: None,
        id: InstrumentId::new("GREEKS-BARRIER"),
        underlying_ticker: "SPX".into(),
        strike: 100.0,
        barrier: Money::new(80.0, Currency::USD).expect("valid money fixture"),
        rebate: None,
        rebate_timing: Default::default(),
        option_type: OptionType::Call,
        barrier_type: BarrierType::DownAndOut,
        expiry: as_of() + time::Duration::days(365),
        observed_barrier_breached: None,
        notional: Money::new(100.0, Currency::USD).expect("valid money fixture"),
        day_count: DayCount::Act365F,
        monitoring: finstack_quant_valuations::instruments::Monitoring::Continuous,
        discount_curve_id: CurveId::new("USD_DISC"),
        spot_id: "SPX".into(),
        vol_surface_id: CurveId::new("SPX_VOL"),
        div_yield_id: Some(finstack_quant_core::types::PriceId::new("SPX_DIV")),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

fn create_asian_option() -> AsianOption {
    // Monthly fixings over the 1y life; all strictly after `as_of`.
    let fixing_dates: Vec<time::Date> = (1..=12)
        .map(|m| Date::from_calendar_date(2025, Month::try_from(m).unwrap(), 28).unwrap())
        .collect();
    // Cap MC paths: each FD greek reprices the full simulation, so the
    // production default (10k paths) would make one iteration take seconds.
    let overrides = finstack_quant_valuations::instruments::InstrumentPricingOverrides::default()
        .with_mc_paths(200);
    AsianOption::builder()
        .id(InstrumentId::new("GREEKS-ASIAN"))
        .underlying_ticker("SPX".to_string())
        .strike(100.0)
        .option_type(OptionType::Call)
        .averaging_method(finstack_quant_valuations::instruments::AveragingMethod::Arithmetic)
        .expiry(as_of() + time::Duration::days(365))
        .fixing_dates(fixing_dates)
        .notional(Money::new(100.0, Currency::USD).expect("valid money fixture"))
        .day_count(DayCount::Act365F)
        .discount_curve_id(CurveId::new("USD_DISC"))
        .spot_id("SPX".into())
        .vol_surface_id(CurveId::new("SPX_VOL"))
        .div_yield_id_opt(Some(PriceId::new("SPX_DIV")))
        .instrument_pricing_overrides(overrides)
        .attributes(Default::default())
        .build()
        .expect("asian option should build in benchmarks")
}

const FD_GREEK_SET: [finstack_quant_valuations::metrics::MetricId; 5] = [
    finstack_quant_valuations::metrics::MetricId::Delta,
    finstack_quant_valuations::metrics::MetricId::Gamma,
    finstack_quant_valuations::metrics::MetricId::Vega,
    finstack_quant_valuations::metrics::MetricId::Vanna,
    finstack_quant_valuations::metrics::MetricId::Volga,
];

fn bench_full_greek_set<I: Instrument>(c: &mut Criterion, name: &str, instrument: &I) {
    let market = create_market();
    c.bench_function(name, |b| {
        b.iter(|| {
            instrument.price_with_metrics(
                black_box(&market),
                black_box(as_of()),
                black_box(&FD_GREEK_SET),
                PricingOptions::default(),
            )
        })
    });
}

fn bench_fd_greeks(c: &mut Criterion) {
    bench_full_greek_set(
        c,
        "fd_greeks/equity_option_closed_form",
        &create_equity_option(),
    );
    bench_full_greek_set(c, "fd_greeks/barrier_option_fd", &create_barrier_option());
    bench_full_greek_set(c, "fd_greeks/asian_option_mc_fd", &create_asian_option());
}

criterion_group!(benches, bench_fd_greeks);
criterion_main!(benches);
