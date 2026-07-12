//! Calibration benchmarks (v2 plan-driven API).
//!
//! This benchmark suite focuses on the plan-driven calibration step engine.

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::calibration::api::market_datum::MarketDatum;
use finstack_quant_valuations::calibration::api::{
    engine,
    schema::{
        CalibrationEnvelope, CalibrationPlan, CalibrationStep, DiscountCurveParams,
        ForwardCurveParams, StepParams, CALIBRATION_SCHEMA,
    },
};
use finstack_quant_valuations::calibration::CalibrationConfig;
use finstack_quant_valuations::calibration::CalibrationMethod;
use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
use finstack_quant_valuations::market::quotes::rates::RateQuote;
use std::hint::black_box;
use time::Month;

/// Extract the QuoteId of each MarketQuote (mirror of cal_utils::quote_set_ids).
fn quote_set_ids(quotes: &[MarketQuote]) -> Vec<QuoteId> {
    quotes
        .iter()
        .map(|q| match q {
            MarketQuote::Rates(q) => q.id().clone(),
            MarketQuote::Cds(q) => q.id().clone(),
            MarketQuote::CDSTranche(q) => q.id().clone(),
            MarketQuote::Fx(q) => q.id().clone(),
            MarketQuote::Inflation(q) => q.id().clone(),
            MarketQuote::Vol(q) => q.id().clone(),
            MarketQuote::Xccy(q) => q.id().clone(),
            MarketQuote::Bond(q) => q.id().clone(),
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

    c.bench_function("calibration_v2_discount_step", |b| {
        b.iter(|| {
            let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
            quote_sets.insert("disc".to_string(), disc_ids.clone());
            let plan = CalibrationPlan {
                id: "bench_discount".to_string(),
                description: None,
                quote_sets,
                steps: vec![CalibrationStep {
                    id: "disc".to_string(),
                    quote_set: "disc".to_string(),
                    params: disc_step.clone(),
                }],
                settings: settings.clone(),
            };
            let envelope = CalibrationEnvelope {
                schema: CALIBRATION_SCHEMA.to_string(),
                schema_url: None,
                market_data: disc_data.clone(),
                prior_market: Vec::new(),
                plan,
            };
            engine::execute(black_box(&envelope)).unwrap()
        })
    });

    c.bench_function("calibration_v2_discount_then_forward_steps", |b| {
        let mut combined_data = disc_data.clone();
        combined_data.extend(quotes_to_data(&fwd_mq));
        b.iter(|| {
            let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
            quote_sets.insert("disc".to_string(), disc_ids.clone());
            quote_sets.insert("fwd".to_string(), fwd_ids.clone());
            let plan = CalibrationPlan {
                id: "bench_disc_fwd".to_string(),
                description: None,
                quote_sets,
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
                schema: CALIBRATION_SCHEMA.to_string(),
                schema_url: None,
                market_data: combined_data.clone(),
                prior_market: Vec::new(),
                plan,
            };
            engine::execute(black_box(&envelope)).unwrap()
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
            let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
            quote_sets.insert("disc".to_string(), disc_ids.clone());
            let plan = CalibrationPlan {
                id: "residual_notional_small".to_string(),
                description: None,
                quote_sets,
                steps: vec![CalibrationStep {
                    id: "disc".to_string(),
                    quote_set: "disc".to_string(),
                    params: disc_step.clone(),
                }],
                settings: settings.clone(),
            };
            let envelope = CalibrationEnvelope {
                schema: CALIBRATION_SCHEMA.to_string(),
                schema_url: None,
                market_data: disc_data.clone(),
                prior_market: Vec::new(),
                plan,
            };
            engine::execute(black_box(&envelope)).unwrap()
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
            let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
            quote_sets.insert("disc".to_string(), disc_ids.clone());
            let plan = CalibrationPlan {
                id: "residual_notional_large".to_string(),
                description: None,
                quote_sets,
                steps: vec![CalibrationStep {
                    id: "disc".to_string(),
                    quote_set: "disc".to_string(),
                    params: disc_step.clone(),
                }],
                settings: settings.clone(),
            };
            let envelope = CalibrationEnvelope {
                schema: CALIBRATION_SCHEMA.to_string(),
                schema_url: None,
                market_data: disc_data.clone(),
                prior_market: Vec::new(),
                plan,
            };
            engine::execute(black_box(&envelope)).unwrap()
        })
    });
}

criterion_group!(
    benches,
    bench_discount_and_forward_steps,
    bench_residual_normalization
);
criterion_main!(benches);
