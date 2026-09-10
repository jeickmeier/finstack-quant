//! Integration tests for inflation calibration conventions (canonical).

use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, InflationCurveParams, StepParams,
};
use finstack_quant_calibration::quotes::ids::QuoteId;
use finstack_quant_calibration::quotes::inflation::InflationQuote;
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, InflationLag,
};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::market::conventions::ids::InflationSwapConventionId;

use crate::calibration_support as cal_utils;
use time::Month;

use super::tolerances::F64_ABS_TOL_LOOSE;
use finstack_quant_test_utils::assert::approx_eq;

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

fn calibrated_market(index: Option<InflationIndex>, base_cpi: f64) -> (MarketContext, Date, Date) {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).expect("base_date");
    let maturity = Date::from_calendar_date(2030, Month::January, 15).expect("maturity");
    let quotes = vec![MarketQuote::Inflation(InflationQuote::InflationSwap {
        id: QuoteId::new("USA-CPI-U-ZCIS-20300115"),
        maturity,
        rate: 0.02,
        index: "USA-CPI-U".to_string(),
        convention: InflationSwapConventionId::new("USD"),
    })];
    let mut source_market = MarketContext::new().insert(create_discount_curve(base_date));
    if let Some(index) = index {
        source_market = source_market.insert_inflation_index("USD-CPI", index);
    }
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("infl".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
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
                base_cpi,
                notional: 1.0,
                method: Default::default(),
                interpolation: Default::default(),
                seasonal_factors: None,
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

    let result = engine::execute(&envelope).expect("execute");
    let ctx = MarketContext::try_from(result.result.final_market).expect("restore context");
    (ctx, base_date, maturity)
}

#[test]
fn inflation_quote_time_uses_lagged_fixing_date() {
    let (ctx, base_date, maturity) = calibrated_market(None, 100.0);
    let curve = ctx.get_inflation_curve("USD-CPI").expect("inflation curve");
    let fixing_date = maturity.replace_day(1).expect("month start").add_months(-2);
    let reference_date = base_date.add_months(-3);
    let expected_t = DayCount::Act365F
        .year_fraction(reference_date, fixing_date, DayCountContext::default())
        .expect("clock");
    assert_eq!(curve.knots().first().copied(), Some(0.0));
    assert_eq!(curve.knots().len(), 2);
    approx_eq(
        curve.knots()[1],
        expected_t,
        F64_ABS_TOL_LOOSE,
        "lagged pillar time",
    );
}

#[test]
fn b17_inflation_curve_origin_matches_reference_cpi_date() {
    let (ctx, base_date, maturity) = calibrated_market(None, 100.0);
    let curve = ctx.get_inflation_curve("USD-CPI").expect("inflation curve");
    let reference_date = base_date.add_months(-3);
    assert_eq!(curve.base_date(), reference_date);
    assert_eq!(
        curve.cpi_on_date(reference_date).expect("reference CPI"),
        100.0
    );
    let anchor0 = maturity.replace_day(1).expect("month start").add_months(-3);
    let cpi0 = curve.cpi_on_date(anchor0).expect("first CPI");
    let cpi1 = curve
        .cpi_on_date(anchor0.add_months(1))
        .expect("second CPI");
    let weight =
        f64::from(maturity.day() - 1) / f64::from(maturity.month().length(maturity.year()));
    let reference_end = cpi0 + weight * (cpi1 - cpi0);
    let expected = 100.0 * 1.02_f64.powi(5);
    assert!(
        (reference_end - expected).abs() < 1e-7,
        "{reference_end} vs {expected}"
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
    let source_market = MarketContext::new()
        .insert(create_discount_curve(base_date))
        .insert_inflation_index("USD-CPI", create_us_cpi_fixings_with_seasonality());
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("infl".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
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

        schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let err = engine::execute(&envelope).expect_err("base CPI mismatch should error");
    let msg = err.to_string();
    assert!(msg.contains("base_cpi mismatch") || msg.contains("base_cpi"));
}

#[test]
fn b17_inflation_calibration_matches_observed_and_explicit_base_cpi() {
    use time::macros::date;
    let index = InflationIndex::new(
        "USD-CPI",
        vec![
            (date!(2024 - 10 - 31), 300.0),
            (date!(2024 - 11 - 30), 303.0),
        ],
        Currency::USD,
    )
    .expect("monthly CPI")
    .with_lag(InflationLag::Months(3))
    .with_interpolation(InflationInterpolation::Linear);
    let reference_cpi = 300.0 + 3.0 * 14.0 / 31.0;
    let (explicit, _, _) = calibrated_market(None, reference_cpi);
    let (observed, _, _) = calibrated_market(Some(index), reference_cpi);
    let left = explicit
        .get_inflation_curve("USD-CPI")
        .expect("explicit curve");
    let right = observed
        .get_inflation_curve("USD-CPI")
        .expect("observed curve");
    assert_eq!(left.base_date(), date!(2024 - 10 - 15));
    assert_eq!(left.base_date(), right.base_date());
    assert_eq!(left.knots(), right.knots());
    for t in [0.0, 0.1, 1.0, 3.0, 5.0] {
        assert!((left.cpi(t) - right.cpi(t)).abs() < 1e-9, "{t}");
    }
}
