//! Explainability tests for calibration canonical.
//!
//! canonical captures explainability traces at the per-step report level.

use crate::calibration::calibration_support as cal_utils;
use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, ForwardCurveParams, StepParams,
};
use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_calibration::quotes::rates::RateQuote;
use finstack_quant_calibration::{CalibrationConfig, CalibrationMethod};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::create_date;
use finstack_quant_core::explain::TraceEntry;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::HashMap;
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
    let (prior, mut market_data) = cal_utils::split_market_context(&ctx);
    cal_utils::extend_market_data(&mut market_data, &fwd_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("fwd".to_string(), cal_utils::quote_set_ids(&fwd_quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
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
                method: CalibrationMethod::GlobalSolve {
                    use_analytical_jacobian: false,
                },
                interpolation: Default::default(),
                conventions: Default::default(),
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::calibrate(&envelope).expect("execute");
    let step = result.result.step_reports.get("fwd").expect("step report");

    assert!(step.explanation.is_none());
}

#[test]
fn explanation_is_present_when_enabled() {
    let base_date = create_date(2025, Month::January, 15).unwrap();

    let ctx = MarketContext::new().insert(base_discount_curve(base_date));
    let fwd_quotes = forward_quotes();
    let (prior, mut market_data) = cal_utils::split_market_context(&ctx);
    cal_utils::extend_market_data(&mut market_data, &fwd_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("fwd".to_string(), cal_utils::quote_set_ids(&fwd_quotes));

    // The wire form is how hosts enable tracing: a JSON plan setting.
    let settings: CalibrationConfig = serde_json::from_value(serde_json::json!({
        "explain": {"enabled": true},
        "compute_diagnostics": true,
    }))
    .expect("explain is a wire setting");
    assert!(settings.explain.enabled && settings.explain.max_entries.is_none());

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
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
                method: CalibrationMethod::GlobalSolve {
                    use_analytical_jacobian: false,
                },
                interpolation: Default::default(),
                conventions: Default::default(),
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::calibrate(&envelope).expect("execute");
    let step = result.result.step_reports.get("fwd").expect("step report");

    assert!(step.success);
    let trace = step.explanation.as_ref().expect("step trace");
    let TraceEntry::ComputationStep { name, metadata, .. } = &trace.entries[0] else {
        panic!("unexpected entry {:?}", trace.entries[0]);
    };
    assert_eq!(name, "global_solve");
    let metadata = metadata.as_ref().expect("solve metadata");
    assert_eq!(
        metadata["solved_params"].as_array().map(Vec::len),
        metadata["times"].as_array().map(Vec::len)
    );

    // The plan-level trace nests every step trace under its step marker.
    let plan_trace = result
        .result
        .report
        .explanation
        .as_ref()
        .expect("plan trace");
    assert!(plan_trace.entries.iter().any(
        |entry| matches!(entry, TraceEntry::ComputationStep { name, .. } if name == "global_solve")
    ));

    // Per-quote rows carry the market quote next to the residual.
    let rows = step.quote_rows();
    let quoted: Vec<_> = rows.iter().map(|row| row.quote_value).collect();
    assert_eq!(quoted, [Some(0.045), Some(0.046)]);

    // The exported Jacobian has one row per quote and one column per solved
    // parameter, and reproduces each quote's reported sensitivity.
    let diagnostics = step.diagnostics.as_ref().expect("diagnostics");
    let jacobian = diagnostics
        .jacobian
        .as_ref()
        .expect("a global solve exports its Jacobian");
    let n_params = metadata["solved_params"]
        .as_array()
        .expect("solved parameters")
        .len();
    assert_eq!(jacobian.len(), diagnostics.per_quote.len());
    for (row, quote) in jacobian.iter().zip(&diagnostics.per_quote) {
        assert_eq!(row.len(), n_params);
        let max_abs = row.iter().map(|value| value.abs()).fold(0.0_f64, f64::max);
        assert!(max_abs > 0.0);
        assert_eq!(quote.sensitivity, max_abs);
    }
}

#[test]
fn disabled_explain_is_omitted_from_the_wire_form() {
    let json = serde_json::to_value(CalibrationConfig::default()).expect("serialize");
    assert!(json.get("explain").is_none());
}
