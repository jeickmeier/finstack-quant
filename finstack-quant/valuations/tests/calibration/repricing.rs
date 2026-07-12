//! Calibration repricing tests (v2) with market-standard tolerance requirements.
//!
//! The goal is to ensure that curves produced by v2 calibration steps can reprice
//! instruments constructed *outside* the solver to reasonable tolerances.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCountContext, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::InflationLag;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, Seniority};
use finstack_quant_core::math::interp::ExtrapolationPolicy;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::calibration::api::engine;
use finstack_quant_valuations::calibration::api::schema::{
    CalibrationEnvelope, CalibrationPlan, CalibrationStep, DiscountCurveParams, ForwardCurveParams,
    HazardCurveParams, InflationCurveParams, StepParams,
};
use finstack_quant_valuations::calibration::{CalibrationConfig, CalibrationMethod};
use finstack_quant_valuations::instruments::rates::InflationSwap;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::PayReceive;
use finstack_quant_valuations::instruments::{
    ForwardRateAgreement, InterestRateFuture, InterestRateSwap,
};
use finstack_quant_valuations::market::build_cds_instrument;
use finstack_quant_valuations::market::build_rate_instrument;
use finstack_quant_valuations::market::conventions::ids::{
    CdsConventionKey, CdsDocClause, InflationSwapConventionId, IrFutureContractId,
};
use finstack_quant_valuations::market::conventions::ConventionRegistry;
use finstack_quant_valuations::market::quotes::cds::CdsQuote;
use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_quant_valuations::market::quotes::inflation::InflationQuote;
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
use finstack_quant_valuations::market::quotes::rates::RateQuote;
use finstack_quant_valuations::market::BuildCtx;
use rust_decimal::Decimal;
use time::Month;

use crate::common::fixtures;
use crate::finstack_quant_test_utils::calibration as cal_utils;

use super::tolerances;

/// FRA repricing tolerance per $1M notional.
const FRA_TOLERANCE_DOLLARS: f64 = tolerances::FRA_REPRICE_ABS_TOL_DOLLARS;
const CDS_TOLERANCE_DOLLARS: f64 = 5.0;
const INFLATION_TOLERANCE_DOLLARS: f64 = 5.0;

fn run_plan(envelope: &CalibrationEnvelope) -> MarketContext {
    let out = engine::execute(envelope).expect("calibration should succeed");
    MarketContext::try_from(out.result.final_market).expect("restore context")
}

fn forward_only_envelope(
    base_date: Date,
    quote: RateQuote,
    curve_id: &str,
    tenor_years: f64,
) -> CalibrationEnvelope {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (5.0, 0.78)])
        .build()
        .expect("discount curve");
    let initial_market = MarketContext::new().insert(discount);
    let (prior_market, mut market_data) = cal_utils::split_initial_market(&initial_market);
    let quotes = vec![MarketQuote::Rates(quote)];
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets = HashMap::default();
    quote_sets.insert("forward".to_string(), cal_utils::quote_set_ids(&quotes));

    CalibrationEnvelope {
        schema_url: None,
        schema: "finstack_quant.calibration/2".to_string(),
        plan: CalibrationPlan {
            id: "forward-global-reprice".to_string(),
            description: None,
            quote_sets,
            settings: CalibrationConfig::default(),
            steps: vec![CalibrationStep {
                id: "fwd".to_string(),
                quote_set: "forward".to_string(),
                params: StepParams::Forward(ForwardCurveParams {
                    curve_id: curve_id.into(),
                    currency: Currency::USD,
                    base_date,
                    tenor_years,
                    discount_curve_id: "USD-OIS".into(),
                    method: CalibrationMethod::GlobalSolve {
                        use_analytical_jacobian: false,
                    },
                    interpolation: Default::default(),
                    conventions: Default::default(),
                }),
            }],
        },
        market_data,
        prior_market,
    }
}

#[test]
fn discount_curve_deposit_repricing() {
    // Use a business day as base_date to avoid holiday adjustment complications.
    let base_date = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let currency = Currency::USD;

    // Market standard deposits are quoted by tenor (from spot).
    let deposit_quotes: Vec<RateQuote> = vec![
        RateQuote::Deposit {
            id: QuoteId::new("DEP-1M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(finstack_quant_core::dates::Tenor::parse("1M").unwrap()),
            rate: 0.045,
        },
        RateQuote::Deposit {
            id: QuoteId::new("DEP-3M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(finstack_quant_core::dates::Tenor::parse("3M").unwrap()),
            rate: 0.046,
        },
        RateQuote::Deposit {
            id: QuoteId::new("DEP-6M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(finstack_quant_core::dates::Tenor::parse("6M").unwrap()),
            rate: 0.047,
        },
    ];

    let mm_quotes: Vec<MarketQuote> = deposit_quotes
        .iter()
        .cloned()
        .map(MarketQuote::Rates)
        .collect();
    let mut market_data = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &mm_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("mm".to_string(), cal_utils::quote_set_ids(&mm_quotes));

    let settings = CalibrationConfig {
        solver: finstack_quant_valuations::calibration::SolverConfig::brent_default()
            .with_tolerance(1e-12)
            .with_max_iterations(200),
        ..Default::default()
    };

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings,
        steps: vec![CalibrationStep {
            id: "disc".to_string(),
            quote_set: "mm".to_string(),
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

        schema: "finstack_quant.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let ctx = run_plan(&envelope);

    let build_ctx = BuildCtx::new(
        base_date,
        fixtures::STANDARD_NOTIONAL,
        [("discount".to_string(), "USD-OIS".to_string())]
            .into_iter()
            .collect(),
    );

    for q in &deposit_quotes {
        let inst = build_rate_instrument(q, &build_ctx).expect("build deposit instrument");
        let pv = inst.value(&ctx, base_date).unwrap();
        assert!(
            pv.amount().abs() <= tolerances::REPRICE_PV_ABS_TOL_DOLLARS,
            "deposit should reprice within ${}. PV=${:.6}",
            tolerances::REPRICE_PV_ABS_TOL_DOLLARS,
            pv.amount(),
        );
    }
}

#[test]
fn discount_curve_swap_repricing() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let currency = Currency::USD;

    let deposit_quotes: Vec<RateQuote> = vec![
        RateQuote::Deposit {
            id: QuoteId::new("DEP-1M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("1M").unwrap()),
            rate: 0.045,
        },
        RateQuote::Deposit {
            id: QuoteId::new("DEP-3M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("3M").unwrap()),
            rate: 0.046,
        },
    ];

    let swap_quotes: Vec<RateQuote> = vec![
        RateQuote::Swap {
            id: QuoteId::new("OIS-1Y"),
            index: IndexId::new("USD-OIS"),
            pillar: Pillar::Tenor(Tenor::parse("1Y").unwrap()),
            rate: 0.0475,
            spread_decimal: None,
        },
        RateQuote::Swap {
            id: QuoteId::new("OIS-2Y"),
            index: IndexId::new("USD-OIS"),
            pillar: Pillar::Tenor(Tenor::parse("2Y").unwrap()),
            rate: 0.0485,
            spread_decimal: None,
        },
        RateQuote::Swap {
            id: QuoteId::new("OIS-5Y"),
            index: IndexId::new("USD-OIS"),
            pillar: Pillar::Tenor(Tenor::parse("5Y").unwrap()),
            rate: 0.0490,
            spread_decimal: None,
        },
    ];

    let mut disc_quotes: Vec<MarketQuote> = deposit_quotes
        .iter()
        .cloned()
        .map(MarketQuote::Rates)
        .collect();
    disc_quotes.extend(swap_quotes.iter().cloned().map(MarketQuote::Rates));

    let mut market_data = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &disc_quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("disc".to_string(), cal_utils::quote_set_ids(&disc_quotes));

    let settings = CalibrationConfig {
        solver: finstack_quant_valuations::calibration::SolverConfig::brent_default()
            .with_tolerance(1e-12)
            .with_max_iterations(200),
        ..Default::default()
    };

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings,
        steps: vec![CalibrationStep {
            id: "disc".to_string(),
            quote_set: "disc".to_string(),
            params: StepParams::Discount(DiscountCurveParams {
                curve_id: CurveId::from("USD-OIS"),
                currency,
                base_date,
                method: CalibrationMethod::GlobalSolve {
                    use_analytical_jacobian: true,
                },
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

        schema: "finstack_quant.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let ctx = run_plan(&envelope);
    let build_ctx = BuildCtx::new(
        base_date,
        fixtures::STANDARD_NOTIONAL,
        [
            ("discount".to_string(), "USD-OIS".to_string()),
            ("forward".to_string(), "USD-OIS".to_string()),
        ]
        .into_iter()
        .collect(),
    );

    for q in &swap_quotes {
        let inst = build_rate_instrument(q, &build_ctx).expect("build swap instrument");
        let pv = inst.value(&ctx, base_date).unwrap();
        assert!(
            pv.amount().abs() <= tolerances::REPRICE_PV_ABS_TOL_DOLLARS,
            "swap should reprice within ${}. PV=${:.6}",
            tolerances::REPRICE_PV_ABS_TOL_DOLLARS,
            pv.amount(),
        );
    }
}

#[test]
fn forward_curve_fra_repricing() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let currency = Currency::USD;

    // Discount quotes (minimal)
    let disc_quotes: Vec<RateQuote> = vec![
        RateQuote::Deposit {
            id: QuoteId::new(format!("DEP-{:?}", base_date + time::Duration::days(30))),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Date(base_date + time::Duration::days(30)),
            rate: 0.0450,
        },
        RateQuote::Deposit {
            id: QuoteId::new(format!("DEP-{:?}", base_date + time::Duration::days(90))),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Date(base_date + time::Duration::days(90)),
            rate: 0.0460,
        },
    ];

    // Forward quotes (FRAs)
    let fra_quotes: Vec<RateQuote> = vec![
        RateQuote::Fra {
            id: QuoteId::new(format!(
                "FRA-{:?}-{:?}",
                base_date + time::Duration::days(91),
                base_date + time::Duration::days(184)
            )),
            index: IndexId::new("USD-LIBOR-3M"),
            start: Pillar::Date(base_date + time::Duration::days(91)),
            end: Pillar::Date(base_date + time::Duration::days(184)),
            rate: 0.0470,
        },
        RateQuote::Fra {
            id: QuoteId::new(format!(
                "FRA-{:?}-{:?}",
                base_date + time::Duration::days(184),
                base_date + time::Duration::days(276)
            )),
            index: IndexId::new("USD-LIBOR-3M"),
            start: Pillar::Date(base_date + time::Duration::days(184)),
            end: Pillar::Date(base_date + time::Duration::days(276)),
            rate: 0.0480,
        },
    ];

    let disc_quote_bundle: Vec<MarketQuote> = disc_quotes
        .iter()
        .cloned()
        .map(MarketQuote::Rates)
        .collect();
    let fra_quote_bundle: Vec<MarketQuote> =
        fra_quotes.iter().cloned().map(MarketQuote::Rates).collect();
    let mut market_data = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &disc_quote_bundle);
    cal_utils::extend_market_data(&mut market_data, &fra_quote_bundle);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert(
        "disc".to_string(),
        cal_utils::quote_set_ids(&disc_quote_bundle),
    );
    quote_sets.insert(
        "fra".to_string(),
        cal_utils::quote_set_ids(&fra_quote_bundle),
    );

    let settings = CalibrationConfig {
        solver: finstack_quant_valuations::calibration::SolverConfig::brent_default()
            .with_tolerance(1e-12)
            .with_max_iterations(200),
        ..Default::default()
    };

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings,
        steps: vec![
            CalibrationStep {
                id: "disc".to_string(),
                quote_set: "disc".to_string(),
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
            },
            CalibrationStep {
                id: "fwd".to_string(),
                quote_set: "fra".to_string(),
                params: StepParams::Forward(ForwardCurveParams {
                    curve_id: CurveId::from("USD-SOFR-3M"),
                    currency,
                    base_date,
                    tenor_years: 0.25,
                    discount_curve_id: CurveId::from("USD-OIS"),
                    method: CalibrationMethod::GlobalSolve {
                        use_analytical_jacobian: false,
                    },
                    interpolation: Default::default(),
                    conventions: Default::default(),
                }),
            },
        ],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack_quant.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let ctx = run_plan(&envelope);
    let forward = ctx
        .get_forward("USD-SOFR-3M")
        .expect("calibrated forward curve");
    let build_ctx = BuildCtx::new(
        base_date,
        fixtures::STANDARD_NOTIONAL,
        [
            ("discount".to_string(), "USD-OIS".to_string()),
            ("forward".to_string(), "USD-SOFR-3M".to_string()),
        ]
        .into_iter()
        .collect(),
    );

    for q in &fra_quotes {
        let rate = match q {
            RateQuote::Fra { rate, .. } => *rate,
            _ => continue,
        };
        let instrument = build_rate_instrument(q, &build_ctx).expect("build FRA instrument");
        let fra = instrument
            .as_any()
            .downcast_ref::<ForwardRateAgreement>()
            .expect("rate quote should build an FRA");

        let t_start = forward
            .day_count()
            .year_fraction(
                forward.base_date(),
                fra.start_date,
                DayCountContext::default(),
            )
            .expect("valid FRA start time");
        let t_end = forward
            .day_count()
            .year_fraction(
                forward.base_date(),
                fra.maturity,
                DayCountContext::default(),
            )
            .expect("valid FRA end time");
        let implied_rate = forward
            .rate_between(t_start, t_end)
            .expect("valid DF-implied FRA rate");
        assert!(
            forward
                .knots()
                .iter()
                .any(|knot| (*knot - t_start).abs() < 1e-12),
            "calibrated curve should retain actual reset time {t_start:.12}"
        );
        assert!(
            (forward.rate(t_start) - rate).abs() <= tolerances::FWD_RATE_ABS_TOL,
            "reset-date knot should retain fixed-tenor quote meaning: knot={:.12}, quote={rate:.12}",
            forward.rate(t_start)
        );
        assert!(
            (implied_rate - rate).abs() <= tolerances::FWD_RATE_ABS_TOL,
            "DF-implied FRA rate should reprice quote: implied={implied_rate:.12}, quote={rate:.12}"
        );

        let pv = instrument.value(&ctx, base_date).unwrap();
        assert!(
            pv.amount().abs() <= FRA_TOLERANCE_DOLLARS,
            "fra should reprice within ${}. PV=${:.2}",
            FRA_TOLERANCE_DOLLARS,
            pv.amount()
        );
    }
    let final_end = fra_quotes
        .iter()
        .filter_map(|quote| {
            let instrument = build_rate_instrument(quote, &build_ctx).ok()?;
            instrument
                .as_any()
                .downcast_ref::<ForwardRateAgreement>()
                .map(|fra| fra.maturity)
        })
        .max()
        .expect("at least one FRA end date");
    let final_end_time = forward
        .day_count()
        .year_fraction(forward.base_date(), final_end, DayCountContext::default())
        .expect("valid final reset-grid end");
    let projection_grid = forward
        .projection_grid()
        .expect("calibrated curve must carry contractual projection boundaries");
    assert!((projection_grid[projection_grid.len() - 1] - final_end_time).abs() < 1e-12);
}

#[test]
fn forward_curve_future_global_solve_reprices_df_implied_quote() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base date");
    let quote = RateQuote::Futures {
        id: QuoteId::new("SR3-MAR25"),
        contract: IrFutureContractId::new("CME:SR3"),
        expiry: Date::from_calendar_date(2025, Month::March, 17).expect("expiry"),
        price: 95.0,
        convexity_adjustment: Some(0.0),
        vol_surface_id: None,
    };
    let envelope = forward_only_envelope(base_date, quote.clone(), "USD-SOFR-3M", 0.25);
    let output = engine::execute(&envelope).expect("future forward calibration");
    let report = output
        .result
        .step_reports
        .get("fwd")
        .expect("forward report");
    assert!(report.success, "{}", report.convergence_reason);
    assert_eq!(
        report.metadata.get("method").map(String::as_str),
        Some("global_fit_lm_weighted_lsq")
    );
    let context = MarketContext::try_from(output.result.final_market).expect("calibrated context");
    let curve = context.get_forward("USD-SOFR-3M").expect("forward curve");
    assert!(curve.projection_grid().is_some());
    let build_ctx = BuildCtx::new(
        base_date,
        fixtures::STANDARD_NOTIONAL,
        [
            ("discount".to_string(), "USD-OIS".to_string()),
            ("forward".to_string(), "USD-SOFR-3M".to_string()),
        ]
        .into_iter()
        .collect(),
    );
    let instrument = build_rate_instrument(&quote, &build_ctx).expect("future instrument");
    let future = instrument
        .as_any()
        .downcast_ref::<InterestRateFuture>()
        .expect("interest-rate future");
    let period_start = future.period_start.expect("resolved future period start");
    let period_end = future.period_end.expect("resolved future period end");
    let t_start = curve
        .day_count()
        .year_fraction(curve.base_date(), period_start, DayCountContext::default())
        .expect("start time");
    let t_end = curve
        .day_count()
        .year_fraction(curve.base_date(), period_end, DayCountContext::default())
        .expect("end time");
    let implied = curve
        .rate_between(t_start, t_end)
        .expect("DF-implied futures rate");
    assert!((implied - 0.05).abs() < 1e-8);
    let pv = instrument
        .value(&context, base_date)
        .expect("future repricing");
    assert!(pv.amount().abs() <= tolerances::REPRICE_PV_ABS_TOL_DOLLARS);
}

#[test]
fn forward_curve_swap_global_solve_reprices_df_implied_periods() {
    let base_date = Date::from_calendar_date(2025, Month::January, 2).expect("base date");
    let quote = RateQuote::Swap {
        id: QuoteId::new("USD-LIBOR-3M-SWAP-1Y"),
        index: IndexId::new("USD-LIBOR-3M"),
        pillar: Pillar::Tenor(Tenor::parse("1Y").expect("tenor")),
        rate: 0.05,
        spread_decimal: None,
    };
    let envelope = forward_only_envelope(base_date, quote.clone(), "USD-LIBOR-3M", 0.25);
    let output = engine::execute(&envelope).expect("swap forward calibration");
    let report = output
        .result
        .step_reports
        .get("fwd")
        .expect("forward report");
    assert!(report.success, "{}", report.convergence_reason);
    assert_eq!(
        report.metadata.get("method").map(String::as_str),
        Some("global_fit_lm_weighted_lsq")
    );
    let context = MarketContext::try_from(output.result.final_market).expect("calibrated context");
    let curve = context.get_forward("USD-LIBOR-3M").expect("forward curve");
    assert!(curve.projection_grid().is_some());
    let build_ctx = BuildCtx::new(
        base_date,
        fixtures::STANDARD_NOTIONAL,
        [
            ("discount".to_string(), "USD-OIS".to_string()),
            ("forward".to_string(), "USD-LIBOR-3M".to_string()),
        ]
        .into_iter()
        .collect(),
    );
    let instrument = build_rate_instrument(&quote, &build_ctx).expect("swap instrument");
    let _swap = instrument
        .as_any()
        .downcast_ref::<InterestRateSwap>()
        .expect("interest-rate swap");
    let projection_grid = curve
        .projection_grid()
        .expect("contractual swap projection grid");
    for period in projection_grid.windows(2) {
        let t_start = period[0];
        let t_end = period[1];
        let df_implied = curve
            .rate_between(t_start, t_end)
            .expect("DF-implied swap period");
        assert!((df_implied - curve.rate(t_start)).abs() < 1e-10);
    }
    let pv = instrument
        .value(&context, base_date)
        .expect("swap repricing");
    assert!(pv.amount().abs() <= tolerances::REPRICE_PV_ABS_TOL_DOLLARS);
}

#[test]
fn hazard_curve_cds_repricing() {
    let base_date = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc_quotes: Vec<RateQuote> = vec![
        RateQuote::Deposit {
            id: QuoteId::new("DEP-1M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("1M").unwrap()),
            rate: 0.045,
        },
        RateQuote::Deposit {
            id: QuoteId::new("DEP-6M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("6M").unwrap()),
            rate: 0.047,
        },
    ];

    let cds_quotes: Vec<CdsQuote> = vec![
        CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-1Y"),
            entity: "REPRICE-ACME".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2026, Month::March, 20).unwrap()),
            spread_bp: 120.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        },
        CdsQuote::CdsParSpread {
            id: QuoteId::new("CDS-3Y"),
            entity: "REPRICE-ACME".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2028, Month::March, 20).unwrap()),
            spread_bp: 160.0,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        },
    ];

    let disc_quote_bundle: Vec<MarketQuote> = disc_quotes
        .iter()
        .cloned()
        .map(MarketQuote::Rates)
        .collect();
    let cds_quote_bundle: Vec<MarketQuote> =
        cds_quotes.iter().cloned().map(MarketQuote::Cds).collect();
    let mut market_data = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &disc_quote_bundle);
    cal_utils::extend_market_data(&mut market_data, &cds_quote_bundle);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert(
        "disc".to_string(),
        cal_utils::quote_set_ids(&disc_quote_bundle),
    );
    quote_sets.insert(
        "cds".to_string(),
        cal_utils::quote_set_ids(&cds_quote_bundle),
    );

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![
            CalibrationStep {
                id: "disc".to_string(),
                quote_set: "disc".to_string(),
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
            },
            CalibrationStep {
                id: "haz".to_string(),
                quote_set: "cds".to_string(),
                params: StepParams::Hazard(HazardCurveParams {
                    curve_id: "REPRICE-ACME-SENIOR".into(),
                    entity: "REPRICE-ACME".to_string(),
                    seniority: Seniority::Senior,
                    currency,
                    base_date,
                    discount_curve_id: "USD-OIS".into(),
                    recovery_rate: 0.40,
                    notional: 1.0,
                    method: CalibrationMethod::Bootstrap,
                    interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                    par_interp:
                        finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                    doc_clause: None,
                    cds_valuation_convention: None,
                }),
            },
        ],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack_quant.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let ctx = run_plan(&envelope);

    let mut curve_ids = HashMap::default();
    curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("credit".to_string(), "REPRICE-ACME-SENIOR".to_string());
    let build_ctx = BuildCtx::new(base_date, fixtures::STANDARD_NOTIONAL, curve_ids);

    for quote in &cds_quotes {
        let inst = build_cds_instrument(quote, &build_ctx).expect("build cds instrument");
        let pv = inst.value(&ctx, base_date).expect("cds valuation");
        assert!(
            pv.amount().abs() <= CDS_TOLERANCE_DOLLARS,
            "cds should reprice within ${}. PV=${:.6}",
            CDS_TOLERANCE_DOLLARS,
            pv.amount(),
        );
    }
}

#[test]
fn hazard_curve_step_report_matches_market_built_cds_repricing() {
    let base_date = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc_quotes: Vec<RateQuote> = vec![
        RateQuote::Deposit {
            id: QuoteId::new("DEP-1M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("1M").unwrap()),
            rate: 0.045,
        },
        RateQuote::Deposit {
            id: QuoteId::new("DEP-6M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("6M").unwrap()),
            rate: 0.047,
        },
    ];

    let quote = CdsQuote::CdsParSpread {
        id: QuoteId::new("CDS-3Y"),
        entity: "REPRICE-DIAG".to_string(),
        pillar: Pillar::Date(Date::from_calendar_date(2028, Month::March, 20).unwrap()),
        spread_bp: 100.0,
        recovery_rate: 0.40,
        convention: CdsConventionKey {
            currency,
            doc_clause: CdsDocClause::IsdaNa,
        },
    };

    let disc_quote_bundle: Vec<MarketQuote> = disc_quotes
        .iter()
        .cloned()
        .map(MarketQuote::Rates)
        .collect();
    let cds_quote_bundle: Vec<MarketQuote> = vec![MarketQuote::Cds(quote.clone())];
    let mut market_data = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &disc_quote_bundle);
    cal_utils::extend_market_data(&mut market_data, &cds_quote_bundle);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert(
        "disc".to_string(),
        cal_utils::quote_set_ids(&disc_quote_bundle),
    );
    quote_sets.insert(
        "cds".to_string(),
        cal_utils::quote_set_ids(&cds_quote_bundle),
    );

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![
            CalibrationStep {
                id: "disc".to_string(),
                quote_set: "disc".to_string(),
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
            },
            CalibrationStep {
                id: "haz".to_string(),
                quote_set: "cds".to_string(),
                params: StepParams::Hazard(HazardCurveParams {
                    curve_id: "REPRICE-DIAG-SENIOR".into(),
                    entity: "REPRICE-DIAG".to_string(),
                    seniority: Seniority::Senior,
                    currency,
                    base_date,
                    discount_curve_id: "USD-OIS".into(),
                    recovery_rate: 0.40,
                    notional: 1.0,
                    method: CalibrationMethod::Bootstrap,
                    interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                    par_interp:
                        finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                    doc_clause: None,
                    cds_valuation_convention: None,
                }),
            },
        ],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack_quant.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let out = engine::execute(&envelope).expect("calibration should succeed");
    let ctx = MarketContext::try_from(out.result.final_market).expect("restore context");
    let haz_report = out
        .result
        .step_reports
        .get("haz")
        .expect("hazard step report");

    let mut unit_curve_ids = HashMap::default();
    unit_curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
    unit_curve_ids.insert("credit".to_string(), "REPRICE-DIAG-SENIOR".to_string());
    let unit_build_ctx = BuildCtx::new(base_date, 1.0, unit_curve_ids);
    let unit_inst =
        build_cds_instrument(&quote, &unit_build_ctx).expect("build unit cds instrument");
    let prepared_pv = unit_inst
        .value_raw(&ctx, base_date)
        .expect("unit cds valuation");

    let mut curve_ids = HashMap::default();
    curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("credit".to_string(), "REPRICE-DIAG-SENIOR".to_string());
    let build_ctx = BuildCtx::new(base_date, fixtures::STANDARD_NOTIONAL, curve_ids);
    let inst = build_cds_instrument(&quote, &build_ctx).expect("build cds instrument");
    let pv = inst.value(&ctx, base_date).expect("cds valuation");

    let report_residual_dollars = haz_report.max_residual * fixtures::STANDARD_NOTIONAL;
    assert!(
        report_residual_dollars.abs() <= CDS_TOLERANCE_DOLLARS
            && prepared_pv.abs() * fixtures::STANDARD_NOTIONAL <= CDS_TOLERANCE_DOLLARS
            && pv.amount().abs() <= CDS_TOLERANCE_DOLLARS,
        "hazard report residual ${:.6}, unit PV ${:.6} per-unit, and repriced PV ${:.6} should all be within ${}",
        report_residual_dollars,
        prepared_pv,
        pv.amount(),
        CDS_TOLERANCE_DOLLARS,
    );
}

#[test]
fn hazard_curve_standard_upfront_cds_repricing() {
    let base_date = Date::from_calendar_date(2025, Month::March, 20).unwrap();
    let currency = Currency::USD;

    let disc_quotes: Vec<RateQuote> = vec![
        RateQuote::Deposit {
            id: QuoteId::new("DEP-1M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("1M").unwrap()),
            rate: 0.045,
        },
        RateQuote::Deposit {
            id: QuoteId::new("DEP-6M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("6M").unwrap()),
            rate: 0.047,
        },
    ];

    let cds_quotes: Vec<CdsQuote> = vec![
        CdsQuote::CdsUpfront {
            id: QuoteId::new("CDS-UP-3Y"),
            entity: "REPRICE-UPFRONT".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2028, Month::March, 20).unwrap()),
            running_spread_bp: 100.0,
            upfront_pct: 0.015,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        },
        CdsQuote::CdsUpfront {
            id: QuoteId::new("CDS-UP-5Y"),
            entity: "REPRICE-UPFRONT".to_string(),
            pillar: Pillar::Date(Date::from_calendar_date(2030, Month::March, 20).unwrap()),
            running_spread_bp: 500.0,
            upfront_pct: 0.045,
            recovery_rate: 0.40,
            convention: CdsConventionKey {
                currency,
                doc_clause: CdsDocClause::IsdaNa,
            },
        },
    ];

    let disc_quote_bundle: Vec<MarketQuote> = disc_quotes
        .iter()
        .cloned()
        .map(MarketQuote::Rates)
        .collect();
    let cds_quote_bundle: Vec<MarketQuote> =
        cds_quotes.iter().cloned().map(MarketQuote::Cds).collect();
    let mut market_data = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &disc_quote_bundle);
    cal_utils::extend_market_data(&mut market_data, &cds_quote_bundle);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert(
        "disc".to_string(),
        cal_utils::quote_set_ids(&disc_quote_bundle),
    );
    quote_sets.insert(
        "cds".to_string(),
        cal_utils::quote_set_ids(&cds_quote_bundle),
    );

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![
            CalibrationStep {
                id: "disc".to_string(),
                quote_set: "disc".to_string(),
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
            },
            CalibrationStep {
                id: "haz".to_string(),
                quote_set: "cds".to_string(),
                params: StepParams::Hazard(HazardCurveParams {
                    curve_id: "REPRICE-UPFRONT-SENIOR".into(),
                    entity: "REPRICE-UPFRONT".to_string(),
                    seniority: Seniority::Senior,
                    currency,
                    base_date,
                    discount_curve_id: "USD-OIS".into(),
                    recovery_rate: 0.40,
                    notional: 1.0,
                    method: CalibrationMethod::Bootstrap,
                    interpolation: finstack_quant_core::math::interp::InterpStyle::LogLinear,
                    par_interp:
                        finstack_quant_core::market_data::term_structures::ParInterp::Linear,
                    doc_clause: None,
                    cds_valuation_convention: None,
                }),
            },
        ],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack_quant.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let ctx = run_plan(&envelope);

    let mut curve_ids = HashMap::default();
    curve_ids.insert("discount".to_string(), "USD-OIS".to_string());
    curve_ids.insert("credit".to_string(), "REPRICE-UPFRONT-SENIOR".to_string());
    let build_ctx = BuildCtx::new(base_date, fixtures::STANDARD_NOTIONAL, curve_ids);

    for quote in &cds_quotes {
        let inst = build_cds_instrument(quote, &build_ctx).expect("build cds instrument");
        let pv = inst.value(&ctx, base_date).expect("cds valuation");
        assert!(
            pv.amount().abs() <= CDS_TOLERANCE_DOLLARS,
            "standard upfront cds should reprice within ${}. PV=${:.6}",
            CDS_TOLERANCE_DOLLARS,
            pv.amount(),
        );
    }
}

#[test]
fn inflation_curve_swap_repricing() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let currency = Currency::USD;
    let base_cpi = 100.0;

    let disc_quotes: Vec<RateQuote> = vec![
        RateQuote::Deposit {
            id: QuoteId::new("DEP-1M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("1M").unwrap()),
            rate: 0.045,
        },
        RateQuote::Deposit {
            id: QuoteId::new("DEP-3M"),
            index: IndexId::new("USD-Deposit"),
            pillar: Pillar::Tenor(Tenor::parse("3M").unwrap()),
            rate: 0.046,
        },
    ];

    let infl_quotes: Vec<InflationQuote> = vec![
        InflationQuote::InflationSwap {
            id: QuoteId::new("USD-CPI-ZCIS-20270115"),
            maturity: Date::from_calendar_date(2027, Month::January, 15).unwrap(),
            rate: 0.02,
            index: "USD-CPI".to_string(),
            convention: InflationSwapConventionId::new("USD"),
        },
        InflationQuote::InflationSwap {
            id: QuoteId::new("USD-CPI-ZCIS-20300115"),
            maturity: Date::from_calendar_date(2030, Month::January, 15).unwrap(),
            rate: 0.025,
            index: "USD-CPI".to_string(),
            convention: InflationSwapConventionId::new("USD"),
        },
    ];

    let disc_quote_bundle: Vec<MarketQuote> = disc_quotes
        .iter()
        .cloned()
        .map(MarketQuote::Rates)
        .collect();
    let infl_quote_bundle: Vec<MarketQuote> = infl_quotes
        .iter()
        .cloned()
        .map(MarketQuote::Inflation)
        .collect();
    let mut market_data = Vec::new();
    cal_utils::extend_market_data(&mut market_data, &disc_quote_bundle);
    cal_utils::extend_market_data(&mut market_data, &infl_quote_bundle);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert(
        "disc".to_string(),
        cal_utils::quote_set_ids(&disc_quote_bundle),
    );
    quote_sets.insert(
        "infl".to_string(),
        cal_utils::quote_set_ids(&infl_quote_bundle),
    );

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: Default::default(),
        steps: vec![
            CalibrationStep {
                id: "disc".to_string(),
                quote_set: "disc".to_string(),
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
            },
            CalibrationStep {
                id: "infl".to_string(),
                quote_set: "infl".to_string(),
                params: StepParams::Inflation(InflationCurveParams {
                    curve_id: "USD-CPI".into(),
                    currency,
                    base_date,
                    discount_curve_id: "USD-OIS".into(),
                    index: "USD-CPI".to_string(),
                    observation_lag: "3M".to_string(),
                    base_cpi,
                    notional: 1.0,
                    method: CalibrationMethod::Bootstrap,
                    interpolation: Default::default(),
                    seasonal_factors: None,
                }),
            },
        ],
    };

    let envelope = CalibrationEnvelope {
        schema_url: None,

        schema: "finstack_quant.calibration/2".to_string(),
        plan,
        market_data,
        prior_market: Vec::new(),
    };

    let ctx = run_plan(&envelope);
    let conventions = ConventionRegistry::try_global()
        .expect("convention registry")
        .require_inflation_swap(&InflationSwapConventionId::new("USD"))
        .expect("inflation swap conventions");
    let lag = conventions
        .inflation_lag
        .months()
        .map(|months| InflationLag::Months(months as u8))
        .unwrap_or(InflationLag::None);

    for quote in &infl_quotes {
        let (maturity, rate) = match quote {
            InflationQuote::InflationSwap { maturity, rate, .. } => (*maturity, *rate),
            InflationQuote::YoYInflationSwap { .. } => continue,
        };
        let swap = InflationSwap::builder()
            .id(format!("INF-SWAP-{}", maturity).into())
            .notional(Money::new(fixtures::STANDARD_NOTIONAL, currency))
            .start_date(base_date)
            .maturity(maturity)
            .fixed_rate(Decimal::try_from(rate).expect("valid decimal"))
            .inflation_index_id("USD-CPI".into())
            .discount_curve_id("USD-OIS".into())
            .day_count(conventions.day_count)
            .side(PayReceive::Pay)
            .lag_override_opt(Some(lag))
            .base_cpi_opt(Some(base_cpi))
            .bdc(conventions.business_day_convention)
            .calendar_id_opt(Some(conventions.calendar_id.clone().into()))
            .build()
            .expect("inflation swap build");

        let pv = swap
            .value(&ctx, base_date)
            .expect("inflation swap valuation");
        assert!(
            pv.amount().abs() <= INFLATION_TOLERANCE_DOLLARS,
            "inflation swap should reprice within ${}. PV=${:.6}",
            INFLATION_TOLERANCE_DOLLARS,
            pv.amount(),
        );
    }
}
