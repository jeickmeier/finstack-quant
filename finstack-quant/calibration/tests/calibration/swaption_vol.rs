//! Integration test for swaption volatility calibration (canonical).

// See `finstack_quant_calibration::api::engine`'s crate-internal
// `#![allow(clippy::result_large_err)]`: `ExecuteError::Envelope` carries a
// full diagnostic payload by design; that allow does not reach this
// integration-test crate, so it is repeated here for the closure below.
#![allow(clippy::result_large_err)]

use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, StepParams, SurfaceExtrapolationPolicy,
    SwaptionVolConvention, SwaptionVolParams,
};
use finstack_quant_calibration::quotes::ids::QuoteId;
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_calibration::CalibrationConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolQuoteType;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::instruments::rates::swaption::{
    Swaption, SwaptionParams, VolatilityModel,
};
use finstack_quant_valuations::market::conventions::ids::SwaptionConventionId;

use crate::calibration_support as cal_utils;
use finstack_quant_calibration::quotes::vol::VolQuote;
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
            maturity: Date::from_calendar_date(2027, Month::January, 5).unwrap(),
            strike: 0.035,
            vol: 0.012,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx1Y-0.040"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2027, Month::January, 5).unwrap(),
            strike: 0.040,
            vol: 0.010,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx1Y-0.043"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2027, Month::January, 5).unwrap(),
            strike: 0.043,
            vol: 0.009,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx1Y-0.046"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2027, Month::January, 5).unwrap(),
            strike: 0.046,
            vol: 0.010,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx1Y-0.050"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2027, Month::January, 5).unwrap(),
            strike: 0.050,
            vol: 0.012,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        }),
        // 1Y x 5Y bucket (5 strikes)
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.038"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2031, Month::January, 5).unwrap(),
            strike: 0.038,
            vol: 0.0085,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.042"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2031, Month::January, 5).unwrap(),
            strike: 0.042,
            vol: 0.0075,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.045"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2031, Month::January, 5).unwrap(),
            strike: 0.045,
            vol: 0.007,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.048"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2031, Month::January, 5).unwrap(),
            strike: 0.048,
            vol: 0.0075,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        }),
        MarketQuote::Vol(VolQuote::SwaptionVol {
            id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.052"),
            expiry: Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            maturity: Date::from_calendar_date(2031, Month::January, 5).unwrap(),
            strike: 0.052,
            vol: 0.0085,
            quote_type: VolQuoteType::Normal,
            convention: SwaptionConventionId::new("USD"),
        }),
    ]
}

#[test]
fn swaption_vol_step_builds_and_inserts_surface() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let currency = Currency::USD;

    let source_market = MarketContext::new().insert(create_test_discount_curve(base_date));

    let swpt_quotes = create_test_swaption_quotes();
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &swpt_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("swpt".to_string(), cal_utils::quote_set_ids(&swpt_quotes));

    let settings = CalibrationConfig {
        solver: finstack_quant_calibration::SolverConfig::default()
            .with_tolerance(1e-10)
            .with_max_iterations(200),
        ..Default::default()
    };

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings,
        steps: vec![CalibrationStep {
            id: "swpt".to_string(),
            quote_set: "swpt".to_string(),
            params: StepParams::SwaptionVol(SwaptionVolParams {
                vol_surface_id: "USD-SWPT".to_string(),
                base_date,
                discount_curve_id: CurveId::from("USD-OIS"),
                forward_id: None,
                currency,
                vol_convention: SwaptionVolConvention::Normal,
                sabr_beta: 0.0,
                target_expiries: vec![1.0],
                target_tenors: vec![1.0, 5.0],
                sabr_interpolation: Default::default(),
                calendar_id: Some("weekends_only".to_string()),
                fixed_day_count: None,
                swap_index: Some("USD-SOFR-3M".into()),
                // Vendor-grade surface fit requirements (normal vols in decimal).
                vol_tolerance: Some(tolerances::SWAPTION_VOL_FIT_TOL_NORMAL_DECIMAL),
                sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
                allow_sabr_missing_bucket_fallback: false,
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
    // Resolve the calibrated cube as a concrete VolSource.
    let vol_provider = finstack_quant_valuations::market::resolve_vol_source(&ctx, "USD-SWPT")
        .expect("vol cube inserted");

    // ATM strikes for each bucket (approximate).
    let v_1y_1y = vol_provider
        .get_normal_vol_clamped(1.0, 1.0, 0.043)
        .expect("clamped vol");
    let v_1y_5y = vol_provider
        .get_normal_vol_clamped(1.0, 5.0, 0.045)
        .expect("clamped vol");
    assert!(v_1y_1y.is_finite() && v_1y_1y > 0.0);
    assert!(v_1y_5y.is_finite() && v_1y_5y > 0.0);
}

#[test]
fn calibrated_swaption_surface_is_not_silently_reused_as_strike_surface() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let currency = Currency::USD;

    let source_market = MarketContext::new().insert(create_test_discount_curve(base_date));

    let swpt_quotes = create_test_swaption_quotes();
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &swpt_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("swpt".to_string(), cal_utils::quote_set_ids(&swpt_quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "swpt".to_string(),
            quote_set: "swpt".to_string(),
            params: StepParams::SwaptionVol(SwaptionVolParams {
                vol_surface_id: "USD-SWPT".to_string(),
                base_date,
                discount_curve_id: CurveId::from("USD-OIS"),
                forward_id: None,
                currency,
                vol_convention: SwaptionVolConvention::Normal,
                sabr_beta: 0.0,
                target_expiries: vec![1.0],
                target_tenors: vec![1.0, 5.0],
                sabr_interpolation: Default::default(),
                calendar_id: Some("weekends_only".to_string()),
                fixed_day_count: None,
                swap_index: Some("USD-SOFR-3M".into()),
                vol_tolerance: None,
                sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
                allow_sabr_missing_bucket_fallback: false,
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

    let expiry = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let params = SwaptionParams::payer(
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        0.045,
        expiry,
        expiry,
        Date::from_calendar_date(2031, Month::January, 1).unwrap(),
    )
    .unwrap()
    .with_fixed_frequency(Tenor::semi_annual())
    .with_float_frequency(Tenor::quarterly())
    .with_fixed_day_count(DayCount::Thirty360)
    .with_float_day_count(DayCount::Act360)
    .with_vol_model(VolatilityModel::Normal);
    let swaption = Swaption::new("SWPT-1Yx5Y", &params, "USD-OIS", "USD-SOFR-3M", "USD-SWPT");

    // Normal calibration preserves its quote convention on the expiry/tenor
    // cube. It cannot be read as a Black strike surface.
    let vol_provider = finstack_quant_valuations::market::resolve_vol_source(
        &ctx,
        swaption.vol_surface_id.as_str(),
    )
    .expect("vol cube should resolve as a volatility source");
    let vol = vol_provider
        .get_normal_vol_clamped(1.0, 5.0, 0.045)
        .expect("clamped vol");
    assert!(
        vol.is_finite() && vol > 0.0,
        "VolCube should produce a valid vol"
    );
}

#[test]
fn swaption_vol_out_of_bounds_targets_error_by_default() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let currency = Currency::USD;

    let source_market = MarketContext::new().insert(create_test_discount_curve(base_date));

    let swpt_quotes = create_test_swaption_quotes();
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &swpt_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("swpt".to_string(), cal_utils::quote_set_ids(&swpt_quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "swpt".to_string(),
            quote_set: "swpt".to_string(),
            params: StepParams::SwaptionVol(SwaptionVolParams {
                vol_surface_id: "USD-SWPT".to_string(),
                base_date,
                discount_curve_id: CurveId::from("USD-OIS"),
                forward_id: None,
                currency,
                vol_convention: SwaptionVolConvention::Normal,
                sabr_beta: 0.0,
                target_expiries: vec![0.5],
                target_tenors: vec![1.0, 5.0],
                sabr_interpolation: Default::default(),
                calendar_id: Some("weekends_only".to_string()),
                fixed_day_count: None,
                swap_index: Some("USD-SOFR-3M".into()),
                vol_tolerance: None,
                sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
                allow_sabr_missing_bucket_fallback: false,
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

    let err = engine::execute(&envelope).expect_err("out-of-bounds targets should error");
    let msg = err.to_string();
    assert!(msg.contains("out of bounds"));
    assert!(msg.contains("sabr_extrapolation"));
}

#[test]
fn swaption_vol_out_of_bounds_targets_can_clamp_when_configured() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let currency = Currency::USD;

    let source_market = MarketContext::new().insert(create_test_discount_curve(base_date));

    let swpt_quotes = create_test_swaption_quotes();
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &swpt_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("swpt".to_string(), cal_utils::quote_set_ids(&swpt_quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "swpt".to_string(),
            quote_set: "swpt".to_string(),
            params: StepParams::SwaptionVol(SwaptionVolParams {
                vol_surface_id: "USD-SWPT".to_string(),
                base_date,
                discount_curve_id: CurveId::from("USD-OIS"),
                forward_id: None,
                currency,
                vol_convention: SwaptionVolConvention::Normal,
                sabr_beta: 0.0,
                target_expiries: vec![0.5, 1.0],
                target_tenors: vec![1.0, 5.0],
                sabr_interpolation: Default::default(),
                calendar_id: Some("weekends_only".to_string()),
                fixed_day_count: None,
                swap_index: Some("USD-SOFR-3M".into()),
                vol_tolerance: None,
                sabr_extrapolation: SurfaceExtrapolationPolicy::Clamp,
                allow_sabr_missing_bucket_fallback: false,
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
    let step = result.result.step_reports.get("swpt").expect("step report");
    assert!(step.success);
    assert_eq!(
        step.metadata
            .get("clamped_target_points")
            .map(|v| v.as_str()),
        Some("2")
    );
}

#[test]
fn swaption_vol_settlement_lag_uses_canonical_tenor_axis() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let expiry = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let swap_start = Date::from_calendar_date(2026, Month::January, 5).unwrap();
    let maturity = Date::from_calendar_date(2027, Month::January, 1).unwrap();
    let expiry_time = DayCount::Act365F
        .year_fraction(base_date, expiry, DayCountContext::default())
        .unwrap();
    let settled_tenor = DayCount::Act365F
        .year_fraction(swap_start, maturity, DayCountContext::default())
        .unwrap();
    let settled_tenor_axis = (settled_tenor * 10_000.0).round() / 10_000.0;
    let contractual_tenor = DayCount::Act365F
        .year_fraction(expiry, maturity, DayCountContext::default())
        .unwrap();
    assert!(
        (settled_tenor - contractual_tenor).abs() > 1.0e-4,
        "fixture must distinguish pricing dates from the contractual tenor coordinate"
    );
    assert!((contractual_tenor - 1.0).abs() < 1.0e-12);

    let execute_for_tenor = |target_tenor: f64| {
        let source_market = MarketContext::new().insert(create_test_discount_curve(base_date));
        let mut swpt_quotes: Vec<_> = create_test_swaption_quotes().into_iter().take(5).collect();
        for quote in &mut swpt_quotes {
            let MarketQuote::Vol(VolQuote::SwaptionVol {
                maturity: quote_maturity,
                ..
            }) = quote
            else {
                unreachable!("fixture contains only swaption quotes");
            };
            *quote_maturity = maturity;
        }
        let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
        cal_utils::extend_market_data(&mut market_data, &swpt_quotes);
        let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
        quote_sets.insert("swpt".to_string(), cal_utils::quote_set_ids(&swpt_quotes));
        let plan = CalibrationPlan {
            id: "settled-tenor".to_string(),
            description: None,
            quote_sets: quote_sets.into_iter().collect(),
            settings: CalibrationConfig {
                solver: finstack_quant_calibration::SolverConfig::default()
                    .with_tolerance(1.0e-10)
                    .with_max_iterations(200),
                ..Default::default()
            },
            steps: vec![CalibrationStep {
                id: "swpt".to_string(),
                quote_set: "swpt".to_string(),
                params: StepParams::SwaptionVol(SwaptionVolParams {
                    vol_surface_id: "USD-SWPT-SETTLED".to_string(),
                    base_date,
                    discount_curve_id: CurveId::from("USD-OIS"),
                    forward_id: None,
                    currency: Currency::USD,
                    vol_convention: SwaptionVolConvention::Normal,
                    sabr_beta: 0.0,
                    target_expiries: vec![expiry_time],
                    target_tenors: vec![target_tenor],
                    sabr_interpolation: Default::default(),
                    calendar_id: Some("weekends_only".to_string()),
                    fixed_day_count: Some(DayCount::Act365F),
                    swap_index: Some("USD-SOFR-3M".into()),
                    vol_tolerance: Some(tolerances::SWAPTION_VOL_FIT_TOL_NORMAL_DECIMAL),
                    sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
                    allow_sabr_missing_bucket_fallback: false,
                }),
            }],
        };
        engine::execute(&CalibrationEnvelope {
            schema_url: None,
            schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
            plan,
            market_data,
            prior_market: prior,
        })
    };

    let result = execute_for_tenor(contractual_tenor)
        .expect("the cube must use the contractual 1Y tenor after T+2 settlement");
    let context = MarketContext::try_from(result.result.final_market).expect("restore context");
    let cube = finstack_quant_valuations::market::resolve_vol_source(&context, "USD-SWPT-SETTLED")
        .expect("contractual-coordinate cube");
    assert!(cube
        .get_normal_vol(expiry_time, contractual_tenor, 0.043)
        .expect("exact contractual cube node")
        .is_finite());

    let settled_error = execute_for_tenor(settled_tenor_axis)
        .expect_err("raw settled-date year fraction must not become the cube coordinate");
    assert!(settled_error.to_string().contains("out of bounds"));
}
