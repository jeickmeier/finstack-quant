//! Serialization tests for the plan-driven calibration v2 API.
//!
//! v2 introduces a strict JSON contract for plan-driven execution:
//! - `CalibrationEnvelope` and `CalibrationPlan` (`deny_unknown_fields`)
//! - `StepParams` for discount/forward/hazard/inflation/vol/swaption/base-correlation
//! - Domain quotes (`MarketQuote` + concrete quote enums)

use crate::finstack_test_utils::calibration as cal_utils;
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::dates::{BusinessDayConvention, DayCount, Tenor};
use finstack_core::market_data::term_structures::Seniority;
use finstack_core::math::interp::ExtrapolationPolicy;
use finstack_core::types::CurveId;
use finstack_core::types::IndexId;
use finstack_core::HashMap;
use finstack_valuations::calibration::api::market_datum::MarketDatum;
use finstack_valuations::calibration::api::schema::{
    BaseCorrelationParams, CalibrationEnvelope, CalibrationPlan, CalibrationStep,
    DiscountCurveParams, ForwardCurveParams, HazardCurveParams, HullWhiteStepParams,
    InflationCurveParams, StepParams, SurfaceExtrapolationPolicy, SviSurfaceParams,
    SwaptionVolParams, VolSurfaceParams,
};
use finstack_valuations::calibration::CalibrationMethod;
use finstack_valuations::market::conventions::ids::{
    CdsConventionKey, CdsDocClause, SwaptionConventionId,
};
use finstack_valuations::market::quotes::cds::CdsQuote;
use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_valuations::market::quotes::market_quote::MarketQuote;
use finstack_valuations::market::quotes::rates::RateQuote;
use finstack_valuations::market::quotes::vol::VolQuote;
use time::Month;

fn maybe_print_json(json: &str) {
    if std::env::var("FINSTACK_TEST_LOG_JSON").is_ok() {
        println!("JSON representation:\n{}\n", json);
    }
}

fn roundtrip_json<T>(value: &T) -> T
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let json = serde_json::to_string_pretty(value).expect("serialize");
    maybe_print_json(&json);
    serde_json::from_str(&json).expect("deserialize")
}

#[test]
fn envelope_v2_roundtrips() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let currency = Currency::USD;

    let rates_quotes = vec![MarketQuote::Rates(RateQuote::Deposit {
        id: QuoteId::new(format!("DEP-{:?}", base_date + time::Duration::days(30))),
        index: IndexId::new("USD-Deposit"),
        pillar: Pillar::Date(base_date + time::Duration::days(30)),
        rate: 0.05,
    })];

    let mut market_data: Vec<MarketDatum> = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &rates_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("rates".to_string(), cal_utils::quote_set_ids(&rates_quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "step_1".to_string(),
            quote_set: "rates".to_string(),
            params: StepParams::Discount(DiscountCurveParams {
                curve_id: CurveId::from("USD-OIS"),
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

    let decoded = roundtrip_json(&envelope);
    assert_eq!(decoded.schema, "finstack.calibration/2");
    assert_eq!(decoded.plan.steps.len(), 1);
}

#[test]
fn step_params_v2_roundtrip_for_all_variants() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let currency = Currency::USD;

    let discount = StepParams::Discount(DiscountCurveParams {
        curve_id: "USD-OIS".into(),
        currency,
        base_date,
        method: CalibrationMethod::Bootstrap,
        interpolation: Default::default(),
        extrapolation: ExtrapolationPolicy::FlatForward,
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: Default::default(),
    });
    let _ = roundtrip_json(&discount);

    let forward = StepParams::Forward(ForwardCurveParams {
        curve_id: "USD-3M".into(),
        currency,
        base_date,
        tenor_years: 0.25,
        discount_curve_id: "USD-OIS".into(),
        method: CalibrationMethod::Bootstrap,
        interpolation: Default::default(),
        conventions: Default::default(),
    });
    let _ = roundtrip_json(&forward);

    let hazard = StepParams::Hazard(HazardCurveParams {
        curve_id: "ACME-SENIOR".into(),
        entity: "ACME".to_string(),
        seniority: Seniority::Senior,
        currency,
        base_date,
        discount_curve_id: "USD-OIS".into(),
        recovery_rate: 0.40,
        notional: 1.0,
        method: CalibrationMethod::Bootstrap,
        interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
        par_interp: finstack_core::market_data::term_structures::ParInterp::Linear,
        doc_clause: None,
        cds_valuation_convention: None,
    });
    let _ = roundtrip_json(&hazard);

    let inflation = StepParams::Inflation(InflationCurveParams {
        curve_id: "USD-CPI".into(),
        currency,
        base_date,
        discount_curve_id: "USD-OIS".into(),
        index: "USA-CPI-U".to_string(),
        observation_lag: "3M".to_string(),
        base_cpi: 100.0,
        notional: 1.0,
        method: CalibrationMethod::Bootstrap,
        interpolation: Default::default(),
        seasonal_factors: None,
    });
    let _ = roundtrip_json(&inflation);

    let vol_surface = StepParams::VolSurface(VolSurfaceParams {
        surface_id: "SPX-VOL".to_string(),
        base_date,
        underlying_ticker: "SPX".to_string(),
        model: "SABR".to_string(),
        discount_curve_id: Some("USD-OIS".into()),
        beta: 0.5,
        target_expiries: vec![0.5, 1.0],
        target_strikes: vec![0.9, 1.0, 1.1],
        spot_override: Some(100.0),
        dividend_yield_override: Some(0.01),
        expiry_extrapolation: SurfaceExtrapolationPolicy::Error,
    });
    let _ = roundtrip_json(&vol_surface);

    let swaption_vol = StepParams::SwaptionVol(SwaptionVolParams {
        surface_id: "USD-SWPT".to_string(),
        base_date,
        discount_curve_id: "USD-OIS".into(),
        forward_id: None,
        currency,
        vol_convention: Default::default(),
        atm_convention: Default::default(),
        sabr_beta: 0.0,
        target_expiries: vec![1.0, 2.0],
        target_tenors: vec![1.0, 5.0],
        sabr_interpolation: Default::default(),
        calendar_id: None,
        fixed_day_count: None,
        swap_index: Some("USD-SOFR-3M".into()),
        vol_tolerance: None,
        sabr_tolerance: None,
        sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
        allow_sabr_missing_bucket_fallback: false,
    });
    let _ = roundtrip_json(&swaption_vol);

    let base_corr = StepParams::BaseCorrelation(BaseCorrelationParams {
        index_id: "CDX".to_string(),
        series: 40,
        maturity_years: 5.0,
        base_date,
        discount_curve_id: "USD-OIS".into(),
        currency,
        notional: 1.0,
        frequency: Some(Tenor::quarterly()),
        day_count: Some(DayCount::Act360),
        bdc: Some(BusinessDayConvention::Following),
        calendar_id: Some("usny".to_string()),
        detachment_points: vec![0.03, 0.07, 0.1],
        use_imm_dates: true,
    });
    let _ = roundtrip_json(&base_corr);
}

#[test]
fn market_quote_roundtrip_smoke() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).unwrap();

    let rq = MarketQuote::Rates(RateQuote::Deposit {
        id: QuoteId::new(format!("DEP-{:?}", base_date + time::Duration::days(30))),
        index: IndexId::new("USD-Deposit"),
        pillar: Pillar::Date(base_date + time::Duration::days(30)),
        rate: 0.05,
    });
    let _ = roundtrip_json(&rq);

    let cq = MarketQuote::Cds(CdsQuote::CdsParSpread {
        id: QuoteId::new(format!(
            "CDS-{:?}",
            base_date + time::Duration::days(365 * 5)
        )),
        entity: "ACME".to_string(),
        pillar: Pillar::Date(base_date + time::Duration::days(365 * 5)),
        spread_bp: 120.0,
        recovery_rate: 0.40,
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        },
    });
    let _ = roundtrip_json(&cq);

    let vq = MarketQuote::Vol(VolQuote::SwaptionVol {
        id: QuoteId::new("USD-SWPTN-VOL-1Yx5Y-0.04"),
        expiry: base_date + time::Duration::days(365),
        maturity: base_date + time::Duration::days(365 * 5),
        strike: 0.04,
        vol: 0.01,
        quote_type: "ATM".to_string(),
        convention: SwaptionConventionId::new("USD"),
    });
    let _ = roundtrip_json(&vq);
}

#[test]
fn envelope_unknown_field_is_rejected() {
    let payload = r#"{
        "schema": "finstack.calibration/2",
        "plan": {
            "id": "p",
            "quote_sets": {},
            "steps": [],
            "settings": {},
            "oops": true
        }
    }"#;
    serde_json::from_str::<CalibrationEnvelope>(payload)
        .expect_err("unknown field should be rejected");
}

#[test]
fn test_hull_white_step_params_serde() {
    let json = r#"{"kind":"hull_white","id":"hw","quote_set":"swaptions","curve_id":"USD-OIS","currency":"USD","base_date":"2024-01-02"}"#;
    let step: CalibrationStep = serde_json::from_str(json).expect("should deserialize");
    assert!(matches!(step.params, StepParams::HullWhite(_)));

    // Also test roundtrip through Rust struct construction
    let base_date = Date::from_calendar_date(2024, Month::January, 2).unwrap();
    let hw = StepParams::HullWhite(HullWhiteStepParams {
        curve_id: "USD-OIS".into(),
        currency: Currency::USD,
        base_date,
        initial_kappa: Some(0.05),
        initial_sigma: None,
    });
    let _ = roundtrip_json(&hw);
}

#[test]
fn test_svi_surface_step_params_serde() {
    let json = r#"{"kind":"svi_surface","id":"svi","quote_set":"vols","surface_id":"EQ_SPX","base_date":"2024-01-02","underlying_ticker":"SPX"}"#;
    let step: CalibrationStep = serde_json::from_str(json).expect("should deserialize");
    assert!(matches!(step.params, StepParams::SviSurface(_)));

    // Also test roundtrip through Rust struct construction
    let base_date = Date::from_calendar_date(2024, Month::January, 2).unwrap();
    let svi = StepParams::SviSurface(SviSurfaceParams {
        surface_id: "EQ_SPX".to_string(),
        base_date,
        underlying_ticker: "SPX".to_string(),
        discount_curve_id: Some("USD-OIS".into()),
        target_expiries: vec![0.5, 1.0],
        target_strikes: vec![90.0, 100.0, 110.0],
        spot_override: Some(100.0),
    });
    let _ = roundtrip_json(&svi);
}
