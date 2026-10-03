//! Criterion benchmark for SABR slice calibration.

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_calibration::api::schema::{StepParams, VolSurfaceParams};
use finstack_quant_calibration::quotes::ids::QuoteId;
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_calibration::quotes::vol::VolQuote;
use finstack_quant_calibration::CalibrationConfig;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_valuations::instruments::OptionType;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
#[path = "../tests/support/calibration.rs"]
mod calibration_support;
use calibration_support::execute_step;
use std::hint::black_box;
use time::Month;

fn bench_sabr_slice(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let expiry = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let target_expiry = DayCount::Act365F
        .year_fraction(base_date, expiry, DayCountContext::default())
        .unwrap();
    let quotes = [
        VolQuote::OptionVol {
            id: QuoteId::new("SPY-VOL-30D-95"),
            underlying: "SPY".to_string().into(),
            expiry,
            strike: 90.0,
            vol: 0.20,
            option_type: OptionType::Call,
        },
        VolQuote::OptionVol {
            id: QuoteId::new("SPY-VOL-30D-100"),
            underlying: "SPY".to_string().into(),
            expiry,
            strike: 100.0,
            vol: 0.20,
            option_type: OptionType::Call,
        },
        VolQuote::OptionVol {
            id: QuoteId::new("SPY-VOL-30D-105"),
            underlying: "SPY".to_string().into(),
            expiry,
            strike: 110.0,
            vol: 0.20,
            option_type: OptionType::Call,
        },
    ];
    let settings = CalibrationConfig {
        fail_on_bad_fit: false,
        ..Default::default()
    };
    let params = VolSurfaceParams {
        vol_surface_id: "SPY-VOL".to_string(),
        base_date,
        underlying_ticker: "SPY".to_string(),
        discount_curve_id: Some("USD-OIS".into()),
        beta: 0.5,
        target_expiries: vec![target_expiry],
        target_strikes: vec![90.0, 100.0, 110.0],
        spot_override: Some(100.0),
        dividend_yield_override: Some(0.0),
        expiry_extrapolation: Default::default(),
    };
    let step = StepParams::VolSurface(params);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (5.0, 0.78)])
        .build()
        .unwrap();
    let market = MarketContext::new()
        .insert(disc)
        .insert_price(
            "SPY",
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(100.0),
        )
        .insert_price(
            "SPY-DIVYIELD",
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(0.02),
        );
    let market_quotes: Vec<MarketQuote> = quotes.iter().cloned().map(MarketQuote::Vol).collect();
    c.bench_function("sabr_slice_calibration", |b| {
        b.iter(|| {
            execute_step(
                black_box(&step),
                black_box(&market_quotes),
                black_box(&market),
                black_box(&settings),
            )
            .unwrap()
        })
    });
}

criterion_group!(benches, bench_sabr_slice);
criterion_main!(benches);
