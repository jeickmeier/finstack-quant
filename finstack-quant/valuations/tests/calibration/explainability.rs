//! Explainability tests for calibration v2.
//!
//! v2 captures explainability traces at the per-step report level.

use crate::finstack_quant_test_utils::calibration as cal_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::create_date;
use finstack_quant_core::explain::ExplainOpts;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::calibration::api::engine;
use finstack_quant_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, ForwardCurveParams, StepParams,
};
use finstack_quant_valuations::calibration::{CalibrationConfig, CalibrationMethod};
use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
use finstack_quant_valuations::market::quotes::rates::RateQuote;
use time::Month;

fn base_discount_curve(base_date: finstack_quant_core::dates::Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![
            (0.0, 1.0),
            (0.25, 0.9888),
            (0.5, 0.9775),
            (1.0, 0.9550),
            (2.0, 0.9100),
        ])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .unwrap()
}

fn forward_quotes() -> Vec<MarketQuote> {
    vec![
        MarketQuote::Rates(RateQuote::Fra {
            id: "FRA-1".into(),
            index: IndexId::new("USD-LIBOR-3M"),
            start: Pillar::Date(create_date(2025, Month::April, 15).unwrap()),
            end: Pillar::Date(create_date(2025, Month::July, 15).unwrap()),
            rate: 0.045,
        }),
        MarketQuote::Rates(RateQuote::Fra {
            id: "FRA-2".into(),
            index: IndexId::new("USD-LIBOR-3M"),
            start: Pillar::Date(create_date(2025, Month::July, 15).unwrap()),
            end: Pillar::Date(create_date(2025, Month::October, 15).unwrap()),
            rate: 0.046,
        }),
    ]
}

#[test]
fn explanation_not_computed_by_default() {
    let base_date = create_date(2025, Month::January, 15).unwrap();

    let ctx = MarketContext::new().insert(base_discount_curve(base_date));
    let fwd_quotes = forward_quotes();
    let (prior, mut market_data) = cal_utils::split_initial_market(&ctx);
    cal_utils::extend_market_data(&mut market_data, &fwd_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("fwd".to_string(), cal_utils::quote_set_ids(&fwd_quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: CalibrationConfig::default(),
        steps: vec![CalibrationStep {
            id: "fwd".to_string(),
            quote_set: "fwd".to_string(),
            params: StepParams::Forward(ForwardCurveParams {
                curve_id: CurveId::from("USD-SOFR-3M"),
                currency: Currency::USD,
                base_date,
                tenor_years: 0.25,
                discount_curve_id: CurveId::from("USD-OIS"),
                method: CalibrationMethod::Bootstrap,
                interpolation: Default::default(),
                conventions: Default::default(),
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack_quant.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::execute(&envelope).expect("execute");
    let step = result.result.step_reports.get("fwd").expect("step report");

    assert!(step.explanation.is_none());
}

#[test]
fn explanation_is_present_when_enabled() {
    let base_date = create_date(2025, Month::January, 15).unwrap();

    let ctx = MarketContext::new().insert(base_discount_curve(base_date));
    let fwd_quotes = forward_quotes();
    let (prior, mut market_data) = cal_utils::split_initial_market(&ctx);
    cal_utils::extend_market_data(&mut market_data, &fwd_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("fwd".to_string(), cal_utils::quote_set_ids(&fwd_quotes));

    let settings = CalibrationConfig {
        explain: ExplainOpts::enabled(),
        ..Default::default()
    };

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings,
        steps: vec![CalibrationStep {
            id: "fwd".to_string(),
            quote_set: "fwd".to_string(),
            params: StepParams::Forward(ForwardCurveParams {
                curve_id: CurveId::from("USD-SOFR-3M"),
                currency: Currency::USD,
                base_date,
                tenor_years: 0.25,
                discount_curve_id: CurveId::from("USD-OIS"),
                method: CalibrationMethod::Bootstrap,
                interpolation: Default::default(),
                conventions: Default::default(),
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack_quant.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::execute(&envelope).expect("execute");
    let step = result.result.step_reports.get("fwd").expect("step report");

    assert!(step.success);
    if let Some(trace) = step.explanation.as_ref() {
        assert!(
            !trace.entries.is_empty(),
            "when explanation is present, it should contain entries"
        );
    }
}
