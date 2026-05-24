//! Tests for the plan-driven calibration v2 API.

use crate::finstack_test_utils::calibration as cal_utils;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::math::interp::ExtrapolationPolicy;
use finstack_core::types::IndexId;
use finstack_core::HashMap;
use finstack_valuations::calibration::api::engine;
use finstack_valuations::calibration::api::market_datum::MarketDatum;
use finstack_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, DiscountCurveParams, StepParams,
};
use finstack_valuations::calibration::{CalibrationConfig, CalibrationMethod};
use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_valuations::market::quotes::market_quote::MarketQuote;
use finstack_valuations::market::quotes::rates::RateQuote;
use time::Month;

#[test]
fn missing_quote_set_fails_fast() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let currency = Currency::USD;

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: HashMap::default(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "step_1".to_string(),
            quote_set: "does_not_exist".to_string(),
            params: StepParams::Discount(DiscountCurveParams {
                curve_id: "USD-OIS".into(),
                currency,
                base_date,
                method: CalibrationMethod::Bootstrap,
                interpolation: Default::default(),
                extrapolation: ExtrapolationPolicy::FlatForward,
                pricing_discount_id: None,
                pricing_forward_id: None,
                conventions: Default::default(),
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data: Vec::new(),
        prior_market: Vec::new(),
    };

    let err = engine::execute(&envelope).expect_err("missing quote set should error");
    assert!(matches!(err, finstack_core::Error::Input(_)));
}

#[test]
fn plan_and_envelope_serde_roundtrip() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let currency = Currency::USD;

    let usd_ois_quotes = vec![MarketQuote::Rates(RateQuote::Deposit {
        id: QuoteId::new(format!("DEP-{:?}", base_date + time::Duration::days(30))),
        index: IndexId::new("USD-Deposit"),
        pillar: Pillar::Date(base_date + time::Duration::days(30)),
        rate: 0.05,
    })];
    let mut market_data: Vec<MarketDatum> = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &usd_ois_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert(
        "usd_ois".to_string(),
        cal_utils::quote_set_ids(&usd_ois_quotes),
    );

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: Some("serde smoke".to_string()),
        quote_sets,
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "step_1".to_string(),
            quote_set: "usd_ois".to_string(),
            params: StepParams::Discount(DiscountCurveParams {
                curve_id: "USD-OIS".into(),
                currency,
                base_date,
                method: CalibrationMethod::Bootstrap,
                interpolation: Default::default(),
                extrapolation: ExtrapolationPolicy::FlatForward,
                pricing_discount_id: None,
                pricing_forward_id: None,
                conventions: Default::default(),
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let json = serde_json::to_string_pretty(&envelope).expect("serialize");
    let decoded: CalibrationEnvelope = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.schema, "finstack.calibration/2");
    assert_eq!(decoded.plan.steps.len(), 1);
}

#[test]
fn parallel_execution_batches_independent_discount_steps() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let currency = Currency::USD;

    let quotes_a = vec![
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("A-DEP-1M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("1M").unwrap()),
            rate: 0.050,
        }),
        MarketQuote::Rates(RateQuote::Swap {
            id: QuoteId::new("A-SWAP-1Y"),
            index: IndexId::new("USD-OIS"),
            pillar: Pillar::Tenor(Tenor::parse("1Y").unwrap()),
            rate: 0.052,
            spread_decimal: None,
        }),
    ];
    let quotes_b = vec![
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("B-DEP-1M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("1M").unwrap()),
            rate: 0.045,
        }),
        MarketQuote::Rates(RateQuote::Swap {
            id: QuoteId::new("B-SWAP-1Y"),
            index: IndexId::new("USD-OIS"),
            pillar: Pillar::Tenor(Tenor::parse("1Y").unwrap()),
            rate: 0.047,
            spread_decimal: None,
        }),
    ];

    let mut market_data: Vec<MarketDatum> = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &quotes_a);
    cal_utils::extend_market_data(&mut market_data, &quotes_b);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("a".to_string(), cal_utils::quote_set_ids(&quotes_a));
    quote_sets.insert("b".to_string(), cal_utils::quote_set_ids(&quotes_b));

    let discount_params = |curve_id: &str| DiscountCurveParams {
        curve_id: curve_id.into(),
        currency,
        base_date,
        method: CalibrationMethod::Bootstrap,
        interpolation: Default::default(),
        extrapolation: ExtrapolationPolicy::FlatForward,
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: Default::default(),
    };
    let plan = CalibrationPlan {
        id: "parallel-plan".to_string(),
        description: None,
        quote_sets,
        settings: CalibrationConfig {
            use_parallel: true,
            ..Default::default()
        },
        steps: vec![
            CalibrationStep {
                id: "disc_a".to_string(),
                quote_set: "a".to_string(),
                params: StepParams::Discount(discount_params("USD-OIS-A")),
            },
            CalibrationStep {
                id: "disc_b".to_string(),
                quote_set: "b".to_string(),
                params: StepParams::Discount(discount_params("USD-OIS-B")),
            },
        ],
    };
    let envelope = CalibrationEnvelope {
        schema_url: None,
        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let result = engine::execute(&envelope).expect("parallel calibration succeeds");
    assert!(result.result.report.success);
    let context = MarketContext::try_from(result.result.final_market).expect("restore context");
    context
        .get_discount("USD-OIS-A")
        .expect("first curve present");
    context
        .get_discount("USD-OIS-B")
        .expect("second curve present");
}
