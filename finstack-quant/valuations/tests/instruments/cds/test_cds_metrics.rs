//! CDS metrics calculation tests.
//!
//! Comprehensive tests for all CDS metrics including CS01, DV01,
//! expected loss, jump-to-default, par spread, and risk sensitivities.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, DiscountCurveRateCalibration, DiscountCurveRateQuote,
    DiscountCurveRateQuoteType, HazardCurve,
};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, IndexId};
use finstack_quant_valuations::calibration::api::schema::DiscountCurveParams;
use finstack_quant_valuations::calibration::bumps::{
    bump_discount_curve, bump_hazard_spreads, BumpRequest,
};
use finstack_quant_valuations::calibration::{CalibrationMethod, RatesStepConventions};
use finstack_quant_valuations::instruments::credit_derivatives::cds::{
    CdsValuationConvention, CreditDefaultSwap,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::market::conventions::ids::CdsDocClause;
use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
use finstack_quant_valuations::market::quotes::rates::RateQuote;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;
use time::Duration;

fn sum_bucketed_cs01(result: &finstack_quant_valuations::results::ValuationResult) -> f64 {
    result
        .measures
        .iter()
        .filter(|(id, _)| id.as_str().starts_with("bucketed_cs01::"))
        .map(|(_, v)| *v)
        .sum()
}

fn sum_bucketed_dv01(result: &finstack_quant_valuations::results::ValuationResult) -> f64 {
    result
        .measures
        .iter()
        .filter(|(id, _)| id.as_str().starts_with("bucketed_dv01::"))
        .map(|(_, v)| *v)
        .sum()
}

fn build_test_discount(rate: f64, base: Date, id: &str) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

fn build_quote_calibrated_discount(rate: f64, base: Date, id: &str) -> DiscountCurve {
    build_test_discount(rate, base, id)
        .to_builder_with_id(id)
        .rate_calibration(DiscountCurveRateCalibration {
            index_id: "USD-SOFR-1M".to_string(),
            currency: Currency::USD,
            quotes: vec![
                DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Deposit,
                    tenor: "1Y".to_string(),
                    rate,
                },
                DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Deposit,
                    tenor: "5Y".to_string(),
                    rate,
                },
                DiscountCurveRateQuote {
                    quote_type: DiscountCurveRateQuoteType::Deposit,
                    tenor: "10Y".to_string(),
                    rate,
                },
            ],
        })
        .build()
        .unwrap()
}

fn bump_quote_calibrated_discount(
    curve: &DiscountCurve,
    calibration: &DiscountCurveRateCalibration,
    market: &MarketContext,
    bump_bp: f64,
) -> DiscountCurve {
    let index = IndexId::new(calibration.index_id.as_str());
    let quotes: Vec<RateQuote> = calibration
        .quotes
        .iter()
        .map(|quote| RateQuote::Deposit {
            id: QuoteId::new(format!("{}-{}", curve.id(), quote.tenor)),
            index: index.clone(),
            pillar: Pillar::Tenor(quote.tenor.parse().unwrap()),
            rate: quote.rate,
        })
        .collect();
    let first_rate = calibration
        .quotes
        .first()
        .map(|quote| quote.rate)
        .unwrap_or(0.0);
    let fixings = ScalarTimeSeries::new(
        format!("FIXING:{}", curve.id()),
        vec![
            (curve.base_date() - Duration::days(3), first_rate),
            (curve.base_date() - Duration::days(2), first_rate),
            (curve.base_date() - Duration::days(1), first_rate),
            (curve.base_date(), first_rate),
        ],
        None,
    )
    .unwrap();
    let params = DiscountCurveParams {
        curve_id: curve.id().clone(),
        currency: calibration.currency,
        base_date: curve.base_date(),
        method: CalibrationMethod::Bootstrap,
        interpolation: curve.interp_style(),
        extrapolation: curve.extrapolation(),
        pricing_discount_id: None,
        pricing_forward_id: None,
        conventions: RatesStepConventions {
            ois_compounding: None,
            curve_day_count: Some(curve.day_count()),
        },
    };
    // Mirror the production delta-overlay semantics: re-bootstrap both the
    // bumped and the unbumped quote sets and apply only their df ratio to the
    // stored curve, preserving the stored base shape.
    let ctx = market.clone().insert_series(fixings);
    let bumped =
        bump_discount_curve(&quotes, &params, &ctx, &BumpRequest::Parallel(bump_bp)).unwrap();
    let unbumped =
        bump_discount_curve(&quotes, &params, &ctx, &BumpRequest::Parallel(0.0)).unwrap();
    let overlaid: Vec<(f64, f64)> = curve
        .knots()
        .iter()
        .zip(curve.dfs())
        .map(|(&t, &df)| (t, df * bumped.df(t) / unbumped.df(t)))
        .collect();
    DiscountCurve::builder(curve.id().clone())
        .base_date(curve.base_date())
        .day_count(curve.day_count())
        .knots(overlaid)
        .interp(curve.interp_style())
        .extrapolation(curve.extrapolation())
        .rate_calibration(calibration.clone())
        .build()
        .unwrap()
}

fn build_test_hazard(hz: f64, rec: f64, base: Date, id: &str) -> HazardCurve {
    HazardCurve::builder(id)
        .base_date(base)
        .recovery_rate(rec)
        .knots([(0.0, hz), (1.0, hz), (5.0, hz), (10.0, hz)])
        .build()
        .unwrap()
}

fn create_test_market(as_of: Date) -> MarketContext {
    MarketContext::new()
        .insert(build_test_discount(0.05, as_of, "USD_OIS"))
        .insert(build_test_hazard(0.015, 0.40, as_of, "CORP"))
}

fn create_test_cds(as_of: Date, maturity: Date) -> CreditDefaultSwap {
    crate::finstack_quant_test_utils::cds_buy_protection(
        "METRICS_TEST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        maturity,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed")
}

#[test]
fn test_cs01_positive_for_buyer() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cs01 = *result.measures.get("cs01").unwrap();

    assert!(cs01 > 0.0, "CS01 should be positive for protection buyer");
    // For $10M 5Y CDS with 1.5% hazard rate, CS01 ≈ notional × (1-rec) × annuity × 1bp ≈ $2,700
    // Upper bound of $10,000 is ~4x expected, a generous but meaningful sanity check
    assert!(
        cs01 < 10_000.0,
        "CS01 should be reasonable for $10M 5Y CDS: {}",
        cs01
    );
}

#[test]
fn test_cs01_hazard_vs_risky_pv01_consistency() {
    // Validate CS01 equals a direct finite-difference bump of the hazard curve.
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);

    // Ensure hazard builder used in market has explicit base/daycount/recovery.
    // Use a hazard curve WITHOUT par points so GenericParallelCs01 uses a model hazard shift.
    let market = {
        let mut ctx = MarketContext::new();
        let disc = DiscountCurve::builder("USD_OIS")
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .knots([(0.0, 1.0), (10.0, (-(0.05_f64 * 10.0_f64)).exp())])
            .build()
            .unwrap();
        let hazard = HazardCurve::builder(cds.protection.credit_curve_id.clone())
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .recovery_rate(0.4)
            .knots([(0.0, 0.01), (5.0, 0.012), (10.0, 0.013)])
            .build()
            .unwrap();
        ctx = ctx.insert(disc).insert(hazard);
        ctx
    };

    // Use value_raw for high-precision comparison (matches how CS01 metric is now computed)
    use finstack_quant_valuations::instruments::Instrument;
    // Manually compute the same central finite-difference CS01 definition used by the metric.
    use finstack_quant_valuations::calibration::bumps::{bump_hazard_shift, BumpRequest};
    let hazard = market
        .get_hazard(cds.protection.credit_curve_id.as_str())
        .unwrap();
    let bumped_up = bump_hazard_shift(hazard.as_ref(), &BumpRequest::Parallel(1.0)).unwrap();
    let bumped_down = bump_hazard_shift(hazard.as_ref(), &BumpRequest::Parallel(-1.0)).unwrap();
    let pv_up = cds
        .value_raw(&market.clone().insert(bumped_up), as_of)
        .unwrap();
    let pv_down = cds
        .value_raw(&market.clone().insert(bumped_down), as_of)
        .unwrap();
    let expected_cs01 = (pv_up - pv_down) / 2.0; // per 1bp central difference

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let cs01 = *result.measures.get("cs01").unwrap();

    assert!(cs01 > 0.0, "CS01 should be positive");

    let tol = 1e-6_f64.max(1e-8 * expected_cs01.abs());
    assert!(
        (cs01 - expected_cs01).abs() <= tol,
        "CS01 should match direct finite-difference bump: metric={}, expected={}, diff={}, tol={}",
        cs01,
        expected_cs01,
        (cs01 - expected_cs01).abs(),
        tol
    );
}

#[test]
#[ignore = "slow: covered by mise rust-test-slow"]
fn test_bucketed_cs01_reconciles_with_parallel_under_cds_convention() {
    let as_of = date!(2026 - 03 - 20);
    let maturity = date!(2031 - 06 - 20);
    let discount_id = CurveId::new("USD_OIS");
    let hazard_id = CurveId::new("CORP");

    let mut cds = create_test_cds(as_of, maturity);
    cds.valuation_convention = CdsValuationConvention::IsdaDirty;
    cds.doc_clause = Some(CdsDocClause::IsdaNa);

    let discount = build_test_discount(0.035, as_of, discount_id.as_str());
    let hazard = HazardCurve::builder(hazard_id)
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([
            (0.0, 0.0060),
            (1.0, 0.0080),
            (3.0, 0.0120),
            (5.0, 0.0180),
            (7.0, 0.0200),
        ])
        .par_spreads([(1.0, 50.0), (3.0, 80.0), (5.0, 120.0), (7.0, 150.0)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(discount).insert(hazard);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01, MetricId::BucketedCs01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let parallel = *result.measures.get("cs01").unwrap();
    let bucket_total = *result.measures.get("bucketed_cs01").unwrap();
    let bucket_sum = sum_bucketed_cs01(&result);

    let total_tol = 1e-6_f64.max(1e-10 * bucket_total.abs());
    assert!(
        (bucket_sum - bucket_total).abs() <= total_tol,
        "Bucketed CS01 stored total should equal bucket sum: total={bucket_total}, sum={bucket_sum}"
    );

    let parallel_tol = 1e-4_f64.max(2e-2 * parallel.abs());
    assert!(
        (bucket_total - parallel).abs() <= parallel_tol,
        "CDS bucketed CS01 should use the same doc clause and valuation convention as parallel CS01: bucketed={bucket_total}, parallel={parallel}, diff={}, tol={parallel_tol}",
        (bucket_total - parallel).abs()
    );
}

#[test]
fn test_risky_pv01_positive() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::RiskyPv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let risky_pv01 = *result.measures.get("risky_pv01").unwrap();

    assert!(risky_pv01 > 0.0, "Risky PV01 should be positive");

    // For $10MM, 5Y CDS, risky PV01 should be in reasonable range
    assert!(
        risky_pv01 > 1_000.0 && risky_pv01 < 100_000.0,
        "Risky PV01={:.2} outside expected range",
        risky_pv01
    );
}

#[test]
fn test_par_spread_metric() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let par_spread = *result.measures.get("par_spread").unwrap();

    assert!(par_spread > 0.0, "Par spread should be positive");
    assert!(par_spread.is_finite(), "Par spread should be finite");

    // Reasonable range for investment grade
    assert!(
        par_spread > 10.0 && par_spread < 500.0,
        "Par spread={:.2} bps outside typical IG range",
        par_spread
    );
}

#[test]
fn test_cds_par_spread_metric_does_not_return_quoted_spread_override() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let mut quoted_cds = cds.clone();
    quoted_cds.pricing_overrides.market_quotes.cds_quote_bp = Some(999.0);
    let market = create_test_market(as_of);

    let base = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let quoted = quoted_cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let base_par_spread = *base.measures.get("par_spread").unwrap();
    let quoted_par_spread = *quoted.measures.get("par_spread").unwrap();
    assert_ne!(
        quoted_par_spread, 999.0,
        "par_spread must be computed by the Finstack Quant pricer, not returned from source quote metadata"
    );
    assert_eq!(
        quoted_par_spread, base_par_spread,
        "quoted spread metadata should not alter the par_spread metric"
    );
}

#[test]
fn test_protection_leg_pv_metric() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ProtectionLegPv],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let prot_pv = *result.measures.get("protection_leg_pv").unwrap();

    assert!(prot_pv > 0.0, "Protection leg PV should be positive");
}

#[test]
fn test_premium_leg_pv_metric() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::PremiumLegPv],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let prem_pv = *result.measures.get("premium_leg_pv").unwrap();

    assert!(
        prem_pv > 0.0,
        "Premium leg PV should be positive for positive spread"
    );
}

#[test]
fn test_expected_loss_positive() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let expected_loss = *result.measures.get("expected_loss").unwrap();

    assert!(expected_loss > 0.0, "Expected loss should be positive");

    // Should be less than notional × LGD
    let max_loss = 10_000_000.0 * 0.6; // 60% LGD
    assert!(
        expected_loss < max_loss,
        "Expected loss should be less than max possible loss"
    );
}

#[test]
fn test_expected_loss_formula() {
    // EL = Notional × PD × LGD
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let hazard_rate = 0.02; // 2% per year
    let recovery = 0.40;

    let mut cds = create_test_cds(as_of, maturity);
    cds.protection.recovery_rate = recovery;

    let market = MarketContext::new()
        .insert(build_test_discount(0.05, as_of, "USD_OIS"))
        .insert(build_test_hazard(hazard_rate, recovery, as_of, "CORP"));

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let expected_loss = *result.measures.get("expected_loss").unwrap();

    // For 5Y: PD ≈ 1 - exp(-λ×T) ≈ 1 - exp(-0.02×5) ≈ 0.095
    // EL ≈ 10MM × 0.6 × 0.095 ≈ $570,000
    assert!(
        expected_loss > 400_000.0 && expected_loss < 800_000.0,
        "Expected loss={:.0} outside expected range",
        expected_loss
    );
}

#[test]
fn test_expected_loss_conditions_on_as_of() {
    // Expected loss should be conditional on survival to `as_of` (i.e. forward PD from as_of→maturity).
    let base = date!(2024 - 01 - 01);
    let as_of = date!(2026 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let hazard_rate = 0.02; // 2% per year
    let recovery = 0.40;
    let notional = 10_000_000.0;
    let lgd = 1.0 - recovery;

    let mut cds = create_test_cds(base, maturity);
    cds.protection.recovery_rate = recovery;

    // Curves can be based at `base` (safer for df_on_date_curve on older dates).
    let market = MarketContext::new()
        .insert(build_test_discount(0.05, base, "USD_OIS"))
        .insert(build_test_hazard(hazard_rate, recovery, base, "CORP"));

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let expected_loss = *result.measures.get("expected_loss").unwrap();

    // Forward PD from as_of to maturity (approx): 1 - exp(-λ * Δt)
    let dt_years = DayCount::Act365F
        .year_fraction(
            as_of,
            maturity,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();
    let pd_forward = 1.0 - (-hazard_rate * dt_years).exp();
    let expected = notional * lgd * pd_forward;

    let tol = 1e-6_f64.max(1e-6 * expected.abs());
    assert!(
        (expected_loss - expected).abs() <= tol,
        "ExpectedLoss should be conditional on as_of: got {}, expected {}, diff {}, tol {}",
        expected_loss,
        expected,
        (expected_loss - expected).abs(),
        tol
    );
}

#[test]
fn test_jump_to_default_positive_for_buyer() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let jtd = *result.measures.get("jump_to_default").unwrap();

    // Protection buyer gains on default
    assert!(jtd > 0.0, "JTD should be positive for protection buyer");
}

#[test]
fn test_jump_to_default_negative_for_seller() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = crate::finstack_quant_test_utils::cds_sell_protection(
        "JTD_SELLER",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        maturity,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let jtd = *result.measures.get("jump_to_default").unwrap();

    // Protection seller loses on default
    assert!(jtd < 0.0, "JTD should be negative for protection seller");
}

#[test]
fn test_jump_to_default_magnitude() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let mut cds = create_test_cds(as_of, maturity);
    cds.protection.recovery_rate = 0.40;

    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let jtd = *result.measures.get("jump_to_default").unwrap();

    // JTD ≈ Notional × LGD = $10MM × 0.6 = $6MM
    assert!(
        jtd > 5_500_000.0 && jtd < 6_500_000.0,
        "JTD={:.0} should be approximately $6MM",
        jtd
    );
}

#[test]
fn test_jump_to_default_uses_adjusted_coupon_schedule_for_accrued() {
    // Pick a quarter IMM date that falls on a weekend (2027-03-20 is Saturday),
    // so the schedule should adjust to the next business day under Modified Following.
    let as_of = date!(2027 - 04 - 01);
    let maturity = date!(2031 - 12 - 20);

    let notional = Money::new(10_000_000.0, Currency::USD);
    let mut cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "JTD_SCHEDULE_ADJ",
        notional,
        100.0,
        date!(2026 - 12 - 20),
        maturity,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");
    cds.protection.recovery_rate = 0.40;

    // Market is required to compute base_value for the valuation result, even though JTD itself
    // is curve-independent.
    let market = create_test_market(as_of);
    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let jtd = *result.measures.get("jump_to_default").unwrap();

    // Expected last coupon is adjusted from 2027-03-20 (Sat) to 2027-03-22 (Mon)
    let last_coupon = date!(2027 - 03 - 22);
    let accrual_fraction = cds
        .premium
        .day_count
        .year_fraction(
            last_coupon,
            as_of,
            finstack_quant_core::dates::DayCountContext::default(),
        )
        .unwrap();
    let spread_decimal = 100.0 / 10_000.0;
    let accrued = notional.amount() * spread_decimal * accrual_fraction;

    let lgd = 1.0 - cds.protection.recovery_rate;
    let expected = notional.amount() * lgd - accrued;

    let tol = 1e-6_f64.max(1e-6 * expected.abs());
    assert!(
        (jtd - expected).abs() <= tol,
        "JTD should use adjusted coupon schedule for accrued premium: got {}, expected {}, diff {}, tol {}",
        jtd,
        expected,
        (jtd - expected).abs(),
        tol
    );
}

// Note: DefaultProbability metric is not currently implemented for CDS
// The probability can be derived from the hazard curve directly if needed

#[test]
fn test_dv01_metric() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let dv01 = *result.measures.get("dv01").unwrap();

    // DV01 = PV(rate+1bp) - PV(base); sign depends on instrument structure
    assert!(dv01.is_finite(), "DV01 should be finite");
    assert!(dv01.abs() > 0.0, "DV01 magnitude should be non-zero");
}

#[test]
fn test_cds_dv01_recalibrates_par_spread_hazard_curve() {
    let as_of = date!(2024 - 03 - 20);
    let maturity = date!(2029 - 03 - 20);
    let discount_id = CurveId::new("USD_OIS");
    let hazard_id = CurveId::new("CORP");

    let cds = create_test_cds(as_of, maturity);
    let discount = build_test_discount(0.04, as_of, discount_id.as_str());
    let hazard = HazardCurve::builder(hazard_id.clone())
        .base_date(as_of)
        .day_count(DayCount::Act365F)
        .recovery_rate(0.4)
        .knots([
            (0.0, 0.0060),
            (1.0, 0.0080),
            (3.0, 0.0120),
            (5.0, 0.0180),
            (7.0, 0.0200),
        ])
        .par_spreads([(1.0, 50.0), (3.0, 80.0), (5.0, 120.0), (7.0, 150.0)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(discount).insert(hazard);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    let bumped_pv = |bump_bp: f64| {
        let mut bumped_market = market.clone();
        bumped_market
            .apply_curve_bump_in_place(&discount_id, BumpSpec::parallel_bp(bump_bp))
            .unwrap();
        let base_hazard = market.get_hazard(hazard_id.as_str()).unwrap();
        let recalibrated = bump_hazard_spreads(
            base_hazard.as_ref(),
            &bumped_market,
            &BumpRequest::Parallel(0.0),
            Some(&discount_id),
        )
        .unwrap();
        cds.value_raw(&bumped_market.insert(recalibrated), as_of)
            .unwrap()
    };
    let expected = (bumped_pv(1.0) - bumped_pv(-1.0)) / 2.0;

    let tol = 1e-6_f64.max(1e-8 * expected.abs());
    assert!(
        (dv01 - expected).abs() <= tol,
        "CDS DV01 should rebootstrap par-spread hazard curves under rate bumps: metric={dv01}, expected={expected}, diff={}, tol={tol}",
        (dv01 - expected).abs()
    );
}

#[test]
fn test_cds_dv01_uses_discount_quote_bump_when_calibration_exists() {
    let as_of = date!(2024 - 03 - 20);
    let maturity = date!(2029 - 03 - 20);
    let discount_id = CurveId::new("USD_OIS");
    let hazard_id = CurveId::new("CORP");

    let cds = create_test_cds(as_of, maturity);
    let discount = build_quote_calibrated_discount(0.04, as_of, discount_id.as_str());
    let hazard = build_test_hazard(0.015, 0.4, as_of, hazard_id.as_str());
    let market = MarketContext::new().insert(discount).insert(hazard);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01 = *result.measures.get("dv01").unwrap();

    let bumped_pv = |bump_bp: f64| {
        let base_discount = market.get_discount(discount_id.as_str()).unwrap();
        let calibration = base_discount.rate_calibration().unwrap();
        let bumped_discount =
            bump_quote_calibrated_discount(base_discount.as_ref(), calibration, &market, bump_bp);
        cds.value_raw(&market.clone().insert(bumped_discount), as_of)
            .unwrap()
    };
    let expected = (bumped_pv(1.0) - bumped_pv(-1.0)) / 2.0;

    let tol = 1e-6_f64.max(1e-8 * expected.abs());
    assert!(
        (dv01 - expected).abs() <= tol,
        "CDS DV01 should bump stored discount calibration quotes before repricing: metric={dv01}, expected={expected}, diff={}, tol={tol}",
        (dv01 - expected).abs()
    );
}

// `test_cdsw_clean_value_excludes_accrued_premium_from_dirty_value` was removed
// when the `cds_clean_price` and `cds_adjust_premium_accrual_dates` override
// flags were folded into `CdsValuationConvention`. Its purpose was to isolate
// the accrued-premium add-back from the schedule-adjustment difference by
// flipping clean/dirty independently — a control surface that no longer exists.
// `test_cdsw_clean_value_uses_first_class_valuation_convention` (below) and the
// CDSW convention tests around it cover the end-to-end clean/dirty behaviour.

#[test]
fn test_cdsw_clean_value_uses_first_class_valuation_convention() {
    let as_of = date!(2026 - 05 - 02);
    let maturity = date!(2031 - 06 - 20);
    let dirty_cds = create_test_cds(date!(2026 - 03 - 20), maturity);

    let mut encoded = serde_json::to_value(&dirty_cds).unwrap();
    encoded.as_object_mut().unwrap().insert(
        "valuation_convention".to_string(),
        "bloomberg_cdsw_clean".into(),
    );
    let clean_cds: CreditDefaultSwap = serde_json::from_value(encoded).unwrap();

    let market = MarketContext::new()
        .insert(build_test_discount(0.035, as_of, "USD_OIS"))
        .insert(build_test_hazard(0.010, 0.40, as_of, "CORP"));

    let mut direct_clean_cds = dirty_cds;
    direct_clean_cds.valuation_convention =
        finstack_quant_valuations::instruments::credit_derivatives::cds::CdsValuationConvention::BloombergCdswClean;

    let clean_value = clean_cds.value_raw(&market, as_of).unwrap();
    let expected_clean = direct_clean_cds.value_raw(&market, as_of).unwrap();
    let tol = 1e-6_f64.max(1e-10 * expected_clean.abs());
    assert!(
        (clean_value - expected_clean).abs() <= tol,
        "Bloomberg CDSW convention should produce clean principal without pricing overrides: clean={clean_value}, expected={expected_clean}, diff={}, tol={tol}",
        (clean_value - expected_clean).abs()
    );
}

#[test]
fn test_cdsw_convention_values_premium_leg_with_adjusted_cashflow_accrual_dates() {
    let as_of = date!(2026 - 03 - 20);
    let maturity = date!(2026 - 09 - 20);
    let mut cds = create_test_cds(as_of, maturity);
    cds.valuation_convention =
        finstack_quant_valuations::instruments::credit_derivatives::cds::CdsValuationConvention::BloombergCdswClean;

    let market = MarketContext::new()
        .insert(build_test_discount(0.0, as_of, "USD_OIS"))
        .insert(build_test_hazard(0.0, 0.40, as_of, "CORP"));

    let value = cds.value_raw(&market, as_of).unwrap();
    let expected_premium: f64 = 10_000_000.0 * 0.01 * ((94.0 + 91.0) / 360.0);
    let expected_value: f64 = -expected_premium;
    let tol = 1e-6_f64.max(1e-10 * expected_value.abs());
    assert!(
        (value - expected_value).abs() <= tol,
        "Bloomberg CDSW cashflows accrue between adjusted cashflow dates: value={value}, expected={expected_value}, diff={}, tol={tol}",
        (value - expected_value).abs()
    );
}

#[test]
fn test_cdsw_par_spread_metric_uses_full_premium_denominator_when_requested() {
    let as_of = date!(2026 - 05 - 02);
    let maturity = date!(2031 - 06 - 20);
    let mut cds = create_test_cds(date!(2026 - 03 - 20), maturity);
    cds.protection_effective_date = Some(as_of);

    let mut cdsw_cds = cds.clone();
    cdsw_cds.valuation_convention =
        finstack_quant_valuations::instruments::credit_derivatives::cds::CdsValuationConvention::BloombergCdswCleanFullPremium;

    let market = MarketContext::new()
        .insert(build_test_discount(0.035, as_of, "USD_OIS"))
        .insert(
            HazardCurve::builder("CORP")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .recovery_rate(0.4)
                .knots([(0.5, 0.01), (3.0, 0.012), (5.0, 0.014), (7.0, 0.015)])
                .build()
                .unwrap(),
        );

    let standard = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let cdsw = cdsw_cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::ParSpread],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let standard_spread = *standard.measures.get("par_spread").unwrap();
    let cdsw_spread = *cdsw.measures.get("par_spread").unwrap();
    assert!(
        cdsw_spread < standard_spread,
        "Including accrual-on-default in the par-spread denominator should lower par spread: cdsw={cdsw_spread}, standard={standard_spread}"
    );
}

#[test]
fn test_theta_metric() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let theta = *result.measures.get("theta").unwrap();

    assert!(theta.is_finite(), "Theta should be finite");
}

#[test]
fn test_hazard_cs01_metric() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::Cs01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let cs01 = *result.measures.get("cs01").unwrap();

    assert!(cs01 > 0.0, "CS01 should be positive");
    assert!(cs01.is_finite(), "CS01 should be finite");
}

#[test]
fn test_multiple_metrics_simultaneously() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let metrics = vec![
        MetricId::Cs01,
        MetricId::RiskyPv01,
        MetricId::ParSpread,
        MetricId::ProtectionLegPv,
        MetricId::PremiumLegPv,
        MetricId::ExpectedLoss,
        MetricId::JumpToDefault,
        MetricId::Dv01,
        MetricId::Theta,
        MetricId::Cs01,
    ];

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // All metrics should be present
    assert!(result.measures.contains_key("cs01"));
    assert!(result.measures.contains_key("risky_pv01"));
    assert!(result.measures.contains_key("par_spread"));
    assert!(result.measures.contains_key("protection_leg_pv"));
    assert!(result.measures.contains_key("premium_leg_pv"));
    assert!(result.measures.contains_key("expected_loss"));
    assert!(result.measures.contains_key("jump_to_default"));
    assert!(result.measures.contains_key("dv01"));
    assert!(result.measures.contains_key("theta"));
}

#[test]
fn test_risky_pv01_computable() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::RiskyPv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let risky_pv01 = *result.measures.get("risky_pv01").unwrap();
    assert!(risky_pv01.abs() > 0.0, "risky_pv01 should be non-zero");
}

#[test]
fn test_metrics_scale_with_notional() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds_small = crate::finstack_quant_test_utils::cds_buy_protection(
        "SMALL",
        Money::new(1_000_000.0, Currency::USD),
        100.0,
        as_of,
        maturity,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let cds_large = crate::finstack_quant_test_utils::cds_buy_protection(
        "LARGE",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        maturity,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let market = create_test_market(as_of);

    let metrics = vec![
        MetricId::RiskyPv01,
        MetricId::ExpectedLoss,
        MetricId::JumpToDefault,
    ];

    let result_small = cds_small
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_large = cds_large
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    for metric in ["risky_pv01", "expected_loss", "jump_to_default"] {
        let val_small = *result_small.measures.get(metric).unwrap();
        let val_large = *result_large.measures.get(metric).unwrap();
        let ratio = val_large / val_small;

        assert!(
            (ratio - 10.0).abs() < 0.1,
            "{} should scale with notional, got ratio {}",
            metric,
            ratio
        );
    }
}

#[test]
fn test_cs01_increases_with_tenor() {
    let as_of = date!(2024 - 01 - 01);
    let market = create_test_market(as_of);

    let mut cs01_values = Vec::new();

    for years in [1, 3, 5, 10] {
        let maturity = Date::from_calendar_date(2024 + years, time::Month::January, 1).unwrap();
        let cds = create_test_cds(as_of, maturity);

        let result = cds
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::Cs01],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let cs01 = *result.measures.get("cs01").unwrap();
        cs01_values.push((years, cs01));
    }

    // CS01 should generally increase with tenor
    for i in 1..cs01_values.len() {
        assert!(
            cs01_values[i].1 > cs01_values[i - 1].1,
            "CS01 should increase with tenor: {}Y={:.2} <= {}Y={:.2}",
            cs01_values[i - 1].0,
            cs01_values[i - 1].1,
            cs01_values[i].0,
            cs01_values[i].1
        );
    }
}

#[test]
fn test_expected_loss_increases_with_tenor() {
    let as_of = date!(2024 - 01 - 01);
    let market = create_test_market(as_of);

    let mut el_values = Vec::new();

    for years in [1, 3, 5, 10] {
        let maturity = Date::from_calendar_date(2024 + years, time::Month::January, 1).unwrap();
        let cds = create_test_cds(as_of, maturity);

        let result = cds
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::ExpectedLoss],
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
            .unwrap();

        let el = *result.measures.get("expected_loss").unwrap();
        el_values.push((years, el));
    }

    // Expected loss should increase with tenor (more time for default)
    for i in 1..el_values.len() {
        assert!(
            el_values[i].1 > el_values[i - 1].1,
            "Expected loss should increase with tenor"
        );
    }
}

#[test]
fn test_bucketed_dv01_metric() {
    let as_of = date!(2024 - 01 - 01);
    let maturity = date!(2029 - 01 - 01);

    let cds = create_test_cds(as_of, maturity);
    let market = create_test_market(as_of);

    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::BucketedDv01, MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let bucket_total = *result.measures.get("bucketed_dv01").unwrap();
    let bucket_sum = sum_bucketed_dv01(&result);
    let diff = (bucket_sum - bucket_total).abs();
    let tol = 1e-6_f64.max(1e-6 * bucket_total.abs());
    assert!(
        diff < tol,
        "Sum of bucketed DV01 should match bucketed total: sum={}, total={}, diff={}",
        bucket_sum,
        bucket_total,
        diff
    );

    // Sanity: bucketed total should be finite and DV01 should be finite.
    let dv01 = *result.measures.get("dv01").unwrap();
    assert!(dv01.is_finite(), "DV01 should be finite");
    assert!(
        bucket_total.is_finite(),
        "Bucketed DV01 total should be finite"
    );
}
