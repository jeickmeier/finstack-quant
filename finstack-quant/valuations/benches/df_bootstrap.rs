//! Df bootstrap benchmarks for valuations.

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::IndexId;
use finstack_quant_valuations::calibration::api::schema::{DiscountCurveParams, StepParams};
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

fn bench_df_bootstrap(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let quotes = [
        RateQuote::Deposit {
            id: QuoteId::new("DEP-30D"),
            index: IndexId::new("USD-SOFR"),
            pillar: Pillar::Date(base_date + time::Duration::days(30)),
            rate: 0.045,
        },
        RateQuote::Deposit {
            id: QuoteId::new("DEP-90D"),
            index: IndexId::new("USD-SOFR"),
            pillar: Pillar::Date(base_date + time::Duration::days(90)),
            rate: 0.046,
        },
        RateQuote::Swap {
            id: QuoteId::new("SWP-1Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Date(base_date + time::Duration::days(365)),
            rate: 0.047,
            spread_decimal: None,
        },
        RateQuote::Swap {
            id: QuoteId::new("SWP-2Y"),
            index: IndexId::new("USD-SOFR-OIS"),
            pillar: Pillar::Date(base_date + time::Duration::days(365 * 2)),
            rate: 0.048,
            spread_decimal: None,
        },
    ];
    let ctx = MarketContext::new();
    let settings = CalibrationConfig::default();
    let params = DiscountCurveParams {
        curve_id: "USD-OIS".into(),
        currency: Currency::USD,
        base_date,
        method: CalibrationMethod::Bootstrap,
        interpolation: Default::default(),
        extrapolation: finstack_quant_core::math::interp::ExtrapolationPolicy::FlatForward,
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: Default::default(),
    };
    let step = StepParams::Discount(params);
    let market_quotes: Vec<MarketQuote> = quotes.iter().cloned().map(MarketQuote::Rates).collect();

    c.bench_function("df_bootstrap_small", |b| {
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

criterion_group!(benches, bench_df_bootstrap);
criterion_main!(benches);
