//! Determinism and smoke tests for calibration v2.

use crate::finstack_test_utils::calibration as cal_utils;
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, Tenor};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::types::IndexId;
use finstack_valuations::calibration::api::engine;
use finstack_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, DiscountCurveParams, StepParams,
};
use finstack_valuations::calibration::{CalibrationConfig, CalibrationMethod};
use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_valuations::market::quotes::market_quote::MarketQuote;
use finstack_valuations::market::quotes::rates::RateQuote;
use time::Month;

fn build_discount_quotes(_base_date: Date) -> Vec<MarketQuote> {
    vec![
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("DEP-1M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("1M").unwrap()),
            rate: 0.05,
        }),
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("DEP-6M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("6M").unwrap()),
            rate: 0.052,
        }),
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("DEP-1Y"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("1Y").unwrap()),
            rate: 0.053,
        }),
    ]
}

fn build_credit_quotes() -> Vec<MarketQuote> {
    use finstack_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
    use finstack_valuations::market::quotes::cds::CdsQuote;
    vec![
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-1"),
            entity: "TEST-ENTITY".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2026, Month::March, 20).unwrap()),
            spread_bp: 100.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-2"),
            entity: "TEST-ENTITY".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2028, Month::March, 20).unwrap()),
            spread_bp: 150.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
        MarketQuote::Cds(CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-3"),
            entity: "TEST-ENTITY".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2030, Month::March, 20).unwrap()),
            spread_bp: 200.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
    ]
}

fn create_test_discount_curve(base_date: Date) -> DiscountCurve {
    use finstack_core::dates::DayCount;
    use finstack_core::math::interp::InterpStyle;
    DiscountCurve::builder("TEST-DISC")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots(vec![
            (0.0, 1.0),
            (1.0, 0.98),
            (3.0, 0.94),
            (5.0, 0.88),
            (10.0, 0.75),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .expect("discount curve")
}

fn run_discount_plan(base_date: Date, quotes: Vec<MarketQuote>) -> DiscountCurve {
    let currency = Currency::USD;
    let mut market_data = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: finstack_core::HashMap<String, Vec<QuoteId>> =
        finstack_core::HashMap::default();
    quote_sets.insert("disc".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![CalibrationStep {
            id: "disc".to_string(),
            quote_set: "disc".to_string(),
            params: StepParams::Discount(DiscountCurveParams {
                curve_id: "USD-OIS".into(),
                currency,
                base_date,
                method: CalibrationMethod::Bootstrap,
                interpolation: Default::default(),
                extrapolation: finstack_core::math::interp::ExtrapolationPolicy::FlatForward,
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

    let result = engine::execute(&envelope).expect("calibration should succeed");
    let ctx = MarketContext::try_from(result.result.final_market).expect("restore market context");
    ctx.get_discount("USD-OIS")
        .expect("discount curve")
        .as_ref()
        .clone()
}

fn run_hazard_plan(
    base_date: Date,
    quotes: Vec<MarketQuote>,
) -> finstack_core::market_data::term_structures::HazardCurve {
    use finstack_valuations::calibration::api::schema::HazardCurveParams;
    let currency = Currency::USD;
    let initial_market = MarketContext::new().insert(create_test_discount_curve(base_date));
    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: finstack_core::HashMap<String, Vec<QuoteId>> =
        finstack_core::HashMap::default();
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
                curve_id: "TEST-ENTITY-SENIOR".into(),
                entity: "TEST-ENTITY".to_string(),
                seniority: finstack_core::market_data::term_structures::Seniority::Senior,
                currency,
                base_date,
                discount_curve_id: "TEST-DISC".into(),
                recovery_rate: 0.40,
                notional: 1.0,
                method: CalibrationMethod::Bootstrap,
                interpolation: Default::default(),
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

    let result = engine::execute(&envelope).expect("calibration should succeed");
    let ctx = MarketContext::try_from(result.result.final_market).expect("restore market context");
    ctx.get_hazard("TEST-ENTITY-SENIOR")
        .expect("hazard curve")
        .as_ref()
        .clone()
}

#[ignore = "slow: covered by mise rust-test-slow"]
#[test]
fn hazard_curve_calibration_is_deterministic_across_runs() {
    let base_date = Date::from_calendar_date(2025, Month::March, 20).unwrap();

    let baseline_curve = run_hazard_plan(base_date, build_credit_quotes());
    let baseline_knots: Vec<(f64, f64)> = baseline_curve.knot_points().collect();
    assert!(!baseline_knots.is_empty(), "baseline should have knots");

    // Subsequent runs must match exactly (engine determinism).
    for i in 0..20 {
        let curve = run_hazard_plan(base_date, build_credit_quotes());
        let knots: Vec<(f64, f64)> = curve.knot_points().collect();
        assert_eq!(
            knots, baseline_knots,
            "run {} produced different knots than baseline",
            i
        );
    }
}

#[test]
fn discount_curve_bootstrap_is_order_independent() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    let quotes_sorted = build_discount_quotes(base_date);
    let mut quotes_shuffled = quotes_sorted.clone();
    quotes_shuffled.reverse();

    let curve_sorted = run_discount_plan(base_date, quotes_sorted);
    let curve_shuffled = run_discount_plan(base_date, quotes_shuffled);

    assert_eq!(
        curve_sorted.knots(),
        curve_shuffled.knots(),
        "discount curve knots should be order-independent"
    );
    assert_eq!(
        curve_sorted.dfs(),
        curve_shuffled.dfs(),
        "discount curve DFs should be order-independent"
    );
}

#[test]
fn discount_curve_global_solve_smoke() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let currency = Currency::USD;

    let quotes = vec![
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("DEP-6M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("6M").unwrap()),
            rate: 0.03,
        }),
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("DEP-1Y"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("1Y").unwrap()),
            rate: 0.031,
        }),
        MarketQuote::Rates(RateQuote::Deposit {
            id: QuoteId::new("DEP-18M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("18M").unwrap()),
            rate: 0.0315,
        }),
    ];

    let mut market_data = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: finstack_core::HashMap<String, Vec<QuoteId>> =
        finstack_core::HashMap::default();
    quote_sets.insert("disc".to_string(), cal_utils::quote_set_ids(&quotes));

    let settings = CalibrationConfig {
        calibration_method: CalibrationMethod::GlobalSolve {
            use_analytical_jacobian: false,
        },
        ..Default::default()
    };

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: settings.clone(),
        steps: vec![CalibrationStep {
            id: "disc".to_string(),
            quote_set: "disc".to_string(),
            params: StepParams::Discount(DiscountCurveParams {
                curve_id: "USD-OIS".into(),
                currency,
                base_date,
                method: CalibrationMethod::GlobalSolve {
                    use_analytical_jacobian: false,
                },
                interpolation: Default::default(),
                extrapolation: finstack_core::math::interp::ExtrapolationPolicy::FlatForward,
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

    let result = engine::execute(&envelope).expect("global solve should succeed");
    let step = result.result.step_reports.get("disc").expect("step report");
    assert!(step.success, "global solve should report success");
    assert!(
        step.max_residual <= settings.discount_curve.validation_tolerance,
        "max_residual {} exceeds tolerance {}",
        step.max_residual,
        settings.discount_curve.validation_tolerance
    );

    let ctx = MarketContext::try_from(result.result.final_market).expect("restore context");
    let curve = ctx.get_discount("USD-OIS").expect("discount curve");
    assert!(
        curve.dfs().windows(2).all(|w| w[1] < w[0]),
        "discount factors should decay"
    );
}
