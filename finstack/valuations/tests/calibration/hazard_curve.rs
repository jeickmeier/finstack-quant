//! Hazard curve calibration tests (v2).

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::Seniority;
use finstack_core::math::interp::InterpStyle;
use finstack_core::types::CurveId;
use finstack_core::HashMap;
use finstack_valuations::calibration::api::engine;
use finstack_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, HazardCurveParams, StepParams,
};
use finstack_valuations::calibration::{
    CalibrationConfig, CalibrationMethod, ResidualWeightingScheme,
};
use finstack_valuations::market::build_cds_instrument;
use finstack_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
use finstack_valuations::market::quotes::cds::CdsQuote;
use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_valuations::market::quotes::market_quote::MarketQuote;
use finstack_valuations::market::BuildCtx;
use time::Month;

use crate::common::fixtures;
use crate::finstack_test_utils::calibration as cal_utils;

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

fn hazard_total_variation(curve: &finstack_core::market_data::term_structures::HazardCurve) -> f64 {
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
fn hazard_calibration_positive_rates() {
    // Use ISDA-friendly dates (IMM 20th) because v2 hazard bootstrapping builds
    // canonical CDS instruments under ISDA conventions.
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc = create_test_discount_curve(base);
    let initial_market = MarketContext::new().insert(disc);

    let quotes = vec![
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
            spread_bp: 150.0,
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
            spread_bp: 200.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
    ];

    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let hazard_id: CurveId = "ACME-Corp-SENIOR".into();

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
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
                interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::execute(&envelope).expect("execute");
    assert!(result.result.report.success);

    let ctx = MarketContext::try_from(result.result.final_market).expect("restore context");
    let curve = ctx.get_hazard(hazard_id.as_str()).expect("hazard curve");

    for (_t, lambda) in curve.knot_points() {
        assert!(lambda > 0.0, "hazard rate should be positive, got {lambda}");
    }
}

#[test]
fn hazard_calibration_rejects_zero_spread() {
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc = create_test_discount_curve(base);
    let initial_market = MarketContext::new().insert(disc);

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

    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
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
                interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: prior,
    };

    let err = engine::execute(&envelope).expect_err("zero spread should be invalid");
    assert!(matches!(
        err,
        finstack_core::Error::Validation(_)
            | finstack_core::Error::Input(_)
            | finstack_core::Error::Calibration { .. }
    ));
}

#[test]
fn hazard_calibration_rejects_negative_spread() {
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc = create_test_discount_curve(base);
    let initial_market = MarketContext::new().insert(disc);

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

    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
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
                interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: prior,
    };

    let err = engine::execute(&envelope).expect_err("negative spread should be invalid");
    assert!(matches!(
        err,
        finstack_core::Error::Validation(_)
            | finstack_core::Error::Input(_)
            | finstack_core::Error::Calibration { .. }
    ));
}

#[test]
fn hazard_calibration_rejects_non_standard_upfront_running_coupon() {
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc = create_test_discount_curve(base);
    let initial_market = MarketContext::new().insert(disc);

    let quotes = vec![MarketQuote::Cds(CdsQuote::CdsUpfront {
        id: QuoteId::new("CDS-UPFRONT-250BP"),
        entity: "NONSTANDARD-UPFRONT".to_string(),
        pillar: Pillar::Date(Date::from_calendar_date(2028, Month::March, 20).unwrap()),
        running_spread_bp: 250.0,
        upfront_pct: 0.02,
        recovery_rate: 0.40,
        convention: CdsConventionKey {
            currency,
            doc_clause: CdsDocClause::IsdaNa,
        },
    })];

    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
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
                interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: prior,
    };

    let err = engine::execute(&envelope)
        .expect_err("non-standard upfront running coupon should be invalid");
    assert!(matches!(
        err,
        finstack_core::Error::Validation(_)
            | finstack_core::Error::Input(_)
            | finstack_core::Error::Calibration { .. }
    ));
}

#[test]
fn hazard_calibration_handles_extreme_high_spread() {
    // Test that very high spreads (>1000bp) are handled correctly.
    // High spreads are valid for distressed credits (e.g., CCC-rated).
    let base = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc = create_test_discount_curve(base);
    let initial_market = MarketContext::new().insert(disc);

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

    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let hazard_id: CurveId = "DISTRESSED-CORP-SENIOR".into();

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
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
                interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::execute(&envelope).expect("high spread calibration should succeed");
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
    let initial_market = MarketContext::new().insert(disc);

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

    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("credit".to_string(), cal_utils::quote_set_ids(&quotes));

    let hazard_id_boot: CurveId = "ACME-Corp-BOOT".into();
    let hazard_id_global: CurveId = "ACME-Corp-GLOBAL".into();

    let bootstrap_plan = CalibrationPlan {
        id: "plan-bootstrap".to_string(),
        description: None,
        quote_sets: quote_sets.clone(),
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
                interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let bootstrap_env = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan: bootstrap_plan,
        market_data: market_data.clone(),
        prior_market: prior.clone(),
    };

    let bootstrap_result = engine::execute(&bootstrap_env).expect("bootstrap execute");
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
        quote_sets,
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
                interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let global_env = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan: global_plan,
        market_data,
        prior_market: prior,
    };

    let global_result = engine::execute(&global_env).expect("global execute");
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
    let initial_market = MarketContext::new().insert(disc);

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
    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
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
        quote_sets,
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
                interpolation: finstack_core::math::interp::InterpStyle::LogLinear,
                par_interp: finstack_core::market_data::term_structures::ParInterp::Linear,
                doc_clause: None,
                cds_valuation_convention: None,
            }),
        }],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: prior,
    };

    let result = engine::execute(&envelope).expect("execute");
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
