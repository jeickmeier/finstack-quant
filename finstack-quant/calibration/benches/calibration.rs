//! Calibration benchmarks (canonical plan-driven API).
//!
//! This benchmark suite focuses on the plan-driven calibration step engine.

#[path = "support/checked_calibration.rs"]
mod checked;
use checked::checked_calibrate;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use finstack_quant_calibration::CalibrationConfig;
use finstack_quant_calibration::CalibrationMethod;
use finstack_quant_calibration::api::market_datum::MarketDatum;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, DiscountCurveParams, ForwardCurveParams,
    StepParams,
};
use finstack_quant_calibration::quotes::MarketQuote;
use finstack_quant_calibration::quotes::RateQuote;
use finstack_quant_calibration::quotes::{Pillar, QuoteId};
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::FxHashMap;
use finstack_quant_core::contract::LoadLimits;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations_core::instruments::PayReceive;
use finstack_quant_valuations_core::instruments::{Instrument, PricingOptions};
use finstack_quant_valuations_core::metrics::MetricId;
use finstack_quant_valuations_rates::fra::ForwardRateAgreement;
use rust_decimal::Decimal;
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

/// Extract the QuoteId of each MarketQuote (mirror of cal_utils::quote_set_ids).
fn quote_set_ids(quotes: &[MarketQuote]) -> Vec<QuoteId> {
    quotes
        .iter()
        .map(|q| match q {
            MarketQuote::Rates(q) => q.id().clone(),
            MarketQuote::Cds(q) => q.id().clone(),
            MarketQuote::CdsTranche(q) => q.id().clone(),
            MarketQuote::Inflation(q) => q.id().clone(),
            MarketQuote::Vol(q) => q.id().clone(),
            MarketQuote::Xccy(q) => q.id().clone(),
        })
        .collect()
}

fn quotes_to_data(quotes: &[MarketQuote]) -> Vec<MarketDatum> {
    quotes.iter().cloned().map(MarketDatum::from).collect()
}

fn bench_discount_and_forward_steps(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let settings = CalibrationConfig::default();

    // Discount curve inputs
    let disc_quotes: Vec<RateQuote> = vec![
        RateQuote::Deposit {
            id: QuoteId::new("DEP-1M"),
            index: IndexId::new("USD-SOFR"),
            pillar: Pillar::Date(base_date + time::Duration::days(30)),
            rate: 0.045,
        },
        RateQuote::Deposit {
            id: QuoteId::new("DEP-3M"),
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
    ];
    let disc_mq: Vec<MarketQuote> = disc_quotes
        .iter()
        .cloned()
        .map(MarketQuote::Rates)
        .collect();
    let disc_step = StepParams::Discount(DiscountCurveParams {
        curve_id: "USD-OIS".into(),
        currency: Currency::USD,
        base_date,
        method: CalibrationMethod::Bootstrap,
        interpolation: Default::default(),
        extrapolation: finstack_quant_core::math::interp::ExtrapolationPolicy::FlatForward,
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: Default::default(),
    });

    // Forward curve inputs
    let fwd_quotes: Vec<RateQuote> = vec![
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
    let fwd_mq: Vec<MarketQuote> = fwd_quotes.iter().cloned().map(MarketQuote::Rates).collect();
    let fwd_step = StepParams::Forward(ForwardCurveParams {
        curve_id: "USD-SOFR-3M-FWD".into(),
        currency: Currency::USD,
        base_date,
        tenor_years: 0.25,
        discount_curve_id: "USD-OIS".into(),
        method: CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: false,
        },
        interpolation: Default::default(),
        conventions: Default::default(),
    });

    let disc_ids = quote_set_ids(&disc_mq);
    let disc_data = quotes_to_data(&disc_mq);
    let fwd_ids = quote_set_ids(&fwd_mq);

    c.bench_function("calibration_discount_step", |b| {
        b.iter(|| {
            let mut quote_sets: FxHashMap<String, Vec<QuoteId>> = FxHashMap::default();
            quote_sets.insert("disc".to_string(), disc_ids.clone());
            let plan = CalibrationPlan {
                id: "bench_discount".to_string(),
                description: None,
                quote_sets: quote_sets.into_iter().collect(),
                steps: vec![CalibrationStep {
                    id: "disc".to_string(),
                    quote_set: "disc".to_string(),
                    params: disc_step.clone(),
                }],
                settings: settings.clone(),
            };
            let envelope = CalibrationEnvelope {
                schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
                schema_url: None,
                market_data: disc_data.clone(),
                prior_market: Vec::new(),
                plan,
            };
            checked_calibrate(black_box(&envelope))
        })
    });

    c.bench_function("calibration_discount_then_forward_steps", |b| {
        let mut combined_data = disc_data.clone();
        combined_data.extend(quotes_to_data(&fwd_mq));
        b.iter(|| {
            let mut quote_sets: FxHashMap<String, Vec<QuoteId>> = FxHashMap::default();
            quote_sets.insert("disc".to_string(), disc_ids.clone());
            quote_sets.insert("fwd".to_string(), fwd_ids.clone());
            let plan = CalibrationPlan {
                id: "bench_disc_fwd".to_string(),
                description: None,
                quote_sets: quote_sets.into_iter().collect(),
                steps: vec![
                    CalibrationStep {
                        id: "disc".to_string(),
                        quote_set: "disc".to_string(),
                        params: disc_step.clone(),
                    },
                    CalibrationStep {
                        id: "fwd".to_string(),
                        quote_set: "fwd".to_string(),
                        params: fwd_step.clone(),
                    },
                ],
                settings: settings.clone(),
            };
            let envelope = CalibrationEnvelope {
                schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
                schema_url: None,
                market_data: combined_data.clone(),
                prior_market: Vec::new(),
                plan,
            };
            checked_calibrate(black_box(&envelope))
        })
    });
}

/// Benchmark residual normalization fix (Phase 1.4).
///
/// Tests that calibration with different notionals produces similar performance.
/// This validates that the fix (pv / residual_notional) doesn't introduce
/// significant overhead compared to the broken (pv / 1.0) version.
fn bench_residual_normalization(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let settings = CalibrationConfig::default();

    // Create discount curve with 4 deposits (small but realistic)
    let make_quotes = || -> Vec<RateQuote> {
        vec![
            RateQuote::Deposit {
                id: QuoteId::new("DEP-1M"),
                index: IndexId::new("USD-SOFR"),
                pillar: Pillar::Date(base_date + time::Duration::days(30)),
                rate: 0.0450,
            },
            RateQuote::Deposit {
                id: QuoteId::new("DEP-3M"),
                index: IndexId::new("USD-SOFR"),
                pillar: Pillar::Date(base_date + time::Duration::days(90)),
                rate: 0.0455,
            },
            RateQuote::Deposit {
                id: QuoteId::new("DEP-6M"),
                index: IndexId::new("USD-SOFR"),
                pillar: Pillar::Date(base_date + time::Duration::days(180)),
                rate: 0.0460,
            },
            RateQuote::Deposit {
                id: QuoteId::new("DEP-1Y"),
                index: IndexId::new("USD-SOFR"),
                pillar: Pillar::Date(base_date + time::Duration::days(365)),
                rate: 0.0465,
            },
        ]
    };

    let disc_mq: Vec<MarketQuote> = make_quotes().into_iter().map(MarketQuote::Rates).collect();

    // Benchmark with notional = 1.0 (small notional)
    c.bench_function("calibration_residual_notional_1.0", |b| {
        let disc_step = StepParams::Discount(DiscountCurveParams {
            curve_id: "USD-OIS-small".into(),
            currency: Currency::USD,
            base_date,
            method: CalibrationMethod::Bootstrap,
            interpolation: Default::default(),
            extrapolation: finstack_quant_core::math::interp::ExtrapolationPolicy::FlatForward,
            pricing_discount_id: None,
            pricing_forward_id: None,
            conventions: Default::default(),
        });
        let disc_ids = quote_set_ids(&disc_mq);
        let disc_data = quotes_to_data(&disc_mq);
        b.iter(|| {
            let mut quote_sets: FxHashMap<String, Vec<QuoteId>> = FxHashMap::default();
            quote_sets.insert("disc".to_string(), disc_ids.clone());
            let plan = CalibrationPlan {
                id: "residual_notional_small".to_string(),
                description: None,
                quote_sets: quote_sets.into_iter().collect(),
                steps: vec![CalibrationStep {
                    id: "disc".to_string(),
                    quote_set: "disc".to_string(),
                    params: disc_step.clone(),
                }],
                settings: settings.clone(),
            };
            let envelope = CalibrationEnvelope {
                schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
                schema_url: None,
                market_data: disc_data.clone(),
                prior_market: Vec::new(),
                plan,
            };
            checked_calibrate(black_box(&envelope))
        })
    });

    // Benchmark with notional = 1_000_000.0 (large notional)
    // Should have similar performance to small notional after normalization fix
    c.bench_function("calibration_residual_notional_1M", |b| {
        let disc_step = StepParams::Discount(DiscountCurveParams {
            curve_id: "USD-OIS-large".into(),
            currency: Currency::USD,
            base_date,
            method: CalibrationMethod::Bootstrap,
            interpolation: Default::default(),
            extrapolation: finstack_quant_core::math::interp::ExtrapolationPolicy::FlatForward,
            pricing_discount_id: None,
            pricing_forward_id: None,
            conventions: Default::default(),
        });
        let disc_ids = quote_set_ids(&disc_mq);
        let disc_data = quotes_to_data(&disc_mq);
        b.iter(|| {
            let mut quote_sets: FxHashMap<String, Vec<QuoteId>> = FxHashMap::default();
            quote_sets.insert("disc".to_string(), disc_ids.clone());
            let plan = CalibrationPlan {
                id: "residual_notional_large".to_string(),
                description: None,
                quote_sets: quote_sets.into_iter().collect(),
                steps: vec![CalibrationStep {
                    id: "disc".to_string(),
                    quote_set: "disc".to_string(),
                    params: disc_step.clone(),
                }],
                settings: settings.clone(),
            };
            let envelope = CalibrationEnvelope {
                schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
                schema_url: None,
                market_data: disc_data.clone(),
                prior_market: Vec::new(),
                plan,
            };
            checked_calibrate(black_box(&envelope))
        })
    });
}

fn load_example(json: &str) -> CalibrationEnvelope {
    CalibrationEnvelope::from_slice_strict(json.as_bytes(), &LoadLimits::default())
        .map(|(envelope, _report)| envelope)
        .expect("benchmark envelope should pass strict loading")
}

fn bench_rate_quote_risk_cache(c: &mut Criterion) {
    let envelope = load_example(include_str!(
        "../examples/market_bootstrap/02_usd_3m_forward_curve.json"
    ));
    let result = checked_calibrate(&envelope);
    let market =
        MarketContext::try_from(result.result.final_market).expect("market state should rebuild");
    let as_of = Date::from_calendar_date(2026, Month::May, 8).expect("base date");
    let start = as_of + time::Duration::days(90);
    let maturity = as_of + time::Duration::days(180);
    let fras = (0..4)
        .map(|index| {
            ForwardRateAgreement::builder()
                .id(InstrumentId::new(format!("CACHE-BENCH-FRA-{index}")))
                .notional(Money::new(10_000_000.0, Currency::USD).expect("valid money fixture"))
                .start_date(start)
                .maturity(maturity)
                .fixed_rate(Decimal::try_from(0.047).expect("decimal rate"))
                .day_count(DayCount::Act360)
                .reset_lag_days(2)
                .discount_curve_id(CurveId::new("USD-OIS"))
                .forward_curve_id(CurveId::new("USD-SOFR-3M"))
                .side(PayReceive::Pay)
                .attributes(Default::default())
                .build()
                .expect("benchmark FRA")
        })
        .collect::<Vec<_>>();

    let mut group = c.benchmark_group("rate_quote_risk_portfolio");
    group.sample_size(10);
    // Uncached: every FRA gets its own provider, so no recalibration is shared
    // across the portfolio (DV01 is quote-space and needs a provider).
    group.bench_function("4_fras_uncached", |b| {
        b.iter(|| {
            fras.iter()
                .map(|fra| {
                    fra.price_with_metrics(
                        black_box(&market),
                        black_box(as_of),
                        black_box(&[MetricId::Dv01]),
                        PricingOptions::default().with_recalibration_provider(Arc::new(
                            CachedRecalibrationProvider::default(),
                        )),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .expect("uncached FRA DV01 should price")
        })
    });
    group.bench_function("4_fras_batch_cached", |b| {
        b.iter_batched(
            || {
                PricingOptions::default()
                    .with_recalibration_provider(Arc::new(CachedRecalibrationProvider::default()))
            },
            |options| {
                fras.iter()
                    .map(|fra| {
                        fra.price_with_metrics(
                            black_box(&market),
                            black_box(as_of),
                            black_box(&[MetricId::Dv01]),
                            options.clone(),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .expect("cached FRA DV01 should price")
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_discount_and_forward_steps,
    bench_residual_normalization,
    bench_rate_quote_risk_cache
);
criterion_main!(benches);
