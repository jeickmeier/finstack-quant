//! Hazard curve calibration tests (canonical).

use crate::api::engine;
use crate::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, HazardCurveParams, StepParams,
};
use crate::build::cds::build_cds_instrument;
use crate::build::BuildCtx;
use crate::quotes::cds::CdsQuote;
use crate::quotes::ids::{Pillar, QuoteId};
use crate::quotes::market_quote::MarketQuote;
use crate::recalibration::bump_hazard_spreads;
use crate::{CalibrationConfig, CalibrationMethod, ResidualWeightingScheme};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::Seniority;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
use finstack_quant_valuations::recalibration::QuoteBump;
use time::Month;

use crate::test_support as cal_utils;
use crate::test_support as fixtures;

fn create_test_discount_curve(base: Date) -> DiscountCurve {
    DiscountCurve::builder("TEST-DISC")
        .base_date(base)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.98),
            (3.0, 0.94),
            (5.0, 0.88),
            (10.0, 0.75),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

fn hazard_total_variation(
    curve: &finstack_quant_core::market_data::term_structures::HazardCurve,
) -> f64 {
    let mut total = 0.0;
    let mut prev: Option<f64> = None;
    for (_t, lambda) in curve.knot_points() {
        if let Some(last) = prev {
            total += (lambda - last).abs();
        }
        prev = Some(lambda);
    }
    total
}

#[test]
fn hazard_recipe_act365f_inputs_replay_round_trip_and_reject_tampering() {
    // Use ISDA-friendly dates (IMM 20th) because canonical hazard bootstrapping builds
    // canonical CDS instruments under ISDA conventions.
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::JPY;

    let disc = create_test_discount_curve(base);
    let source_market = MarketContext::new().insert(disc);

    let mut quotes = vec![
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!(
                "CDS-{:?}",
                Date::from_calendar_date(2026, Month::March, 20).unwrap()
            )),
            entity: "ACME-Corp".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2026, Month::March, 20).unwrap()),
            spread_bp: 100.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaAs,
            },
        }),
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!(
                "CDS-{:?}",
                Date::from_calendar_date(2028, Month::March, 20).unwrap()
            )),
            entity: "ACME-Corp".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2028, Month::March, 20).unwrap()),
            spread_bp: 150.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaAs,
            },
        }),
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!(
                "CDS-{:?}",
                Date::from_calendar_date(2030, Month::March, 20).unwrap()
            )),
            entity: "ACME-Corp".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2030, Month::March, 20).unwrap()),
            spread_bp: 200.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaAs,
            },
        }),
    ];
    quotes.reverse();

    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let hazard_id: CurveId = "ACME-Corp-SENIOR".into();

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "haz".to_string(),
            quote_set: "credit".to_string(),
            params: StepParams::Hazard(HazardCurveParams {
                curve_id: hazard_id.clone(),
                entity: "ACME-Corp".to_string(),
                seniority: Seniority::Senior,
                currency,
                base_date: base,
                discount_curve_id: "TEST-DISC".into(),
                recovery_rate: 0.40,
                notional: 1.0,
                method: CalibrationMethod::Bootstrap,
                interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: Some("isda_as".to_string()),
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: crate::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::calibrate(&envelope).expect("execute");
    assert!(result.result.report.success);

    let ctx = MarketContext::try_from(result.result.final_market).expect("restore context");
    let curve = ctx.get_hazard(hazard_id.as_str()).expect("hazard curve");

    for (_t, lambda) in curve.knot_points() {
        assert!(lambda > 0.0, "hazard rate should be positive, got {lambda}");
    }

    let recipe = curve
        .hazard_calibration()
        .expect("calibrated hazard curve must retain replay inputs");
    assert!(
        recipe
            .calibration_inputs
            .windows(2)
            .all(|pair| pair[0].pillar_time < pair[1].pillar_time),
        "unsorted market inputs must persist in canonical pillar order"
    );
    let serialized_recipe = serde_json::to_string(recipe).expect("serialize recipe");
    let round_trip: finstack_quant_core::market_data::term_structures::HazardCalibrationRecipe =
        serde_json::from_str(&serialized_recipe).expect("deserialize recipe");
    assert!(
        round_trip
            .calibration_inputs
            .windows(2)
            .all(|pair| pair[0].pillar_time < pair[1].pillar_time),
        "canonical pillar ordering must survive serde"
    );
    assert_eq!(
        recipe.calibration_inputs[0].quote["pillar"]["date"],
        serde_json::json!("2026-03-20"),
        "absolute CDS pillars must survive calibration losslessly"
    );
    assert_eq!(
        recipe.calibration_inputs[0].pillar_date,
        Date::from_calendar_date(2026, Month::March, 20).expect("valid pillar date")
    );
    assert_eq!(
        recipe.calibration_inputs[0].pillar_time,
        recipe.spread_risk_inputs[0].pillar_time
    );

    let mut mismatched_recipe = recipe.clone();
    mismatched_recipe.spread_risk_inputs[0].pillar_time += 0.25;
    let mismatched_curve = curve
        .to_builder_with_id(curve.id().clone())
        .hazard_calibration(mismatched_recipe)
        .build()
        .expect("core accepts convention-dependent bindings");
    let mismatch_error = bump_hazard_spreads(
        &mismatched_curve,
        &ctx,
        &QuoteBump::ParallelBp(1.0),
        Some(&CurveId::new("TEST-DISC")),
        None,
        None,
    )
    .expect_err("valuations replay must reject mismatched pillar time");
    assert!(
        mismatch_error.to_string().contains("stored pillar"),
        "{mismatch_error}"
    );

    let mut tampered_recipe = recipe.clone();
    for input in [
        &mut tampered_recipe.calibration_inputs[0],
        &mut tampered_recipe.spread_risk_inputs[0],
    ] {
        input.quote["pillar"]["date"] = serde_json::json!("2027-03-20");
    }
    let tampered_curve = curve
        .to_builder_with_id(curve.id().clone())
        .hazard_calibration(tampered_recipe)
        .build()
        .expect("core accepts structurally coherent serialized quote");
    let tamper_error = bump_hazard_spreads(
        &tampered_curve,
        &ctx,
        &QuoteBump::ParallelBp(1.0),
        Some(&CurveId::new("TEST-DISC")),
        None,
        None,
    )
    .expect_err("valuations replay must reject altered quote pillar");
    assert!(
        tamper_error.to_string().contains("serialized quote")
            && tamper_error.to_string().contains("stored pillar"),
        "{tamper_error}"
    );

    let mut upfront_recipe = recipe.clone();
    let par_quote: CdsQuote =
        serde_json::from_value(upfront_recipe.spread_risk_inputs[0].quote.clone())
            .expect("stored par quote");
    let CdsQuote::CdsParSpread {
        id,
        entity,
        pillar,
        recovery_rate,
        convention,
        ..
    } = par_quote
    else {
        panic!("spread-risk input must start as par spread");
    };
    upfront_recipe.spread_risk_inputs[0].quote = serde_json::to_value(CdsQuote::CdsUpfront {
        id,
        entity,
        pillar,
        coupon_bp: 100.0,
        upfront_pct: 2.0,
        recovery_rate,
        convention,
    })
    .expect("serialize malformed upfront risk quote");
    let upfront_error = curve
        .to_builder_with_id(curve.id().clone())
        .hazard_calibration(upfront_recipe)
        .build()
        .expect_err("curve construction must reject upfront spread-risk input");
    assert!(
        upfront_error.to_string().contains("par-spread"),
        "{upfront_error}"
    );

    let zero = bump_hazard_spreads(
        curve.as_ref(),
        &ctx,
        &QuoteBump::ParallelBp(0.0),
        Some(&CurveId::new("TEST-DISC")),
        None,
        None,
    )
    .expect("zero-shock replay");
    assert_eq!(
        zero.knot_points().collect::<Vec<_>>(),
        curve.knot_points().collect::<Vec<_>>(),
        "zero-shock replay must be identical"
    );

    let up = bump_hazard_spreads(
        curve.as_ref(),
        &ctx,
        &QuoteBump::ParallelBp(1.0),
        Some(&CurveId::new("TEST-DISC")),
        None,
        None,
    )
    .expect("up replay");
    let down = bump_hazard_spreads(
        curve.as_ref(),
        &ctx,
        &QuoteBump::ParallelBp(-1.0),
        Some(&CurveId::new("TEST-DISC")),
        None,
        None,
    )
    .expect("down replay");
    assert!(up.hazard_rate(3.0) > zero.hazard_rate(3.0));
    assert!(down.hazard_rate(3.0) < zero.hazard_rate(3.0));
}

#[test]
fn hazard_calibration_rejects_zero_spread() {
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc = create_test_discount_curve(base);
    let source_market = MarketContext::new().insert(disc);

    let quotes = vec![MarketQuote::Cds(CdsQuote::CdsParSpread {
        id: QuoteId::new(format!(
            "CDS-{:?}",
            Date::from_calendar_date(2026, Month::March, 20).unwrap()
        )),
        entity: "ZERO-SPREAD".to_string(),
        pillar: Pillar::Date(Date::from_calendar_date(2026, Month::March, 20).unwrap()),
        spread_bp: 0.0,
        recovery_rate: 0.40,
        convention: CdsConventionKey {
            currency,
            doc_clause: CdsDocClause::IsdaNa,
        },
    })];

    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "haz".to_string(),
            quote_set: "credit".to_string(),
            params: StepParams::Hazard(HazardCurveParams {
                curve_id: "ZERO-SPREAD-SENIOR".into(),
                entity: "ZERO-SPREAD".to_string(),
                seniority: Seniority::Senior,
                currency,
                base_date: base,
                discount_curve_id: "TEST-DISC".into(),
                recovery_rate: 0.40,
                notional: 1.0,
                method: CalibrationMethod::Bootstrap,
                interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: crate::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let err: finstack_quant_core::Error = engine::calibrate(&envelope)
        .expect_err("zero spread should be invalid")
        .into();
    assert!(matches!(
        err,
        finstack_quant_core::Error::Validation(_)
            | finstack_quant_core::Error::Input(_)
            | finstack_quant_core::Error::Calibration { .. }
    ));
}

#[test]
fn hazard_calibration_rejects_negative_spread() {
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc = create_test_discount_curve(base);
    let source_market = MarketContext::new().insert(disc);

    let quotes = vec![MarketQuote::Cds(CdsQuote::CdsParSpread {
        id: QuoteId::new(format!(
            "CDS-{:?}",
            Date::from_calendar_date(2026, Month::March, 20).unwrap()
        )),
        entity: "NEGATIVE-SPREAD".to_string(),
        pillar: Pillar::Date(Date::from_calendar_date(2026, Month::March, 20).unwrap()),
        spread_bp: -50.0, // Negative spread is invalid
        recovery_rate: 0.40,
        convention: CdsConventionKey {
            currency,
            doc_clause: CdsDocClause::IsdaNa,
        },
    })];

    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "haz".to_string(),
            quote_set: "credit".to_string(),
            params: StepParams::Hazard(HazardCurveParams {
                curve_id: "NEGATIVE-SPREAD-SENIOR".into(),
                entity: "NEGATIVE-SPREAD".to_string(),
                seniority: Seniority::Senior,
                currency,
                base_date: base,
                discount_curve_id: "TEST-DISC".into(),
                recovery_rate: 0.40,
                notional: 1.0,
                method: CalibrationMethod::Bootstrap,
                interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: crate::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let err: finstack_quant_core::Error = engine::calibrate(&envelope)
        .expect_err("negative spread should be invalid")
        .into();
    assert!(matches!(
        err,
        finstack_quant_core::Error::Validation(_)
            | finstack_quant_core::Error::Input(_)
            | finstack_quant_core::Error::Calibration { .. }
    ));
}

#[test]
fn hazard_calibration_rejects_non_standard_upfront_running_coupon() {
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc = create_test_discount_curve(base);
    let source_market = MarketContext::new().insert(disc);

    let quotes = vec![MarketQuote::Cds(CdsQuote::CdsUpfront {
        id: QuoteId::new("CDS-UPFRONT-250BP"),
        entity: "NONSTANDARD-UPFRONT".to_string(),
        pillar: Pillar::Date(Date::from_calendar_date(2028, Month::March, 20).unwrap()),
        coupon_bp: 250.0,
        upfront_pct: 0.02,
        recovery_rate: 0.40,
        convention: CdsConventionKey {
            currency,
            doc_clause: CdsDocClause::IsdaNa,
        },
    })];

    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "haz".to_string(),
            quote_set: "credit".to_string(),
            params: StepParams::Hazard(HazardCurveParams {
                curve_id: "NONSTANDARD-UPFRONT-SENIOR".into(),
                entity: "NONSTANDARD-UPFRONT".to_string(),
                seniority: Seniority::Senior,
                currency,
                base_date: base,
                discount_curve_id: "TEST-DISC".into(),
                recovery_rate: 0.40,
                notional: 1.0,
                method: CalibrationMethod::Bootstrap,
                interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: crate::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let err: finstack_quant_core::Error = engine::calibrate(&envelope)
        .expect_err("non-standard upfront running coupon should be invalid")
        .into();
    assert!(matches!(
        err,
        finstack_quant_core::Error::Validation(_)
            | finstack_quant_core::Error::Input(_)
            | finstack_quant_core::Error::Calibration { .. }
    ));
}

#[test]
fn hazard_calibration_handles_extreme_high_spread() {
    // Test that very high spreads (>1000bp) are handled correctly.
    // High spreads are valid for distressed credits (e.g., CCC-rated).
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc = create_test_discount_curve(base);
    let source_market = MarketContext::new().insert(disc);

    let quotes = vec![
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!(
                "CDS-{:?}",
                Date::from_calendar_date(2026, Month::March, 20).unwrap()
            )),
            entity: "DISTRESSED-CORP".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2026, Month::March, 20).unwrap()),
            spread_bp: 1500.0, // 15% spread - distressed
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!(
                "CDS-{:?}",
                Date::from_calendar_date(2028, Month::March, 20).unwrap()
            )),
            entity: "DISTRESSED-CORP".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2028, Month::March, 20).unwrap()),
            spread_bp: 2000.0, // 20% spread - very distressed
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!(
                "CDS-{:?}",
                Date::from_calendar_date(2030, Month::March, 20).unwrap()
            )),
            entity: "DISTRESSED-CORP".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2030, Month::March, 20).unwrap()),
            spread_bp: 2500.0, // 25% spread - near-default
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
    ];

    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let hazard_id: CurveId = "DISTRESSED-CORP-SENIOR".into();

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "haz".to_string(),
            quote_set: "credit".to_string(),
            params: StepParams::Hazard(HazardCurveParams {
                curve_id: hazard_id.clone(),
                entity: "DISTRESSED-CORP".to_string(),
                seniority: Seniority::Senior,
                currency,
                base_date: base,
                discount_curve_id: "TEST-DISC".into(),
                recovery_rate: 0.40,
                notional: 1.0,
                method: CalibrationMethod::Bootstrap,
                interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: crate::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::calibrate(&envelope).expect("high spread calibration should succeed");
    assert!(result.result.report.success);

    let ctx = MarketContext::try_from(result.result.final_market).expect("restore context");
    let curve = ctx.get_hazard(hazard_id.as_str()).expect("hazard curve");

    // Verify hazard rates are high (consistent with distressed spreads)
    for (_t, lambda) in curve.knot_points() {
        assert!(
            lambda > 0.10,
            "hazard rate for distressed credit should be high, got {lambda}"
        );
    }
}

#[test]
fn hazard_calibration_global_solve_sqrt_time_is_not_rougher_than_bootstrap() {
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc = create_test_discount_curve(base);
    let source_market = MarketContext::new().insert(disc);

    let quotes = vec![
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!(
                "CDS-{:?}",
                Date::from_calendar_date(2026, Month::March, 20).unwrap()
            )),
            entity: "ACME-Corp".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2026, Month::March, 20).unwrap()),
            spread_bp: 110.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!(
                "CDS-{:?}",
                Date::from_calendar_date(2028, Month::March, 20).unwrap()
            )),
            entity: "ACME-Corp".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2028, Month::March, 20).unwrap()),
            spread_bp: 170.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!(
                "CDS-{:?}",
                Date::from_calendar_date(2030, Month::March, 20).unwrap()
            )),
            entity: "ACME-Corp".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2030, Month::March, 20).unwrap()),
            spread_bp: 210.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new(format!(
                "CDS-{:?}",
                Date::from_calendar_date(2032, Month::March, 20).unwrap()
            )),
            entity: "ACME-Corp".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2032, Month::March, 20).unwrap()),
            spread_bp: 190.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
    ];

    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let hazard_id_boot: CurveId = "ACME-Corp-BOOT".into();
    let hazard_id_global: CurveId = "ACME-Corp-GLOBAL".into();

    let bootstrap_plan = CalibrationPlan {
        id: "plan-bootstrap".to_string(),
        description: None,
        quote_sets: quote_sets.clone().into_iter().collect(),
        settings: CalibrationConfig::default(),
        steps: vec![CalibrationStep {
            id: "haz".to_string(),
            quote_set: "credit".to_string(),
            params: StepParams::Hazard(HazardCurveParams {
                curve_id: hazard_id_boot.clone(),
                entity: "ACME-Corp".to_string(),
                seniority: Seniority::Senior,
                currency,
                base_date: base,
                discount_curve_id: "TEST-DISC".into(),
                recovery_rate: 0.40,
                notional: 1.0,
                method: CalibrationMethod::Bootstrap,
                interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let bootstrap_env = CalibrationEnvelope {
        schema_url: None,

        schema: crate::api::schema::CalibrationSchema::CURRENT,
        plan: bootstrap_plan,
        market_data: market_data.clone(),
        prior_market: prior.clone(),
    };

    let bootstrap_result = engine::calibrate(&bootstrap_env).expect("bootstrap execute");
    let bootstrap_report = bootstrap_result
        .result
        .step_reports
        .get("haz")
        .expect("bootstrap report");
    assert!(bootstrap_report.success);

    let bootstrap_ctx =
        MarketContext::try_from(bootstrap_result.result.final_market).expect("restore context");
    let bootstrap_curve = bootstrap_ctx
        .get_hazard(hazard_id_boot.as_str())
        .expect("bootstrap curve");

    let mut global_settings = CalibrationConfig::default();
    global_settings.discount_curve.weighting_scheme = ResidualWeightingScheme::SqrtTime;
    global_settings.calibration_method = CalibrationMethod::GlobalSolve {
        use_analytical_jacobian: false,
    };

    let global_plan = CalibrationPlan {
        id: "plan-global".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: global_settings.clone(),
        steps: vec![CalibrationStep {
            id: "haz".to_string(),
            quote_set: "credit".to_string(),
            params: StepParams::Hazard(HazardCurveParams {
                curve_id: hazard_id_global.clone(),
                entity: "ACME-Corp".to_string(),
                seniority: Seniority::Senior,
                currency,
                base_date: base,
                discount_curve_id: "TEST-DISC".into(),
                recovery_rate: 0.40,
                notional: 1.0,
                method: CalibrationMethod::GlobalSolve {
                    use_analytical_jacobian: false,
                },
                interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let global_env = CalibrationEnvelope {
        schema_url: None,

        schema: crate::api::schema::CalibrationSchema::CURRENT,
        plan: global_plan,
        market_data,
        prior_market: prior,
    };

    let global_result = engine::calibrate(&global_env).expect("global execute");
    let global_report = global_result
        .result
        .step_reports
        .get("haz")
        .expect("global report");
    assert!(global_report.success);
    assert!(
        global_report.max_residual <= global_settings.discount_curve.validation_tolerance,
        "max_residual {} exceeds tolerance {}",
        global_report.max_residual,
        global_settings.discount_curve.validation_tolerance
    );

    let global_ctx =
        MarketContext::try_from(global_result.result.final_market).expect("restore context");
    let global_curve = global_ctx
        .get_hazard(hazard_id_global.as_str())
        .expect("global curve");

    let bootstrap_tv = hazard_total_variation(&bootstrap_curve);
    let global_tv = hazard_total_variation(&global_curve);

    assert!(
        global_tv <= bootstrap_tv + 1e-6,
        "expected global solve to be no rougher (global {:.6e}, bootstrap {:.6e})",
        global_tv,
        bootstrap_tv
    );
}

#[test]
fn hazard_calibration_reprices_par_spread() {
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;
    let recovery_rate = 0.40;
    let spread_bp = 120.0;
    let maturity = Date::from_calendar_date(2026, Month::March, 20).unwrap();

    let disc = create_test_discount_curve(base);
    let source_market = MarketContext::new().insert(disc);

    let cds_quote = CdsQuote::CdsParSpread {
        id: QuoteId::new("CDS-1Y"),
        entity: "APPROX-REF".to_string(),
        pillar: Pillar::Date(maturity),
        spread_bp,
        recovery_rate,
        convention: CdsConventionKey {
            currency,
            doc_clause: CdsDocClause::IsdaNa,
        },
    };

    let credit_quotes = vec![MarketQuote::Cds(cds_quote.clone())];
    let (prior, mut market_data) = cal_utils::split_market_context(&source_market);
    cal_utils::extend_market_data(&mut market_data, &credit_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert(
        "credit".to_string(),
        cal_utils::quote_set_ids(&credit_quotes),
    );

    let hazard_id: CurveId = "APPROX-REF-SENIOR".into();

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets: quote_sets.into_iter().collect(),
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "haz".to_string(),
            quote_set: "credit".to_string(),
            params: StepParams::Hazard(HazardCurveParams {
                curve_id: hazard_id.clone(),
                entity: "APPROX-REF".to_string(),
                seniority: Seniority::Senior,
                currency,
                base_date: base,
                discount_curve_id: "TEST-DISC".into(),
                recovery_rate,
                notional: 1.0,
                method: CalibrationMethod::Bootstrap,
                interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: crate::api::schema::CalibrationSchema::CURRENT,
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::calibrate(&envelope).expect("execute");
    let ctx = MarketContext::try_from(result.result.final_market).expect("restore context");

    let mut curve_ids = HashMap::default();
    curve_ids.insert("discount".to_string(), "TEST-DISC".to_string());
    curve_ids.insert("credit".to_string(), hazard_id.as_str().to_string());
    let build_ctx = BuildCtx::new(base, fixtures::STANDARD_NOTIONAL, curve_ids);

    let instrument = build_cds_instrument(&cds_quote, &build_ctx).expect("cds instrument build");

    let pv = instrument.value(&ctx, base).expect("cds valuation");
    let tolerance = 5.0;
    assert!(
        pv.amount().abs() <= tolerance,
        "CDS par spread repricing should be within ${}. PV=${:.6}",
        tolerance,
        pv.amount(),
    );
}
