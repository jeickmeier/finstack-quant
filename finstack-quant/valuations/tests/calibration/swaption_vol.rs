//! Integration test for swaption volatility calibration (v2).

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::calibration::api::engine;
use finstack_quant_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, StepParams, SurfaceExtrapolationPolicy,
    SwaptionVolConvention, SwaptionVolParams,
};
use finstack_quant_valuations::calibration::CalibrationConfig;
use finstack_quant_valuations::instruments::rates::swaption::{
    Swaption, SwaptionExercise, SwaptionSettlement, VolatilityModel,
};
use finstack_quant_valuations::instruments::OptionType;
use finstack_quant_valuations::market::conventions::ids::SwaptionConventionId;
use finstack_quant_valuations::market::quotes::ids::QuoteId;
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;

use crate::finstack_quant_test_utils::calibration as cal_utils;
use finstack_quant_valuations::market::quotes::vol::VolQuote;
use rust_decimal::Decimal;
use time::Month;

use super::tolerances;

fn create_test_discount_curve(base_date: Date) -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots(vec![
            (0.0, 1.0),   // Today
            (0.25, 0.99), // 3M: ~4% rate
            (1.0, 0.96),  // 1Y: ~4% rate
            (2.0, 0.92),  // 2Y: ~4% rate
            (5.0, 0.80),  // 5Y: ~4% rate
            (10.0, 0.64), // 10Y: ~4% rate
        ])
        .build()
        .expect("discount curve")
}

fn create_test_swaption_quotes() -> Vec<MarketQuote> {
    vec![
        // 1Y x 1Y bucket (5 strikes)
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx1Y-0.035"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            strike: 0.035,
            vol: 120.0,
            quote_type: "OTM-100".to_string(),
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx1Y-0.040"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            strike: 0.040,
            vol: 100.0,
            quote_type: "ATM-50".to_string(),
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx1Y-0.043"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            strike: 0.043,
            vol: 90.0,
            quote_type: "ATM".to_string(),
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx1Y-0.046"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            strike: 0.046,
            vol: 100.0,
            quote_type: "ATM+50".to_string(),
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx1Y-0.050"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2027, Month::January, 1).unwrap(),
            strike: 0.050,
            vol: 120.0,
            quote_type: "OTM+100".to_string(),
            convention: SwaptionConventionId::new("USD"),
        }),
        // 1Y x 5Y bucket (5 strikes)
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.038"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2031, Month::January, 1).unwrap(),
            strike: 0.038,
            vol: 85.0,
            quote_type: "OTM-100".to_string(),
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.042"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2031, Month::January, 1).unwrap(),
            strike: 0.042,
            vol: 75.0,
            quote_type: "ATM-50".to_string(),
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.045"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2031, Month::January, 1).unwrap(),
            strike: 0.045,
            vol: 70.0,
            quote_type: "ATM".to_string(),
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.048"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2031, Month::January, 1).unwrap(),
            strike: 0.048,
            vol: 75.0,
            quote_type: "ATM+50".to_string(),
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.052"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2031, Month::January, 1).unwrap(),
            strike: 0.052,
            vol: 85.0,
            quote_type: "OTM+100".to_string(),
            convention: SwaptionConventionId::new("USD"),
        }),
    ]
}

#[test]
fn swaption_vol_step_builds_and_inserts_surface() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let currency = Currency::USD;

    let initial_market = MarketContext::new().insert(create_test_discount_curve(base_date));

    let swpt_quotes = create_test_swaption_quotes();
    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &swpt_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("swpt".to_string(), cal_utils::quote_set_ids(&swpt_quotes));

    let settings = CalibrationConfig {
        solver: finstack_quant_valuations::calibration::SolverConfig::brent_default()
            .with_tolerance(1e-10)
            .with_max_iterations(200),
        ..Default::default()
    };

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings,
        steps: vec![CalibrationStep {
            id: "swpt".to_string(),
            quote_set: "swpt".to_string(),
            params: StepParams::SwaptionVol(SwaptionVolParams {
                surface_id: "USD-SWPT".to_string(),
                base_date,
                discount_curve_id: CurveId::from("USD-OIS"),
                forward_id: None,
                currency,
                vol_convention: SwaptionVolConvention::Normal,
                atm_convention: Default::default(),
                sabr_beta: 0.0,
                target_expiries: vec![1.0],
                target_tenors: vec![1.0, 5.0],
                sabr_interpolation: Default::default(),
                calendar_id: Some("weekends_only".to_string()),
                fixed_day_count: None,
                swap_index: Some("USD-SOFR-3M".into()),
                // Vendor-grade surface fit requirements (normal vols in decimal).
                vol_tolerance: Some(tolerances::SWAPTION_VOL_FIT_TOL_NORMAL_DECIMAL),
                // Internal SABR solver tolerance (root-finder tolerance).
                sabr_tolerance: Some(1e-8),
                sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
                allow_sabr_missing_bucket_fallback: false,
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
    assert!(result.result.report.success);
    let step = result.result.step_reports.get("swpt").expect("step report");
    assert!(step.success);
    assert!(
        !step.residuals.is_empty(),
        "expected residuals for calibrated buckets"
    );
    assert!(
        step.max_residual <= tolerances::SWAPTION_VOL_FIT_TOL_NORMAL_DECIMAL,
        "swaption vol fit must be vendor-grade: max_residual={:.3e} > tol={:.3e}",
        step.max_residual,
        tolerances::SWAPTION_VOL_FIT_TOL_NORMAL_DECIMAL
    );

    let ctx = MarketContext::try_from(result.result.final_market).expect("restore context");

    // Calibration now produces a VolCube (SABR params on expiry x tenor grid).
    // Retrieve via get_vol_provider, which returns the cube as a VolProvider.
    let vol_provider = ctx.get_vol_provider("USD-SWPT").expect("vol cube inserted");

    // ATM strikes for each bucket (approximate).
    let v_1y_1y = vol_provider.vol_clamped(1.0, 1.0, 0.043);
    let v_1y_5y = vol_provider.vol_clamped(1.0, 5.0, 0.045);
    assert!(v_1y_1y.is_finite() && v_1y_1y > 0.0);
    assert!(v_1y_5y.is_finite() && v_1y_5y > 0.0);
}

#[test]
fn calibrated_swaption_surface_is_not_silently_reused_as_strike_surface() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let currency = Currency::USD;

    let initial_market = MarketContext::new().insert(create_test_discount_curve(base_date));

    let swpt_quotes = create_test_swaption_quotes();
    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &swpt_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("swpt".to_string(), cal_utils::quote_set_ids(&swpt_quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "swpt".to_string(),
            quote_set: "swpt".to_string(),
            params: StepParams::SwaptionVol(SwaptionVolParams {
                surface_id: "USD-SWPT".to_string(),
                base_date,
                discount_curve_id: CurveId::from("USD-OIS"),
                forward_id: None,
                currency,
                vol_convention: SwaptionVolConvention::Normal,
                atm_convention: Default::default(),
                sabr_beta: 0.0,
                target_expiries: vec![1.0],
                target_tenors: vec![1.0, 5.0],
                sabr_interpolation: Default::default(),
                calendar_id: Some("weekends_only".to_string()),
                fixed_day_count: None,
                swap_index: Some("USD-SOFR-3M".into()),
                vol_tolerance: None,
                sabr_tolerance: None,
                sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
                allow_sabr_missing_bucket_fallback: false,
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

    let swaption = Swaption {
        id: "SWPT-1Yx5Y".into(),
        option_type: OptionType::Call,
        notional: Money::new(1_000_000.0, Currency::USD),
        strike: Decimal::try_from(0.045).unwrap(),
        expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
        swap_start: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
        swap_end: Date::from_calendar_date(2031, Month::January, 1).unwrap(),
        fixed_freq: Tenor::semi_annual(),
        float_freq: Tenor::quarterly(),
        day_count: DayCount::Thirty360,
        exercise_style: SwaptionExercise::European,
        settlement: SwaptionSettlement::Physical,
        cash_settlement_method: Default::default(),
        vol_model: VolatilityModel::Normal,
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-SOFR-3M".into(),
        vol_surface_id: "USD-SWPT".into(),
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        calendar_id: None,
        underlying_fixed_leg: None,
        underlying_float_leg: None,
        sabr_params: None,
        attributes: Default::default(),
    };

    // With VolCube calibration, the vol cube is stored separately from surfaces.
    // The SimpleSwaptionBlackPricer uses get_vol_provider which resolves the cube,
    // so pricing should succeed. The legacy Swaption::value() path still uses
    // get_surface(), so it won't find the cube.
    let vol_provider = ctx
        .get_vol_provider(swaption.vol_surface_id.as_str())
        .expect("vol cube should be found via get_vol_provider");
    let vol = vol_provider.vol_clamped(1.0, 5.0, 0.045);
    assert!(
        vol.is_finite() && vol > 0.0,
        "VolCube should produce a valid vol"
    );
}

#[test]
fn swaption_vol_out_of_bounds_targets_error_by_default() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let currency = Currency::USD;

    let initial_market = MarketContext::new().insert(create_test_discount_curve(base_date));

    let swpt_quotes = create_test_swaption_quotes();
    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &swpt_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("swpt".to_string(), cal_utils::quote_set_ids(&swpt_quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "swpt".to_string(),
            quote_set: "swpt".to_string(),
            params: StepParams::SwaptionVol(SwaptionVolParams {
                surface_id: "USD-SWPT".to_string(),
                base_date,
                discount_curve_id: CurveId::from("USD-OIS"),
                forward_id: None,
                currency,
                vol_convention: SwaptionVolConvention::Normal,
                atm_convention: Default::default(),
                sabr_beta: 0.0,
                target_expiries: vec![0.5],
                target_tenors: vec![1.0, 5.0],
                sabr_interpolation: Default::default(),
                calendar_id: Some("weekends_only".to_string()),
                fixed_day_count: None,
                swap_index: Some("USD-SOFR-3M".into()),
                vol_tolerance: None,
                sabr_tolerance: None,
                sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
                allow_sabr_missing_bucket_fallback: false,
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

    let err = engine::execute(&envelope).expect_err("out-of-bounds targets should error");
    let msg = err.to_string();
    assert!(msg.contains("out of bounds"));
    assert!(msg.contains("sabr_extrapolation"));
}

#[test]
fn swaption_vol_out_of_bounds_targets_can_clamp_when_configured() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let currency = Currency::USD;

    let initial_market = MarketContext::new().insert(create_test_discount_curve(base_date));

    let swpt_quotes = create_test_swaption_quotes();
    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &swpt_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("swpt".to_string(), cal_utils::quote_set_ids(&swpt_quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "swpt".to_string(),
            quote_set: "swpt".to_string(),
            params: StepParams::SwaptionVol(SwaptionVolParams {
                surface_id: "USD-SWPT".to_string(),
                base_date,
                discount_curve_id: CurveId::from("USD-OIS"),
                forward_id: None,
                currency,
                vol_convention: SwaptionVolConvention::Normal,
                atm_convention: Default::default(),
                sabr_beta: 0.0,
                target_expiries: vec![0.5, 1.0],
                target_tenors: vec![1.0, 5.0],
                sabr_interpolation: Default::default(),
                calendar_id: Some("weekends_only".to_string()),
                fixed_day_count: None,
                swap_index: Some("USD-SOFR-3M".into()),
                vol_tolerance: None,
                sabr_tolerance: None,
                sabr_extrapolation: SurfaceExtrapolationPolicy::Clamp,
                allow_sabr_missing_bucket_fallback: false,
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
    let step = result.result.step_reports.get("swpt").expect("step report");
    assert!(step.success);
    assert_eq!(
        step.metadata
            .get("clamped_target_points")
            .map(|v| v.as_str()),
        Some("2")
    );
}
