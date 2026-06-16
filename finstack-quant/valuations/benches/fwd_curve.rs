//! Fwd curve benchmarks for valuations.

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::types::IndexId;
use finstack_quant_valuations::calibration::api::schema::{ForwardCurveParams, StepParams};
use finstack_quant_valuations::calibration::CalibrationConfig;
use finstack_quant_valuations::calibration::CalibrationMethod;
use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
use finstack_quant_valuations::market::quotes::rates::RateQuote;
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

fn bench_forward_curve(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80)])
        .build()
        .unwrap();
    let ctx = MarketContext::new().insert(disc);
    let quotes = [
        RateQuote::Fra {
            id: QuoteId::new("FRA-3x6"),
            index: IndexId::new("USD-SOFR-3M"),
            start: Pillar::Date(base_date + time::Duration::days(90)),
            end: Pillar::Date(base_date + time::Duration::days(180)),
            rate: 0.047,
        },
        RateQuote::Fra {
            id: QuoteId::new("FRA-6x9"),
            index: IndexId::new("USD-SOFR-3M"),
            start: Pillar::Date(base_date + time::Duration::days(180)),
            end: Pillar::Date(base_date + time::Duration::days(270)),
            rate: 0.048,
        },
    ];
    let settings = CalibrationConfig::default();
    let params = ForwardCurveParams {
        curve_id: "USD-SOFR-3M-FWD".into(),
        currency: Currency::USD,
        base_date,
        tenor_years: 0.25,
        discount_curve_id: "USD-OIS".into(),
        method: CalibrationMethod::Bootstrap,
        interpolation: Default::default(),
        conventions: Default::default(),
    };
    let step = StepParams::Forward(params);
    let market_quotes: Vec<MarketQuote> = quotes.iter().cloned().map(MarketQuote::Rates).collect();
    c.bench_function("fwd_curve_fra_strip", |b| {
        b.iter(|| {
            execute_step(
                black_box(&step),
                black_box(&market_quotes),
                black_box(&ctx),
                black_box(&settings),
            )
            .unwrap()
        })
    });
}

criterion_group!(benches, bench_forward_curve);
criterion_main!(benches);
