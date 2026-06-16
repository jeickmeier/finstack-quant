//! Integration tests for inflation calibration conventions (v2).

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, InflationLag,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::calibration::api::engine;
use finstack_quant_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, InflationCurveParams, StepParams,
};
use finstack_quant_valuations::market::conventions::ids::InflationSwapConventionId;
use finstack_quant_valuations::market::quotes::ids::QuoteId;
use finstack_quant_valuations::market::quotes::inflation::InflationQuote;
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;

use crate::finstack_quant_test_utils::calibration as cal_utils;
use time::Month;

use super::tolerances::{assert_close_abs, F64_ABS_TOL_LOOSE};

fn create_discount_curve(base_date: Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.96),
            (3.0, 0.88),
            (5.0, 0.82),
            (10.0, 0.68),
        ])
        .build()
        .expect("discount curve")
}

fn create_us_cpi_fixings_with_seasonality() -> InflationIndex {
    let observations = vec![
        (
            Date::from_calendar_date(2024, Month::September, 30).expect("date"),
            300.0,
        ),
        (
            Date::from_calendar_date(2024, Month::October, 31).expect("date"),
            301.0,
        ),
        (
            Date::from_calendar_date(2024, Month::November, 30).expect("date"),
            302.0,
        ),
        (
            Date::from_calendar_date(2024, Month::December, 31).expect("date"),
            303.0,
        ),
    ];

    // Simple seasonality pattern: October +1%, others neutral.
    let mut factors = [1.0_f64; 12];
    factors[(Month::October as usize) - 1] = 1.01;

    InflationIndex::new("USD-CPI", observations, Currency::USD)
        .expect("index")
        .with_interpolation(InflationInterpolation::Linear)
        .with_lag(InflationLag::Months(3))
        .with_seasonality(factors)
        .expect("seasonality")
}

#[test]
fn inflation_quote_time_uses_lagged_fixing_date() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).expect("base_date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("maturity");
    let fixing_date = maturity.add_months(-3);
    let expected_t = DayCount::Act365F
        .year_fraction(base_date, fixing_date, DayCountContext::default())
        .expect("t");

    let quotes = vec![MarketQuote::Inflation(InflationQuote::InflationSwap {
        id: QuoteId::new("USA-CPI-U-ZCIS-20300115"),
        maturity,
        rate: 0.02,
        index: "USA-CPI-U".to_string(),
        convention: InflationSwapConventionId::new("USD"),
    })];
    let initial_market = MarketContext::new().insert(create_discount_curve(base_date));
    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("infl".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "infl".to_string(),
            quote_set: "infl".to_string(),
            params: StepParams::Inflation(InflationCurveParams {
                curve_id: "USD-CPI".into(),
                currency: Currency::USD,
                base_date,
                discount_curve_id: "USD-OIS".into(),
                index: "USA-CPI-U".to_string(),
                observation_lag: "3M".to_string(),
                base_cpi: 100.0,
                notional: 1.0,
                method: Default::default(),
                interpolation: Default::default(),
                seasonal_factors: None,
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
    let ctx = MarketContext::try_from(result.result.final_market).expect("restore context");
    let curve = ctx.get_inflation_curve("USD-CPI").expect("inflation curve");

    assert_eq!(curve.knots().first().copied(), Some(0.0));
    assert_eq!(curve.knots().len(), 2);
    assert_close_abs(
        curve.knots()[1],
        expected_t,
        F64_ABS_TOL_LOOSE,
        "inflation knot time should match expected lag-adjusted time-axis",
    );
}

#[test]
fn inflation_preflight_rejects_base_cpi_mismatch_with_fixings() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).expect("base_date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("maturity");

    let quotes = vec![MarketQuote::Inflation(InflationQuote::InflationSwap {
        id: QuoteId::new("USA-CPI-U-ZCIS-20300115"),
        maturity,
        rate: 0.02,
        index: "USA-CPI-U".to_string(),
        convention: InflationSwapConventionId::new("USD"),
    })];
    let initial_market = MarketContext::new()
        .insert(create_discount_curve(base_date))
        .insert_inflation_index("USD-CPI", create_us_cpi_fixings_with_seasonality());
    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("infl".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "infl".to_string(),
            quote_set: "infl".to_string(),
            params: StepParams::Inflation(InflationCurveParams {
                curve_id: "USD-CPI".into(),
                currency: Currency::USD,
                base_date,
                discount_curve_id: "USD-OIS".into(),
                index: "USA-CPI-U".to_string(),
                observation_lag: "3M".to_string(),
                base_cpi: 100.0, // intentionally wrong when fixings are provided
                notional: 1.0,
                method: Default::default(),
                interpolation: Default::default(),
                seasonal_factors: None,
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

    let err = engine::execute(&envelope).expect_err("base CPI mismatch should error");
    let msg = err.to_string();
    assert!(msg.contains("base_cpi mismatch") || msg.contains("base_cpi"));
}
