//! Integration test for base correlation calibration (v2).

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, HazardCurve,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::calibration::api::engine;
use finstack_quant_valuations::calibration::api::schema::{
    BaseCorrelationParams, CalibrationEnvelope, CalibrationPlan, CalibrationStep, StepParams,
};
use finstack_quant_valuations::calibration::CalibrationConfig;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranchePricer;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
    CDSTranche, TrancheSide,
};
use finstack_quant_valuations::instruments::Attributes;
use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};

use crate::finstack_quant_test_utils::calibration as cal_utils;
use finstack_quant_core::HashMap;
use finstack_quant_valuations::market::quotes::cds_tranche::CDSTrancheQuote;
use finstack_quant_valuations::market::quotes::ids::QuoteId;
use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
use std::env;
use std::sync::Arc;
use time::Month;

use super::tolerances;

// Fixture upfronts (percent of tranche notional) generated from a frozen market snapshot.
// To regenerate after a pricing model change:
// FINSTACK_REGEN_BASE_CORR_FIXTURES=1 cargo test -p finstack-quant-valuations base_correlation_step_builds_curve_and_updates_credit_index_data -- --nocapture
const UPFRONT_0_3_PCT: f64 = 3.98577377;
const UPFRONT_3_7_PCT: f64 = -4.30385846;

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

fn create_hazard_curve(base_date: Date) -> Arc<HazardCurve> {
    Arc::new(
        HazardCurve::builder("CDX_HAZARD")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.40)
            .knots([(0.0, 0.0010), (5.0, 0.0012), (10.0, 0.0015)])
            .build()
            .expect("hazard curve"),
    )
}

fn create_credit_index(
    hazard: Arc<HazardCurve>,
    base_corr: Arc<BaseCorrelationCurve>,
) -> CreditIndexData {
    CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(hazard)
        .base_correlation_curve(base_corr)
        .build()
        .expect("credit index")
}

fn tranche_upfront_pct(
    base_date: Date,
    maturity: Date,
    attach_pct: f64,
    detach_pct: f64,
    running_coupon_bp: f64,
    notional: f64,
    market: &MarketContext,
) -> f64 {
    let tranche = CDSTranche::builder()
        .id("QUOTE_TRANCHE".into())
        .index_name("CDX".to_string())
        .series(40)
        .attach_pct(attach_pct)
        .detach_pct(detach_pct)
        .notional(Money::new(notional, Currency::USD))
        .maturity(maturity)
        .running_coupon_bp(running_coupon_bp)
        .frequency(Tenor::quarterly())
        .day_count(DayCount::Act360)
        .bdc(BusinessDayConvention::Following)
        .calendar_id_opt(None)
        .discount_curve_id(CurveId::from("USD-OIS"))
        .credit_index_id(CurveId::from("CDX"))
        .side(TrancheSide::BuyProtection)
        .effective_date_opt(None)
        .accumulated_loss(0.0)
        .standard_imm_dates(true)
        .attributes(Attributes::new())
        .build()
        .expect("tranche");

    let pv = CDSTranchePricer::new()
        .price_tranche(&tranche, market, base_date)
        .expect("price")
        .amount();

    (pv / notional) * 100.0
}

fn fixture_upfronts(
    base_date: Date,
    maturity: Date,
    running_coupon_bp: f64,
    notional: f64,
    quote_market: &MarketContext,
) -> (f64, f64) {
    if env::var("FINSTACK_REGEN_BASE_CORR_FIXTURES").is_ok() {
        let upfront_0_3 = tranche_upfront_pct(
            base_date,
            maturity,
            0.0,
            3.0,
            running_coupon_bp,
            notional,
            quote_market,
        );
        let upfront_3_7 = tranche_upfront_pct(
            base_date,
            maturity,
            3.0,
            7.0,
            running_coupon_bp,
            notional,
            quote_market,
        );
        println!("UPFRONT_0_3_PCT={upfront_0_3:.8}");
        println!("UPFRONT_3_7_PCT={upfront_3_7:.8}");
        return (upfront_0_3, upfront_3_7);
    }

    (UPFRONT_0_3_PCT, UPFRONT_3_7_PCT)
}

#[test]
fn base_correlation_step_builds_curve_and_updates_credit_index_data() {
    let base_date = Date::from_calendar_date(2025, Month::March, 20).expect("base_date");
    let maturity = Date::from_calendar_date(2030, Month::March, 20).expect("maturity");

    // Generate tranche quotes from a known "target" base correlation curve.
    let hazard = create_hazard_curve(base_date);
    let target_corr = Arc::new(
        BaseCorrelationCurve::builder("TARGET")
            .knots([(3.0, 0.25), (7.0, 0.35)])
            .build()
            .expect("target base correlation"),
    );

    let quote_market = MarketContext::new()
        .insert(create_discount_curve(base_date))
        .insert(hazard.as_ref().clone())
        .insert(target_corr.as_ref().clone())
        .insert_credit_index(
            "CDX",
            create_credit_index(Arc::clone(&hazard), Arc::clone(&target_corr)),
        );

    let notional = 1.0;
    let running_coupon_bp = 100.0;
    let (upfront_0_3, upfront_3_7) = fixture_upfronts(
        base_date,
        maturity,
        running_coupon_bp,
        notional,
        &quote_market,
    );

    // Start calibration from a different seed curve to ensure the step updates the context.
    let seed_corr = Arc::new(
        BaseCorrelationCurve::builder("SEED")
            .knots([(3.0, 0.10), (7.0, 0.15)])
            .build()
            .expect("seed base correlation"),
    );
    let initial_market = MarketContext::new()
        .insert(create_discount_curve(base_date))
        .insert(hazard.as_ref().clone())
        .insert(seed_corr.as_ref().clone())
        .insert_credit_index(
            "CDX",
            create_credit_index(Arc::clone(&hazard), Arc::clone(&seed_corr)),
        );

    // Use fraction attachment/detachment in the quote to validate unit normalization.
    let quotes = vec![
        MarketQuote::CDSTranche(CDSTrancheQuote::CDSTranche {
            id: QuoteId::new("TRANCHE-1"),
            index: "CDX".to_string(),
            attachment: 0.0,
            detachment: 0.03,
            maturity,
            upfront_pct: upfront_0_3,
            running_spread_bp: running_coupon_bp,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
        MarketQuote::CDSTranche(CDSTrancheQuote::CDSTranche {
            id: QuoteId::new("TRANCHE-2"),
            index: "CDX".to_string(),
            attachment: 0.03,
            detachment: 0.07,
            maturity,
            upfront_pct: upfront_3_7,
            running_spread_bp: running_coupon_bp,
            convention: CdsConventionKey {
                currency: Currency::USD,
                doc_clause: CdsDocClause::IsdaNa,
            },
        }),
    ];

    let (prior, mut market_data) = cal_utils::split_initial_market(&initial_market);
    cal_utils::extend_market_data(&mut market_data, &quotes);
    let mut quote_sets: HashMap<String, Vec<QuoteId>> = HashMap::default();
    quote_sets.insert("tranches".to_string(), cal_utils::quote_set_ids(&quotes));

    let plan = CalibrationPlan {
        id: "plan".to_string(),
        description: None,
        quote_sets,
        settings: CalibrationConfig {
            solver: finstack_quant_valuations::calibration::SolverConfig::brent_default()
                .with_tolerance(tolerances::BASE_CORR_SOLVER_TOL)
                .with_max_iterations(500),
            ..Default::default()
        },
        steps: vec![CalibrationStep {
            id: "corr".to_string(),
            quote_set: "tranches".to_string(),
            params: StepParams::BaseCorrelation(BaseCorrelationParams {
                index_id: "CDX".to_string(),
                series: 40,
                maturity_years: 5.0,
                base_date,
                discount_curve_id: CurveId::from("USD-OIS"),
                currency: Currency::USD,
                notional,
                frequency: Some(Tenor::quarterly()),
                day_count: Some(DayCount::Act360),
                bdc: Some(BusinessDayConvention::Following),
                calendar_id: None,
                detachment_points: vec![0.03, 0.07],
                use_imm_dates: true,
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
    let step = result.result.step_reports.get("corr").expect("step report");
    assert!(step.success);
    assert!(
        step.max_residual <= tolerances::BASE_CORR_UPFRONT_FRAC_TOL,
        "base correlation fit must be vendor-grade: max_residual={:.3e} > tol={:.3e}",
        step.max_residual,
        tolerances::BASE_CORR_UPFRONT_FRAC_TOL
    );

    let ctx = MarketContext::try_from(result.result.final_market).expect("restore context");

    // Calibrated curve is inserted as "{index_id}_CORR".
    let curve = ctx
        .get_base_correlation("CDX_CORR")
        .expect("base correlation curve");
    let arb = curve.validate_arbitrage_free();
    assert!(
        arb.is_arbitrage_free,
        "calibrated base correlation curve must be arbitrage-free; violations={:?}",
        arb.violations
    );

    // Credit index aggregate is updated to reference the calibrated curve.
    let index = ctx.get_credit_index("CDX").expect("credit index");
    assert_eq!(index.base_correlation_curve.id().as_str(), "CDX_CORR");
}
