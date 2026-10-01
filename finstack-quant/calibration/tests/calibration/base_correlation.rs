//! Base-correlation calibration must reprice the complete settlement-quoted trade.

use finstack_quant_calibration::api::engine;
use finstack_quant_calibration::api::schema::{
    BaseCorrelationParams, CalibrationEnvelope, CalibrationPlan, CalibrationSchema,
    CalibrationStep, StepParams,
};
use finstack_quant_calibration::quotes::cds_tranche::CdsTrancheQuote;
use finstack_quant_calibration::quotes::ids::QuoteId;
use finstack_quant_calibration::quotes::market_quote::MarketQuote;
use finstack_quant_calibration::CalibrationConfig;
use finstack_quant_cashflows::builder::specs::RollRule;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    adjust, calendar_by_id_strict, prev_cds_date, Date, DateExt, DayCount,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    BaseCorrelationCurve, CreditIndexData, DiscountCurve, HazardCurve,
};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::{
    CdsTranche, CdsTranchePricer,
};
use finstack_quant_valuations::instruments::{Attributes, PayReceive};
use finstack_quant_valuations::market::conventions::ids::{CdsConventionKey, CdsDocClause};
use finstack_quant_valuations::market::conventions::ConventionRegistry;
use std::sync::Arc;
use time::Month;

use super::tolerances;
use crate::calibration::calibration_support as cal_utils;

fn create_market(base_date: Date, correlations: [f64; 2]) -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
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
        .expect("discount curve");
    let hazard = Arc::new(
        HazardCurve::builder("CDX_HAZARD")
            .base_date(base_date)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.40)
            .knots([(0.0, 0.0010), (5.0, 0.0012), (10.0, 0.0015)])
            .build()
            .expect("hazard curve"),
    );
    let correlation = Arc::new(
        BaseCorrelationCurve::builder("CDX_CORR")
            .knots([(3.0, correlations[0]), (7.0, correlations[1])])
            .build()
            .expect("base correlation"),
    );
    let index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::clone(&hazard))
        .base_correlation_curve(Arc::clone(&correlation))
        .build()
        .expect("credit index");
    MarketContext::new()
        .insert(discount)
        .insert(hazard.as_ref().clone())
        .insert(correlation.as_ref().clone())
        .insert_credit_index("CDX", index)
        .expect("credit index dependencies")
}

fn build_tranche(quote: &CdsTrancheQuote, base_date: Date, notional: f64) -> CdsTranche {
    let registry = ConventionRegistry::try_global().expect("conventions");
    let convention = registry
        .resolve_cds(&quote.convention)
        .expect("CDS convention");
    let calendar = calendar_by_id_strict(&convention.calendar_id).expect("calendar");
    let settlement = adjust(
        base_date
            .add_business_days(i32::from(convention.settlement_days), calendar)
            .expect("cash settlement lag"),
        convention.business_day_convention,
        calendar,
    )
    .expect("cash settlement date");
    let tranche_notional = notional * (quote.detachment - quote.attachment);
    // Construct through the public instrument API, independently of the
    // calibration quote builder, to check its economic contract end to end.
    let mut tranche = CdsTranche::builder()
        .id(quote.id.as_str().into())
        .index_name(quote.index.clone())
        .series(quote.series)
        .attach_pct(quote.attachment * 100.0)
        .detach_pct(quote.detachment * 100.0)
        .notional(Money::new(tranche_notional, quote.convention.currency).expect("notional"))
        .maturity(quote.maturity)
        .coupon_bp(quote.coupon_bp)
        .frequency(convention.frequency)
        .day_count(convention.day_count)
        .business_day_convention(convention.business_day_convention)
        .calendar_id_opt(Some(convention.calendar_id.clone().into()))
        .discount_curve_id("USD-OIS".into())
        .credit_index_id("CDX".into())
        .side(PayReceive::Pay)
        .start_date_opt(Some(
            prev_cds_date(base_date).expect("prior quarterly roll"),
        ))
        .realized_loss(0.0)
        .roll_rule(RollRule::CdsImm)
        .stub(convention.stub)
        .attributes(Attributes::new())
        .build()
        .expect("tranche");
    tranche.upfront = Some((
        settlement,
        Money::new(
            tranche_notional * quote.upfront_pct,
            quote.convention.currency,
        )
        .expect("upfront cash"),
    ));
    tranche
}

fn quote(attachment: f64, detachment: f64, maturity: Date) -> CdsTrancheQuote {
    CdsTrancheQuote {
        id: QuoteId::new(format!("TRANCHE-{attachment}-{detachment}")),
        index: "CDX".to_string(),
        series: 40,
        attachment,
        detachment,
        maturity,
        upfront_pct: 0.0,
        coupon_bp: 100.0,
        convention: CdsConventionKey {
            currency: Currency::USD,
            doc_clause: CdsDocClause::IsdaNa,
        },
    }
}

/// Construct a par quote in settlement cash units from the independent tranche pricer.
fn par_quote(
    mut quote: CdsTrancheQuote,
    base_date: Date,
    notional: f64,
    market: &MarketContext,
) -> CdsTrancheQuote {
    // A nonzero upfront supplies the convention settlement date. Remove only
    // that cashflow before computing the price of the premium/protection legs.
    quote.upfront_pct = 1.0;
    let mut tranche = build_tranche(&quote, base_date, notional);
    let (settlement, _) = tranche.upfront.take().expect("settlement cashflow");
    let df = market
        .get_discount("USD-OIS")
        .expect("discount curve")
        .df_between_dates(base_date, settlement)
        .expect("settlement discount factor");
    let pv = CdsTranchePricer::new()
        .price_tranche(&tranche, market, base_date)
        .expect("price tranche legs")
        .amount();
    quote.upfront_pct = pv / (tranche.notional.amount() * df);
    quote
}

fn envelope(
    base_date: Date,
    notional: f64,
    quotes: &[CdsTrancheQuote],
    market: &MarketContext,
) -> CalibrationEnvelope {
    let market_quotes: Vec<MarketQuote> = quotes
        .iter()
        .cloned()
        .map(MarketQuote::CdsTranche)
        .collect();
    let (prior_market, mut market_data) = cal_utils::split_market_context(market);
    cal_utils::extend_market_data(&mut market_data, &market_quotes);
    CalibrationEnvelope {
        schema_url: None,
        schema: CalibrationSchema::CURRENT,
        plan: CalibrationPlan {
            id: "base-correlation-regression".to_string(),
            description: None,
            quote_sets: [(
                "tranches".to_string(),
                cal_utils::quote_set_ids(&market_quotes),
            )]
            .into_iter()
            .collect(),
            settings: CalibrationConfig {
                solver: finstack_quant_calibration::SolverConfig::default()
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
                    discount_curve_id: "USD-OIS".into(),
                    currency: Currency::USD,
                    notional,
                    frequency: None,
                    day_count: None,
                    business_day_convention: None,
                    calendar_id: None,
                    detachment_points: quotes.iter().map(|q| q.detachment).collect(),
                    roll_rule: RollRule::CdsImm,
                }),
            }],
        },
        market_data,
        prior_market,
    }
}

#[test]
fn base_correlation_step_builds_curve_and_reprices_settlement_quoted_trades() {
    // March 21 is just after the semiannual roll. The standard 5Y quote
    // matures June 20, 2030, 91 days beyond the old anniversary-based check.
    // Prior frozen inputs used a legacy March maturity and omitted the
    // settlement discount factor; use a consistent synthetic trade instead.
    let base_date = Date::from_calendar_date(2025, Month::March, 21).expect("base date");
    let maturity = Date::from_calendar_date(2030, Month::June, 20).expect("maturity");
    let notional = 1_000_000.0;
    let target_market = create_market(base_date, [0.25, 0.35]);
    let quotes: Vec<CdsTrancheQuote> = [(0.0, 0.03), (0.03, 0.07)]
        .into_iter()
        .map(|(attachment, detachment)| {
            par_quote(
                quote(attachment, detachment, maturity),
                base_date,
                notional,
                &target_market,
            )
        })
        .collect();
    let source_market = create_market(base_date, [0.10, 0.15]);
    let result = engine::execute(&envelope(base_date, notional, &quotes, &source_market))
        .expect("calibrate standard 5Y tranches");
    assert!(result.result.report.success);
    let step = result.result.step_reports.get("corr").expect("step report");
    assert!(step.success);
    assert!(step.max_residual < 1e-8, "residual: {}", step.max_residual);

    let market = MarketContext::try_from(result.result.final_market).expect("restore context");
    let curve = market
        .get_base_correlation("CDX_CORR")
        .expect("calibrated curve");
    assert!(curve.validate_shape().is_monotonic);
    for (actual, expected) in curve.correlations().iter().zip([0.25, 0.35]) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "rho={actual}, expected={expected}"
        );
    }
    let index = market.get_credit_index("CDX").expect("credit index");
    assert_eq!(index.base_correlation_curve.id().as_str(), "CDX_CORR");

    for quote in &quotes {
        let tranche = build_tranche(quote, base_date, notional);
        let (settlement, upfront) = tranche.upfront.expect("quoted upfront");
        assert!(settlement > base_date);
        let df = market
            .get_discount("USD-OIS")
            .expect("discount curve")
            .df_between_dates(base_date, settlement)
            .expect("discount factor");
        let omitted_discounting_error =
            upfront.amount().abs() * (1.0 - df) / tranche.notional.amount();
        assert!(
            omitted_discounting_error > 1e-6,
            "fixture must detect undiscounted upfront"
        );
        let npv = CdsTranchePricer::new()
            .price_tranche(&tranche, &market, base_date)
            .expect("reprice complete quoted trade")
            .amount();
        assert!(
            (npv / tranche.notional.amount()).abs() < 1e-8,
            "complete quoted trade did not reprice: NPV={npv}"
        );
    }
}

#[test]
fn base_correlation_rejects_thin_tranche_mispricing_above_ten_basis_points() {
    let base_date = Date::from_calendar_date(2025, Month::March, 21).expect("base date");
    let maturity = Date::from_calendar_date(2030, Month::June, 20).expect("maturity");
    let notional = 1_000_000.0;
    // Equity-tranche upfront is maximal at zero correlation. Add 170 bp of
    // tranche-notional upfront beyond that bound: portfolio scaling reduced
    // this to 5.1 bp, incorrectly accepting it under the 10 bp fit tolerance.
    let market = create_market(base_date, [0.0, 0.0]);
    let mut unreachable = par_quote(quote(0.0, 0.03, maturity), base_date, notional, &market);
    unreachable.upfront_pct += 0.017;
    let err = engine::execute(&envelope(base_date, notional, &[unreachable], &market))
        .expect_err("170 bp error on a 3% tranche must fail calibration");
    assert!(err.to_string().contains("exceeds tolerance"), "{err}");
}
