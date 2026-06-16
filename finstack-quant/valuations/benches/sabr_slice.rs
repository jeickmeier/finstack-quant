//! Criterion benchmark for SABR slice calibration.

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_valuations::calibration::api::schema::{StepParams, VolSurfaceParams};
use finstack_quant_valuations::calibration::CalibrationConfig;
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::market::conventions::ids::OptionConventionId;
use finstack_quant_valuations::market::quotes::ids::QuoteId;
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
use finstack_quant_valuations::market::quotes::vol::VolQuote;
#[allow(dead_code, unused_imports, clippy::expect_used, clippy::unwrap_used)]
mod test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}
use std::hint::black_box;
use test_utils::calibration::execute_step;
use time::Month;

fn bench_sabr_slice(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let quotes = [
        VolQuote::OptionVol {
            id: QuoteId::new("SPY-VOL-30D-95"),
            underlying: "SPY".to_string().into(),
            expiry: base_date + time::Duration::days(30),
            strike: 95.0,
            vol: 0.22,
            option_type: OptionType::Call,
            convention: OptionConventionId::new("USD-Option"),
        },
        VolQuote::OptionVol {
            id: QuoteId::new("SPY-VOL-30D-100"),
            underlying: "SPY".to_string().into(),
            expiry: base_date + time::Duration::days(30),
            strike: 100.0,
            vol: 0.20,
            option_type: OptionType::Call,
            convention: OptionConventionId::new("USD-Option"),
        },
        VolQuote::OptionVol {
            id: QuoteId::new("SPY-VOL-30D-105"),
            underlying: "SPY".to_string().into(),
            expiry: base_date + time::Duration::days(30),
            strike: 105.0,
            vol: 0.21,
            option_type: OptionType::Call,
            convention: OptionConventionId::new("USD-Option"),
        },
    ];
    let settings = CalibrationConfig::default();
    let params = VolSurfaceParams {
        surface_id: "SPY-VOL".to_string(),
        base_date,
        underlying_ticker: "SPY".to_string(),
        model: "SABR".to_string(),
        discount_curve_id: Some("USD-OIS".into()),
        beta: 1.0,
        target_expiries: vec![1.0 / 12.0],
        target_strikes: vec![95.0, 100.0, 105.0],
        spot_override: None,
        dividend_yield_override: None,
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
