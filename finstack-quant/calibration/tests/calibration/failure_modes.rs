//! Failure mode coverage for plan-driven calibration preflight checks.

use crate::calibration_support as cal_utils;
use crate::common::fixtures;
use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::{
    BaseCorrelationParams, CalibrationEnvelope, CalibrationPlan, CalibrationStep,
    ForwardCurveParams, HazardCurveParams, InflationCurveParams, SabrInterpolationMethod,
    StepParams, SurfaceExtrapolationPolicy, SwaptionVolConvention, SwaptionVolParams,
    VolSurfaceModel, VolSurfaceParams,
};
use finstack_quant_calibration::quotes::cds::CdsQuote;
use finstack_quant_calibration::quotes::cds_tranche::CdsTrancheQuote;
use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
use finstack_quant_calibration::quotes::inflation::InflationQuote;
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::{MarketContext, MarketContextState};
use finstack_quant_core::market_data::scalars::{
    InflationIndex, InflationInterpolation, InflationLag,
};
use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::{HazardCurve, ParInterp};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
use std::sync::Arc;
use time::Month;

/// Reuse shared base_date from fixtures module.
fn base_date() -> Date {
    fixtures::base_date()
}

/// Reuse shared minimal USD discount curve from fixtures module.
fn usd_discount_curve(base_date: Date) -> DiscountCurve {
    fixtures::usd_discount_curve_minimal(base_date, "USD-OIS")
}

#[test]
fn market_context_split_rejects_malformed_collateral_currency() {
    let mut state = MarketContextState::from(&MarketContext::new());
    state
        .collateral
        .insert("NOT_A_CURRENCY".to_string(), "USD".to_string());

    let err =
        cal_utils::split_market_context_state(state).expect_err("invalid currency should error");
    assert!(err.to_string().contains("Invalid collateral currency"));
    assert!(err.to_string().contains("NOT_A_CURRENCY"));
}

fn envelope_for_step(
    step: CalibrationStep,
    quotes: Vec<MarketQuote>,
    source_market: MarketContext,
) -> CalibrationEnvelope {
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert(step.quote_set.clone(), cal_utils::quote_set_ids(&quotes));
    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![step],
    };

    CalibrationEnvelope {
        schema_url: None,

        schema: finstack_quant_calibration::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    }
}

#[test]
fn hazard_preflight_rejects_entity_mismatch() {
    let base_date = base_date();
    let discount = usd_discount_curve(base_date);
    let source_market = MarketContext::new().insert(discount);

    let quote = MarketQuote::Cds(CdsQuote::CdsParSpread {
        id: QuoteId::new("CDS-ACME-5Y"),
        entity: "ACME".to_string(),
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
        pillar: Pillar::Tenor("5Y".parse().expect("tenor")),
        spread_bp: 120.0,
        recovery_rate: 0.40,
    });

    let step = CalibrationStep {
        id: "hazard".to_string(),
        quote_set: "cds".to_string(),
        params: StepParams::Hazard(HazardCurveParams {
            curve_id: "ACME-CDS".into(),
            entity: "BETA".to_string(),
            seniority: finstack_quant_core::market_data::term_structures::Seniority::Senior,
            currency: Currency::USD,
            base_date,
            discount_curve_id: "USD-OIS".into(),
            recovery_rate: 0.40,
            notional: 1.0,
            method: Default::default(),
            interpolation: InterpStyle::Linear,
            par_interp: ParInterp::Linear,
            doc_clause: None,
            cds_valuation_convention: None,
        }),
    };

    let envelope = envelope_for_step(step, vec![quote], source_market);
    let err = engine::execute(&envelope).expect_err("entity mismatch should fail");
    let msg = err.to_string();
    assert!(msg.contains("entity mismatch"), "unexpected error: {msg}");
}

#[test]
fn inflation_preflight_rejects_invalid_observation_lag() {
    let base_date = base_date();
    let discount = usd_discount_curve(base_date);
    let source_market = MarketContext::new().insert(discount);

    let quote = MarketQuote::Inflation(InflationQuote::InflationSwap {
        id: QuoteId::new("USA-CPI-U-ZCIS-20300102"),
        maturity: Date::from_calendar_date(2030, Month::January, 2).expect("maturity"),
        rate: 0.02,
        index: "USA-CPI-U".to_string(),
        convention:
            finstack_quant_valuations::market::conventions::ids::InflationSwapConventionId::new(
                "USD",
            ),
    });

    let step = CalibrationStep {
        id: "infl".to_string(),
        quote_set: "infl".to_string(),
        params: StepParams::Inflation(InflationCurveParams {
            curve_id: "USD-CPI".into(),
            currency: Currency::USD,
            base_date,
            discount_curve_id: "USD-OIS".into(),
            index: "USA-CPI-U".to_string(),
            observation_lag: "3Q".to_string(),
            base_cpi: 100.0,
            notional: 1.0,
            method: Default::default(),
            interpolation: Default::default(),
            seasonal_factors: None,
        }),
    };

    let envelope = envelope_for_step(step, vec![quote], source_market);
    let err = engine::execute(&envelope).expect_err("invalid lag should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("Invalid observation_lag"),
        "unexpected error: {msg}"
    );
}

#[test]
fn vol_surface_deserialization_rejects_unknown_model() {
    let base_date = base_date();

    let step = CalibrationStep {
        id: "vol".to_string(),
        quote_set: "vols".to_string(),
        params: StepParams::VolSurface(VolSurfaceParams {
            vol_surface_id: "USD-SWAPTION-SABR".to_string(),
            base_date,
            underlying_ticker: "USD-SWAPTION".to_string(),
            model: VolSurfaceModel::Sabr,
            discount_curve_id: Some("USD-OIS".into()),
            beta: 0.5,
            target_expiries: Vec::new(),
            target_strikes: Vec::new(),
            spot_override: None,
            dividend_yield_override: None,
            expiry_extrapolation: SurfaceExtrapolationPolicy::Error,
        }),
    };

    let mut encoded = serde_json::to_value(step).expect("calibration step must serialize");
    encoded["model"] = serde_json::json!("heston");
    let err = serde_json::from_value::<CalibrationStep>(encoded)
        .expect_err("unknown model must fail at the serde boundary");
    let msg = err.to_string();
    assert!(msg.contains("unknown variant"), "unexpected error: {msg}");
}

#[test]
fn swaption_vol_preflight_rejects_invalid_shift() {
    let base_date = base_date();
    let discount = usd_discount_curve(base_date);
    let source_market = MarketContext::new().insert(discount);

    let step = CalibrationStep {
        id: "swaption".to_string(),
        quote_set: "swaption_quotes".to_string(),
        params: StepParams::SwaptionVol(SwaptionVolParams {
            vol_surface_id: "USD-SWAPTION-VOL".to_string(),
            base_date,
            discount_curve_id: "USD-OIS".into(),
            forward_id: None,
            currency: Currency::USD,
            vol_convention: SwaptionVolConvention::ShiftedLognormal { shift: 0.0 },
            sabr_beta: 0.5,
            target_expiries: Vec::new(),
            target_tenors: Vec::new(),
            sabr_interpolation: SabrInterpolationMethod::default(),
            calendar_id: None,
            fixed_day_count: None,
            swap_index: None,
            vol_tolerance: None,
            sabr_extrapolation: SurfaceExtrapolationPolicy::Error,
            allow_sabr_missing_bucket_fallback: false,
        }),
    };

    let envelope = envelope_for_step(step, Vec::new(), source_market);
    let err = engine::execute(&envelope).expect_err("invalid shift should fail");
    let msg = err.to_string();
    assert!(msg.contains("Shifted lognormal"), "unexpected error: {msg}");
}

#[test]
fn base_correlation_preflight_rejects_invalid_attachment_detachment() {
    let base_date = base_date();
    let hazard = Arc::new(
        HazardCurve::builder("CDX-HAZARD")
            .base_date(base_date)
            .knots(vec![(1.0, 0.01), (5.0, 0.02)])
            .recovery_rate(0.40)
            .build()
            .expect("hazard curve"),
    );
    let base_corr = Arc::new(
        BaseCorrelationCurve::builder("CDX-CORR")
            .knots(vec![(3.0, 0.25), (10.0, 0.55)])
            .build()
            .expect("base correlation curve"),
    );
    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(std::sync::Arc::clone(&hazard))
        .base_correlation_curve(std::sync::Arc::clone(&base_corr))
        .build()
        .expect("credit index data");

    let source_market = MarketContext::new()
        .insert(hazard.as_ref().clone())
        .insert(base_corr.as_ref().clone())
        .insert_credit_index("CDX.NA.IG", index_data);

    let tranche_quote = MarketQuote::CdsTranche(CdsTrancheQuote {
        id: QuoteId::new("CDX-IG-7-3"),
        index: "CDX.NA.IG".to_string(),
        series: 1,
        attachment: 0.07,
        detachment: 0.03,
        maturity: Date::from_calendar_date(2030, Month::June, 20).expect("maturity"),
        upfront_pct: -0.02,
        running_spread_bp: 500.0,
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
    });

    let step = CalibrationStep {
        id: "corr".to_string(),
        quote_set: "tranche".to_string(),
        params: StepParams::BaseCorrelation(BaseCorrelationParams {
            index_id: "CDX.NA.IG".to_string(),
            series: 41,
            maturity_years: 5.0,
            base_date,
            discount_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            notional: 1.0,
            frequency: None,
            day_count: None,
            business_day_convention: None,
            calendar_id: None,
            detachment_points: Vec::new(),
            use_imm_dates: false,
        }),
    };

    let envelope = envelope_for_step(step, vec![tranche_quote], source_market);
    let err = engine::execute(&envelope).expect_err("invalid tranche should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("attachment must be less than detachment"),
        "unexpected error: {msg}"
    );
}

#[test]
fn base_correlation_preflight_requires_credit_index_data() {
    let base_date = base_date();
    let discount = usd_discount_curve(base_date);
    let source_market = MarketContext::new().insert(discount);

    let tranche_quote = MarketQuote::CdsTranche(CdsTrancheQuote {
        id: QuoteId::new("CDX-IG-0-3"),
        index: "CDX.NA.IG".to_string(),
        series: 1,
        attachment: 0.0,
        detachment: 0.03,
        maturity: Date::from_calendar_date(2030, Month::June, 20).expect("maturity"),
        upfront_pct: -0.02,
        running_spread_bp: 500.0,
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
    });

    let step = CalibrationStep {
        id: "corr".to_string(),
        quote_set: "tranche".to_string(),
        params: StepParams::BaseCorrelation(BaseCorrelationParams {
            index_id: "CDX.NA.IG".to_string(),
            series: 41,
            maturity_years: 5.0,
            base_date,
            discount_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            notional: 1.0,
            frequency: None,
            day_count: None,
            business_day_convention: None,
            calendar_id: None,
            detachment_points: vec![0.03],
            use_imm_dates: false,
        }),
    };

    let envelope = envelope_for_step(step, vec![tranche_quote], source_market);
    let err = engine::execute(&envelope).expect_err("missing credit index should fail");
    let msg = err.to_string();
    assert!(
        msg.to_ascii_lowercase().contains("credit index")
            || msg.to_ascii_lowercase().contains("not found"),
        "unexpected error: {msg}"
    );
}

#[test]
fn base_correlation_preflight_rejects_non_monotone_tranche_points() {
    let base_date = base_date();
    let hazard = Arc::new(
        HazardCurve::builder("CDX-HAZARD")
            .base_date(base_date)
            .knots(vec![(1.0, 0.01), (5.0, 0.02)])
            .recovery_rate(0.40)
            .build()
            .expect("hazard curve"),
    );
    let base_corr = Arc::new(
        BaseCorrelationCurve::builder("CDX-CORR")
            .knots(vec![(3.0, 0.25), (10.0, 0.55)])
            .build()
            .expect("base correlation curve"),
    );
    let hazard_clone = hazard.as_ref().clone();
    let base_corr_clone = base_corr.as_ref().clone();
    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect("credit index data");

    let source_market = MarketContext::new()
        .insert(hazard_clone)
        .insert(base_corr_clone)
        .insert_credit_index("CDX.NA.IG", index_data);

    let tranche_quote = MarketQuote::CdsTranche(CdsTrancheQuote {
        id: QuoteId::new("CDX-IG-7-3"),
        index: "CDX.NA.IG".to_string(),
        series: 1,
        attachment: 0.15,
        detachment: 0.10, // invalid: detachment < attachment
        maturity: Date::from_calendar_date(2030, Month::June, 20).expect("maturity"),
        upfront_pct: -0.02,
        running_spread_bp: 500.0,
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::Cr14,
        },
    });

    let step = CalibrationStep {
        id: "corr".to_string(),
        quote_set: "tranche".to_string(),
        params: StepParams::BaseCorrelation(BaseCorrelationParams {
            index_id: "CDX.NA.IG".to_string(),
            series: 41,
            maturity_years: 5.0,
            base_date,
            discount_curve_id: "USD-OIS".into(),
            currency: Currency::USD,
            notional: 1.0,
            frequency: None,
            day_count: None,
            business_day_convention: None,
            calendar_id: None,
            detachment_points: vec![0.03],
            use_imm_dates: false,
        }),
    };

    let envelope = envelope_for_step(step, vec![tranche_quote], source_market);
    let err = engine::execute(&envelope).expect_err("invalid tranche attachment should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("attachment must be less than detachment"),
        "unexpected error: {msg}"
    );
}

#[test]
fn inflation_preflight_rejects_lag_mismatch_with_index() {
    let base_date = base_date();
    let discount = usd_discount_curve(base_date);
    let observations = vec![
        (
            Date::from_calendar_date(2025, Month::January, 2).expect("obs date"),
            100.0,
        ),
        (
            Date::from_calendar_date(2025, Month::February, 2).expect("obs date"),
            101.0,
        ),
    ];
    let index = InflationIndex::new("USD-CPI", observations, Currency::USD)
        .expect("index")
        .with_interpolation(InflationInterpolation::Linear)
        .with_lag(InflationLag::Months(3));

    let source_market = MarketContext::new()
        .insert(discount)
        .insert_inflation_index("USD-CPI", index);

    let quote = MarketQuote::Inflation(InflationQuote::InflationSwap {
        id: QuoteId::new("USD-CPI-ZCIS-20300102"),
        maturity: Date::from_calendar_date(2030, Month::January, 2).expect("maturity"),
        rate: 0.02,
        index: "USD-CPI".to_string(),
        convention:
            finstack_quant_valuations::market::conventions::ids::InflationSwapConventionId::new(
                "USD",
            ),
    });

    let step = CalibrationStep {
        id: "infl".to_string(),
        quote_set: "infl".to_string(),
        params: StepParams::Inflation(InflationCurveParams {
            curve_id: "USD-CPI".into(),
            currency: Currency::USD,
            base_date,
            discount_curve_id: "USD-OIS".into(),
            index: "USD-CPI".to_string(),
            observation_lag: "1M".to_string(), // mismatch vs index lag (3M)
            base_cpi: 100.0,
            notional: 1.0,
            method: Default::default(),
            interpolation: Default::default(),
            seasonal_factors: None,
        }),
    };

    let envelope = envelope_for_step(step, vec![quote], source_market);
    let err = engine::execute(&envelope).expect_err("lag mismatch should fail");
    let msg = err.to_string();
    assert!(msg.contains("lag mismatch"), "unexpected error: {msg}");
}

#[test]
fn forward_preflight_requires_quotes() {
    let base_date = base_date();
    let discount = usd_discount_curve(base_date);
    let source_market = MarketContext::new().insert(discount);

    let step = CalibrationStep {
        id: "fwd".to_string(),
        quote_set: "fwd_quotes".to_string(),
        params: StepParams::Forward(ForwardCurveParams {
            curve_id: "USD-FWD".into(),
            currency: Currency::USD,
            base_date,
            tenor_years: 5.0,
            discount_curve_id: "USD-OIS".into(),
            method: Default::default(),
            interpolation: Default::default(),
            conventions: Default::default(),
        }),
    };

    let envelope = envelope_for_step(step, Vec::new(), source_market);
    let err = engine::execute(&envelope).expect_err("missing forward quotes should fail");
    let msg = err.to_string().to_ascii_lowercase();
    assert!(
        msg.contains("too few points") || msg.contains("at least two"),
        "unexpected error: {msg}"
    );
}

#[test]
fn vol_surface_requires_quotes_even_when_params_valid() {
    let base_date = base_date();
    let discount = usd_discount_curve(base_date);
    let source_market = MarketContext::new().insert(discount);

    let step = CalibrationStep {
        id: "vol".to_string(),
        quote_set: "vols".to_string(),
        params: StepParams::VolSurface(VolSurfaceParams {
            vol_surface_id: "EQ-VOL".to_string(),
            base_date,
            underlying_ticker: "SPX".to_string(),
            model: VolSurfaceModel::Sabr,
            discount_curve_id: Some("USD-OIS".into()),
            beta: 0.5,
            target_expiries: vec![1.0], // year fraction (validated by VolSurfaceTarget)
            target_strikes: vec![0.9, 1.0, 1.1],
            spot_override: Some(100.0),
            dividend_yield_override: None,
            expiry_extrapolation: SurfaceExtrapolationPolicy::Error,
        }),
    };

    let envelope = envelope_for_step(step, Vec::new(), source_market);
    let err = engine::execute(&envelope).expect_err("missing vol quotes should fail");
    let msg = err.to_string().to_ascii_lowercase();
    assert!(
        msg.contains("too few points") || msg.contains("at least two"),
        "unexpected error: {msg}"
    );
}
