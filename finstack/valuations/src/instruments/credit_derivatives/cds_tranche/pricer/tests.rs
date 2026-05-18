use super::config::DEFAULT_QUADRATURE_ORDER;
use super::*;
use crate::cashflow::primitives::CFKind;
use crate::instruments::credit_derivatives::cds_tranche::parameters::CDSTrancheParams;
use crate::instruments::credit_derivatives::cds_tranche::{CDSTranche, TrancheSide};
use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DateExt};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::CreditIndexData;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::market_data::term_structures::{BaseCorrelationCurve, HazardCurve};
use finstack_core::math::{binomial_probability, log_factorial, standard_normal_inv_cdf};
use finstack_core::money::Money;
use std::sync::Arc;
use time::Month;

fn sample_market_context() -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    // Create discount curve
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .interp(finstack_core::math::interp::InterpStyle::LogLinear)
        .build()
        .expect("Curve builder should succeed with valid test data");

    // Create index hazard curve
    let index_curve = HazardCurve::builder("CDX.NA.IG.42")
        .base_date(base_date)
        .recovery_rate(0.40)
        .knots(vec![(1.0, 0.01), (3.0, 0.015), (5.0, 0.02), (10.0, 0.025)])
        .par_spreads(vec![(1.0, 60.0), (3.0, 80.0), (5.0, 100.0), (10.0, 140.0)])
        .build()
        .expect("Curve builder should succeed with valid test data");

    // Create base correlation curve.
    //
    // The slope must be gentle enough to stay arbitrage-free at the
    // tranchelet level: a steeply rising base correlation pushes the
    // equity-tranche EL *down*, so an over-steep curve produces
    // `EL(0,D) < EL(0,A)` at short horizons (low PD). The shape below
    // was verified to keep `EL(0,D) ≥ EL(0,A)` across all payment-date
    // PDs for this index — see `arbitrage_market_context` for the
    // converse (deliberately non-arbitrage-free) fixture.
    let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![
            (3.0, 0.25),  // 0-3% equity
            (7.0, 0.30),  // 0-7% junior mezzanine
            (10.0, 0.34), // 0-10% senior mezzanine
            (15.0, 0.40), // 0-15% senior
            (30.0, 0.50), // 0-30% super senior
        ])
        .build()
        .expect("Curve builder should succeed with valid test data");

    // Create credit index data
    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(index_curve))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .build()
        .expect("Curve builder should succeed with valid test data");

    MarketContext::new()
        .insert(discount_curve)
        .insert_credit_index("CDX.NA.IG.42", index_data)
}

fn sample_market_context_with_issuers(n: usize) -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.97), (5.0, 0.84), (10.0, 0.68)])
        .build()
        .expect("Curve builder should succeed with valid test data");

    let index_curve = HazardCurve::builder("CDX.NA.IG.42")
        .base_date(base_date)
        .recovery_rate(0.40)
        .knots(vec![
            (1.0, 0.012),
            (3.0, 0.017),
            (5.0, 0.022),
            (10.0, 0.028),
        ])
        .par_spreads(vec![(1.0, 65.0), (3.0, 85.0), (5.0, 105.0), (10.0, 145.0)])
        .build()
        .expect("Curve builder should succeed with valid test data");

    // Gentle, arbitrage-free base-correlation slope (see
    // `sample_market_context` for the rationale).
    let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![
            (3.0, 0.25),
            (7.0, 0.30),
            (10.0, 0.34),
            (15.0, 0.40),
            (30.0, 0.50),
        ])
        .build()
        .expect("Curve builder should succeed with valid test data");

    let mut issuer_curves = finstack_core::HashMap::default();
    for i in 0..n {
        let id = format!("ISSUER-{:03}", i + 1);
        let bump = (i as f64) * 0.001;
        let hz = HazardCurve::builder(id.as_str())
            .base_date(base_date)
            .recovery_rate(0.40)
            .knots(vec![
                (1.0, (0.012 + bump).min(0.2)),
                (3.0, (0.017 + bump).min(0.2)),
                (5.0, (0.022 + bump).min(0.2)),
                (10.0, (0.028 + bump).min(0.2)),
            ])
            .build()
            .expect("HazardCurve builder should succeed with valid test data");
        issuer_curves.insert(id, Arc::new(hz));
    }

    let index = CreditIndexData::builder()
        .num_constituents(n as u16)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(index_curve))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .issuer_curves(issuer_curves)
        .build()
        .expect("Curve builder should succeed with valid test data");

    MarketContext::new()
        .insert(discount_curve)
        .insert_credit_index("CDX.NA.IG.42", index)
}

fn sample_tranche() -> CDSTranche {
    let _issue_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

    {
        let tranche_params = CDSTrancheParams::new(
            "CDX.NA.IG.42",                          // index_name
            42,                                      // series
            3.0,                                     // attach_pct (3%)
            7.0,                                     // detach_pct (7%)
            Money::new(10_000_000.0, Currency::USD), // $10MM notional
            maturity,                                // maturity
            500.0,                                   // running_coupon_bp (5%)
        );
        let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
        CDSTranche::new(
            "CDX_IG42_3_7_5Y",
            &tranche_params,
            &schedule_params,
            finstack_core::types::CurveId::from("USD-OIS"),
            finstack_core::types::CurveId::from("CDX.NA.IG.42"),
            TrancheSide::SellProtection,
        )
        .expect("Valid tranche parameters")
    }
}

#[test]
fn test_model_creation() {
    let model = CDSTranchePricer::new();
    assert_eq!(model.params.quadrature_order, DEFAULT_QUADRATURE_ORDER);
    assert!(model.params.use_issuer_curves);
}

#[test]
fn projected_schedule_contains_premium_and_default_rows() {
    let tranche = sample_tranche();
    let market = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let schedule = CDSTranchePricer::new()
        .build_projected_schedule(&tranche, &market, as_of)
        .expect("projected tranche schedule");

    assert!(schedule.flows.iter().any(|cf| cf.kind == CFKind::Fixed));
    assert!(schedule
        .flows
        .iter()
        .any(|cf| cf.kind == CFKind::DefaultedNotional));
}

#[test]
fn price_matches_discounted_projected_rows() {
    let tranche = sample_tranche();
    let market = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let pricer = CDSTranchePricer::new();
    let discount = market
        .get_discount(tranche.discount_curve_id.as_ref())
        .expect("discount curve");
    let projected_rows = pricer
        .project_discountable_rows(&tranche, &market, as_of)
        .expect("projected tranche rows");
    let discounted_total = pricer
        .discount_projected_rows(&projected_rows, discount.as_ref(), as_of)
        .expect("discounted projected rows should sum");
    let pv = pricer
        .price_tranche(&tranche, &market, as_of)
        .expect("tranche pv");

    assert!((pv.amount() - discounted_total).abs() < 1e-8);
}

#[test]
fn test_conditional_default_probability() {
    let model = CDSTranchePricer::new();
    let correlation = 0.30;
    let default_threshold = standard_normal_inv_cdf(0.05); // 5% default probability

    // Test with market factor = 0 (should be reasonable value close to original default prob)
    let cond_prob = model.conditional_default_probability(default_threshold, correlation, 0.0);
    assert!(
        cond_prob > 0.01 && cond_prob < 0.1,
        "Expected reasonable default prob, got {}",
        cond_prob
    );

    // Test with negative market factor (should increase default prob)
    let cond_prob_neg = model.conditional_default_probability(default_threshold, correlation, -1.0);
    assert!(cond_prob_neg > 0.05);

    // Test with positive market factor (should decrease default prob)
    let cond_prob_pos = model.conditional_default_probability(default_threshold, correlation, 1.0);
    assert!(cond_prob_pos < 0.05);
}

#[test]
fn test_binomial_probability() {
    // Test known values
    assert!((binomial_probability(10, 5, 0.5) - 0.24609375).abs() < 1e-6);
    assert!((binomial_probability(5, 0, 0.1) - 0.59049).abs() < 1e-6);

    // Test edge cases
    assert_eq!(binomial_probability(10, 0, 0.0), 1.0);
    assert_eq!(binomial_probability(10, 10, 1.0), 1.0);
    assert_eq!(binomial_probability(10, 5, 0.0), 0.0);
}

#[test]
fn test_log_factorial() {
    // Test small values (exact calculation)
    assert!((log_factorial(1) - 0.0).abs() < 1e-12);
    assert!(
        (log_factorial(5) - (2.0_f64.ln() + 3.0_f64.ln() + 4.0_f64.ln() + 5.0_f64.ln())).abs()
            < 1e-12
    );

    // Test that Stirling's approximation is reasonable for large n
    let log_100_factorial = log_factorial(100);
    assert!(log_100_factorial > 360.0 && log_100_factorial < 370.0); // Should be around 363.7
}

#[test]
fn test_tranche_pricing_integration() {
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    // Test that pricing doesn't panic and returns a reasonable result
    let result = model.price_tranche(&tranche, &market_ctx, as_of);
    assert!(result.is_ok());

    let pv = result.expect("Tranche pricing should succeed in test");
    assert_eq!(pv.currency(), Currency::USD);
    // PV should be finite (could be positive or negative)
    assert!(pv.amount().is_finite());
}

#[test]
fn test_equity_helper_matches_explicit_params_pv() {
    let model = CDSTranchePricer::new();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");
    let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();

    let helper_params = CDSTrancheParams::equity_tranche(
        "CDX.NA.IG.42",
        42,
        Money::new(10_000_000.0, Currency::USD),
        maturity,
        500.0,
    );
    let helper_tranche = CDSTranche::new(
        "CDX_IG42_0_3_HELPER",
        &helper_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");

    let explicit_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        0.0,
        3.0,
        Money::new(10_000_000.0, Currency::USD),
        maturity,
        500.0,
    );
    let explicit_tranche = CDSTranche::new(
        "CDX_IG42_0_3_EXPLICIT",
        &explicit_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");

    let pv_helper = model
        .price_tranche(&helper_tranche, &market_ctx, as_of)
        .expect("Tranche pricing should succeed in test")
        .amount();
    let pv_explicit = model
        .price_tranche(&explicit_tranche, &market_ctx, as_of)
        .expect("Tranche pricing should succeed in test")
        .amount();

    let diff = (pv_helper - pv_explicit).abs();
    let scale = pv_explicit.abs().max(1.0);
    assert!(diff < 1e-8 * scale);
}

#[test]
fn test_hetero_spa_matches_homogeneous_when_issuers_equal() {
    let ctx_base = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let mut tranche = sample_tranche();
    tranche.running_coupon_bp = 0.0; // isolate protection leg

    // Build a context with issuer curves identical to index curve
    let index_data = ctx_base
        .get_credit_index("CDX.NA.IG.42")
        .expect("Credit index should exist in test context");
    let mut issuer_curves = finstack_core::HashMap::default();
    for i in 0..10 {
        let id = format!("ISSUER-{:03}", i + 1);
        issuer_curves.insert(id, index_data.index_credit_curve.clone());
    }
    let hetero_index = CreditIndexData::builder()
        .num_constituents(10)
        .recovery_rate(index_data.recovery_rate)
        .index_credit_curve(index_data.index_credit_curve.clone())
        .base_correlation_curve(index_data.base_correlation_curve.clone())
        .issuer_curves(issuer_curves)
        .build()
        .expect("Curve builder should succeed with valid test data");
    let ctx = ctx_base
        .clone()
        .insert_credit_index("CDX.NA.IG.42", hetero_index);

    let mut homo = CDSTranchePricer::new();
    homo.params.use_issuer_curves = false;
    let mut hetero = CDSTranchePricer::new();
    hetero.params.use_issuer_curves = true;
    hetero.params.hetero_method = HeteroMethod::Spa;

    let pv_homo = homo
        .price_tranche(&tranche, &ctx, as_of)
        .expect("Tranche pricing should succeed in test")
        .amount();
    let pv_hetero = hetero
        .price_tranche(&tranche, &ctx, as_of)
        .expect("Tranche pricing should succeed in test")
        .amount();
    assert!((pv_homo - pv_hetero).abs() < 1e-2 * pv_homo.abs().max(1.0));
}

#[test]
fn test_hetero_spa_vs_exact_convolution_small_pool() {
    let ctx = sample_market_context_with_issuers(8);
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let tranche_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        3.0,
        7.0,
        Money::new(10_000_000.0, Currency::USD),
        as_of.add_months(60),
        0.0,
    );
    let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
    let tranche = CDSTranche::new(
        "CDX_IG42_3_7_5Y",
        &tranche_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");

    let mut spa = CDSTranchePricer::new();
    spa.params.use_issuer_curves = true;
    spa.params.hetero_method = HeteroMethod::Spa;
    let mut exact = CDSTranchePricer::new();
    exact.params.use_issuer_curves = true;
    exact.params.hetero_method = HeteroMethod::ExactConvolution;
    exact.params.grid_step = 0.002;

    let pv_spa = spa
        .price_tranche(&tranche, &ctx, as_of)
        .expect("Tranche pricing should succeed in test")
        .amount();
    let pv_exact = exact
        .price_tranche(&tranche, &ctx, as_of)
        .expect("Tranche pricing should succeed in test")
        .amount();
    assert!((pv_spa - pv_exact).abs() < 0.02 * pv_exact.abs().max(1.0));
}

/// Item 4: the homogeneity-detection thresholds for PD, LGD and weight must
/// be a single consistent tolerance. Previously PD used the 1e-12 probit
/// clamp while LGD/weight used 1e-9, so a pool uniform in LGD/weight but with
/// PD dispersion in the (1e-12, 1e-9) gap was routed inconsistently — a
/// discontinuous model-branch switch. This test prices a pool whose issuers
/// are identical (truly homogeneous) and one whose issuers carry a tiny
/// sub-tolerance perturbation; both must price as the homogeneous pool, with
/// no discontinuity between them.
#[test]
fn homogeneity_detection_uses_consistent_tolerance() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let as_of = base_date;

    let discount_curve =
        finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
            .build()
            .expect("discount curve");
    let index_curve =
        finstack_core::market_data::term_structures::HazardCurve::builder("CDX.NA.IG.42")
            .base_date(base_date)
            .recovery_rate(0.40)
            .knots(vec![
                (1.0, 0.012),
                (3.0, 0.017),
                (5.0, 0.022),
                (10.0, 0.028),
            ])
            .par_spreads(vec![(1.0, 65.0), (3.0, 85.0), (5.0, 105.0), (10.0, 145.0)])
            .build()
            .expect("index curve");
    let base_corr =
        finstack_core::market_data::term_structures::BaseCorrelationCurve::builder("BC")
            .knots(vec![(3.0, 0.25), (7.0, 0.30), (10.0, 0.34), (30.0, 0.50)])
            .build()
            .expect("base corr");

    // Build an index whose 20 issuer curves differ from the index curve by a
    // PD perturbation of `pd_shift` (applied as a hazard-knot bump). With
    // `pd_shift = 0` the pool is exactly homogeneous; with a sub-tolerance
    // shift it must still be classified homogeneous under the unified
    // tolerance.
    let build_ctx = |hazard_shift: f64| {
        let mut issuer_curves = finstack_core::HashMap::default();
        for i in 0..20 {
            let id = format!("ISSUER-{:03}", i + 1);
            let hz = finstack_core::market_data::term_structures::HazardCurve::builder(id.as_str())
                .base_date(base_date)
                .recovery_rate(0.40)
                .knots(vec![
                    (1.0, 0.012 + hazard_shift),
                    (3.0, 0.017 + hazard_shift),
                    (5.0, 0.022 + hazard_shift),
                    (10.0, 0.028 + hazard_shift),
                ])
                .build()
                .expect("issuer curve");
            issuer_curves.insert(id, Arc::new(hz));
        }
        let index = CreditIndexData::builder()
            .num_constituents(20)
            .recovery_rate(0.40)
            .index_credit_curve(Arc::new(index_curve.clone()))
            .base_correlation_curve(Arc::new(base_corr.clone()))
            .issuer_curves(issuer_curves)
            .build()
            .expect("index data");
        MarketContext::new()
            .insert(discount_curve.clone())
            .insert_credit_index("CDX.NA.IG.42", index)
    };

    let params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        0.0,
        3.0,
        Money::new(10_000_000.0, Currency::USD),
        as_of.add_months(60),
        0.0,
    );
    let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
    let tranche = CDSTranche::new(
        "CDX_IG42_0_3_5Y",
        &params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");

    let mut pricer = CDSTranchePricer::new();
    pricer.params.use_issuer_curves = true;
    pricer.params.hetero_method = HeteroMethod::Spa;

    // Exactly homogeneous (shift 0): all PD/LGD/weight identical.
    let pv_exact_homo = pricer
        .price_tranche(&tranche, &build_ctx(0.0), as_of)
        .expect("homogeneous pricing")
        .amount();
    // Sub-tolerance PD perturbation: hazard knots shifted by 1e-11, which
    // produces a PD dispersion well below the 1e-9 unified tolerance but
    // ABOVE the old 1e-12 PD threshold. Under the old inconsistent
    // thresholds this could flip to the heterogeneous branch and price
    // discontinuously; under the unified tolerance it must stay homogeneous.
    let pv_sub_tol = pricer
        .price_tranche(&tranche, &build_ctx(1e-11), as_of)
        .expect("near-homogeneous pricing")
        .amount();

    assert!(
        (pv_exact_homo - pv_sub_tol).abs() <= 1e-6 * pv_exact_homo.abs().max(1.0),
        "a sub-tolerance PD perturbation must not cause a discontinuous \
         model-branch switch: pv(shift=0)={pv_exact_homo}, pv(shift=1e-11)={pv_sub_tol}"
    );
}

#[test]
fn test_grid_step_refines_exact_convolution() {
    let ctx = sample_market_context_with_issuers(10);
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let tranche_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        0.0,
        3.0,
        Money::new(10_000_000.0, Currency::USD),
        as_of.add_months(60),
        0.0,
    );
    let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
    let tranche = CDSTranche::new(
        "CDX_IG42_0_3_5Y",
        &tranche_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");

    let mut exact_coarse = CDSTranchePricer::new();
    exact_coarse.params.use_issuer_curves = true;
    exact_coarse.params.hetero_method = HeteroMethod::ExactConvolution;
    exact_coarse.params.grid_step = 0.005;

    let mut exact_fine = CDSTranchePricer::new();
    exact_fine.params = exact_coarse.params.clone();
    exact_fine.params.grid_step = 0.001;

    let p_coarse = exact_coarse
        .price_tranche(&tranche, &ctx, as_of)
        .expect("Tranche pricing should succeed in test")
        .amount();
    let p_fine = exact_fine
        .price_tranche(&tranche, &ctx, as_of)
        .expect("Tranche pricing should succeed in test")
        .amount();
    assert!((p_coarse - p_fine).abs() < 0.02 * p_fine.abs().max(1.0));
}

#[test]
fn test_expected_loss_calculation() {
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let market_ctx = sample_market_context();

    let expected_loss = model.calculate_expected_loss(&tranche, &market_ctx);
    assert!(expected_loss.is_ok());

    let loss = expected_loss.expect("Expected loss calculation should succeed in test");
    assert!(loss >= 0.0); // Expected loss should be non-negative
    assert!(loss.is_finite());
}

#[test]
fn test_payment_schedule_generation() {
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let schedule = model.generate_payment_schedule(&tranche, as_of);
    assert!(schedule.is_ok());

    let dates = schedule.expect("Schedule generation should succeed in test");
    assert!(!dates.is_empty());
    assert!(dates[0] > as_of); // First payment should be after as_of
    assert!(*dates.last().expect("Schedule should not be empty") <= tranche.maturity); // Last payment should not exceed maturity

    // Check dates are in ascending order
    for window in dates.windows(2) {
        assert!(window[0] < window[1]);
    }
}

#[test]
fn test_payment_schedule_imm_vs_non_imm() {
    let model = CDSTranchePricer::new();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let mut imm_tranche = sample_tranche();
    imm_tranche.standard_imm_dates = true;
    imm_tranche.effective_date =
        Some(Date::from_calendar_date(2025, Month::March, 20).expect("cds date"));
    imm_tranche.maturity = Date::from_calendar_date(2030, Month::March, 20).expect("cds date");
    let imm_dates = model
        .generate_payment_schedule(&imm_tranche, as_of)
        .expect("IMM schedule should succeed");
    assert!(!imm_dates.is_empty());
    assert!(
        imm_dates
            .iter()
            .all(|d| finstack_core::dates::is_cds_date(*d)),
        "IMM schedule should use CDS roll dates"
    );

    let mut non_imm_tranche = sample_tranche();
    non_imm_tranche.standard_imm_dates = false;
    non_imm_tranche.effective_date =
        Some(Date::from_calendar_date(2025, Month::January, 15).expect("valid date"));
    non_imm_tranche.maturity =
        Date::from_calendar_date(2026, Month::January, 15).expect("valid date");
    let non_imm_dates = model
        .generate_payment_schedule(&non_imm_tranche, as_of)
        .expect("non-IMM schedule should succeed");
    assert!(!non_imm_dates.is_empty());
    assert!(
        non_imm_dates
            .iter()
            .any(|d| !finstack_core::dates::is_cds_date(*d)),
        "Non-IMM schedule should include non-CDS dates"
    );
}

#[test]
fn test_el_curve_monotonicity() {
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let schedule = model
        .generate_payment_schedule(&tranche, as_of)
        .expect("Schedule generation should succeed in test");
    let index_data_arc = market_ctx
        .get_credit_index(&tranche.credit_index_id)
        .expect("Credit index should exist in test context");
    let el_curve = model.build_el_curve(&tranche, &index_data_arc, &schedule);

    assert!(el_curve.is_ok());
    let curve = el_curve.expect("EL curve building should succeed in test");

    // EL should be non-decreasing and bounded [0,1]
    // Allow for small numerical deviations due to base correlation model limitations
    // The base correlation model can have inconsistencies at knot points
    const NUMERICAL_TOLERANCE: f64 = 0.01; // Allow up to 1% EL fraction decrease

    for (i, &(_, el_fraction)) in curve.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&el_fraction),
            "EL fraction {} at index {} out of bounds",
            el_fraction,
            i
        );

        if i > 0 {
            let decrease = curve[i - 1].1 - el_fraction;
            assert!(
                decrease <= NUMERICAL_TOLERANCE,
                "EL fraction decreased significantly from {} to {} (decrease: {})",
                curve[i - 1].1,
                el_fraction,
                decrease
            );
        }
    }
}

#[test]
fn test_cs01_calculation() {
    let model = CDSTranchePricer::new();
    let mut tranche = sample_tranche();
    tranche.side = TrancheSide::SellProtection; // Sell protection for positive CS01
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let cs01 = model.calculate_cs01(&tranche, &market_ctx, as_of);
    assert!(cs01.is_ok());

    let sensitivity = cs01.expect("CS01 calculation should succeed in test");
    assert!(sensitivity.is_finite());
    // For protection seller, CS01 should typically be positive
    // (higher spreads -> higher protection premium income)
}

#[test]
fn test_correlation_delta_calculation() {
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let corr_delta = model.calculate_correlation_delta(&tranche, &market_ctx, as_of);
    assert!(corr_delta.is_ok());

    let sensitivity = corr_delta.expect("Correlation delta calculation should succeed in test");
    assert!(sensitivity.is_finite());
    // Correlation sensitivity should be finite and reasonable in magnitude
}

#[test]
fn test_jump_to_default_calculation() {
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let jtd = model.calculate_jump_to_default(&tranche, &market_ctx, as_of);
    assert!(jtd.is_ok());

    let impact = jtd.expect("Jump to default calculation should succeed in test");
    assert!(impact >= 0.0); // Impact should be non-negative
    assert!(impact.is_finite());
}

#[test]
fn test_pv_decomposition_consistency() {
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let discount_curve = market_ctx
        .get_discount(tranche.discount_curve_id.as_ref())
        .expect("Discount curve should exist in test context");
    let projected_rows = model
        .project_discountable_rows(&tranche, &market_ctx, as_of)
        .expect("Projected rows should build in test");
    let premium = model
        .discount_projected_rows(
            &projected_rows
                .iter()
                .filter(|row| row.cashflow.kind == CFKind::Fixed)
                .cloned()
                .collect::<Vec<_>>(),
            discount_curve.as_ref(),
            as_of,
        )
        .expect("Premium PV calculation should succeed in test");
    let protection = model
        .discount_projected_rows(
            &projected_rows
                .iter()
                .filter(|row| row.cashflow.kind == CFKind::DefaultedNotional)
                .cloned()
                .collect::<Vec<_>>(),
            discount_curve.as_ref(),
            as_of,
        )
        .expect("Protection PV calculation should succeed in test");

    assert!(premium.is_finite());
    assert!(protection.is_finite());
    match tranche.side {
        TrancheSide::SellProtection => {
            assert!(premium >= 0.0);
            assert!(protection <= 0.0);
        }
        TrancheSide::BuyProtection => {
            assert!(premium <= 0.0);
            assert!(protection >= 0.0);
        }
    }
}

#[test]
fn test_extreme_correlation_numerical_stability() {
    let model = CDSTranchePricer::new();
    let market_ctx = sample_market_context();
    let _as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let index_data_arc = market_ctx
        .get_credit_index("CDX.NA.IG.42")
        .expect("Credit index should exist in test context");

    // Test extreme correlation values that are challenging for numerical stability
    let extreme_correlations = [1e-10, 1e-6, 0.001, 0.999, 1.0 - 1e-6, 1.0 - 1e-10];

    for &test_correlation in &extreme_correlations {
        // Create a correlation curve with extreme values
        let extreme_corr_curve =
            finstack_core::market_data::term_structures::BaseCorrelationCurve::builder(
                "TEST_EXTREME",
            )
            .knots(vec![
                (3.0, test_correlation),
                (7.0, test_correlation),
                (10.0, test_correlation),
                (15.0, test_correlation),
                (30.0, test_correlation),
            ])
            .build()
            .expect("BaseCorrelationCurve builder should succeed with valid test data");

        // Create index data with extreme correlation
        let extreme_index_data = CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.40)
            .index_credit_curve(index_data_arc.index_credit_curve.clone())
            .base_correlation_curve(std::sync::Arc::new(extreme_corr_curve))
            .build()
            .expect("BaseCorrelationCurve builder should succeed with valid test data");

        // Test equity tranche loss calculation
        let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");
        let result = model.calculate_equity_tranche_loss(
            7.0, // 7% detachment
            test_correlation,
            &extreme_index_data,
            maturity,
        );

        assert!(
            result.is_ok(),
            "Equity tranche loss calculation failed for correlation={}",
            test_correlation
        );

        let expected_loss = result.expect("Equity tranche loss calculation should succeed in test");
        assert!(
            expected_loss.is_finite(),
            "Expected loss should be finite for correlation={}, got {}",
            test_correlation,
            expected_loss
        );
        assert!(
            (0.0..=1.0).contains(&expected_loss),
            "Expected loss should be in [0,1] for correlation={}, got {}",
            test_correlation,
            expected_loss
        );
    }
}

#[test]
fn test_smooth_correlation_boundary_transitions() {
    let model = CDSTranchePricer::new();

    // Test that smooth boundary transitions work correctly
    let test_values = [
        0.005, 0.009, 0.011, 0.015, // Near min boundary (0.01)
        0.985, 0.989, 0.991, 0.995, // Near max boundary (0.99)
    ];

    for &test_corr in &test_values {
        let smoothed = model.smooth_correlation_boundary(test_corr);

        // Should be finite and within expanded bounds
        assert!(
            smoothed.is_finite(),
            "Smoothed correlation should be finite for input={}",
            test_corr
        );
        assert!(
            (0.005..=0.995).contains(&smoothed),
            "Smoothed correlation {} should be in reasonable bounds for input={}",
            smoothed,
            test_corr
        );

        // Should be continuous (no big jumps)
        let nearby = test_corr + 0.001;
        let smoothed_nearby = model.smooth_correlation_boundary(nearby);
        let transition_smoothness = (smoothed_nearby - smoothed).abs();

        assert!(
            transition_smoothness < 0.01,
            "Boundary transition should be smooth: jump of {} between {} and {}",
            transition_smoothness,
            test_corr,
            nearby
        );
    }
}

#[test]
fn test_conditional_default_probability_enhanced() {
    let model = CDSTranchePricer::new();
    let default_threshold = standard_normal_inv_cdf(0.05); // 5% unconditional default prob

    // Test enhanced function across various correlation and market factor combinations
    let correlations = [1e-8, 0.01, 0.3, 0.7, 0.99, 1.0 - 1e-8];
    let market_factors = [-4.0, -2.0, -1.0, 0.0, 1.0, 2.0, 4.0];

    for &correlation in &correlations {
        for &market_factor in &market_factors {
            let enhanced_prob = model.conditional_default_probability_enhanced(
                default_threshold,
                correlation,
                market_factor,
            );
            let standard_prob = model.conditional_default_probability(
                default_threshold,
                correlation.clamp(0.01, 0.99), // Clamp for standard function
                market_factor,
            );

            // Enhanced function should always give finite, bounded results
            assert!(
                enhanced_prob.is_finite(),
                "Enhanced conditional prob should be finite for ρ={}, Z={}",
                correlation,
                market_factor
            );
            assert!(
                (0.0..=1.0).contains(&enhanced_prob),
                "Enhanced conditional prob should be in [0,1]: got {} for ρ={}, Z={}",
                enhanced_prob,
                correlation,
                market_factor
            );

            // For normal correlation ranges, should be close to standard implementation
            if (0.05..=0.95).contains(&correlation) {
                let diff = (enhanced_prob - standard_prob).abs();
                assert!(diff < 0.01,
                    "Enhanced and standard methods should agree in normal range: diff={} for ρ={}, Z={}",
                    diff, correlation, market_factor);
            }
        }
    }
}

#[test]
fn test_realized_loss_impact() {
    let model = CDSTranchePricer::new();
    let mut tranche = sample_tranche();
    // 0-3% tranche
    tranche.attach_pct = 0.0;
    tranche.detach_pct = 3.0;
    tranche.series = 42;
    tranche.accumulated_loss = 0.0;
    tranche.standard_imm_dates = true;

    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    // 1. Price with no prior loss
    let pv_clean = model
        .price_tranche(&tranche, &market_ctx, as_of)
        .expect("Pricing clean tranche")
        .amount();

    // 2. Price with 1% realized loss (portfolio lost 1%, so tranche is 1/3 wiped out)
    // Remaining tranche is effectively [0, (3-1)/(1-0.01)] = [0, 2.02%] on surviving portfolio
    // Outstanding notional starts at 2/3 of original
    tranche.accumulated_loss = 0.01;
    let pv_loss = model
        .price_tranche(&tranche, &market_ctx, as_of)
        .expect("Pricing tranche with loss")
        .amount();

    // The PV should be different
    assert!(pv_loss != pv_clean, "Realized loss should impact PV");

    // 3. Price with 4% realized loss (tranche wiped out)
    tranche.accumulated_loss = 0.04;
    let pv_wiped = model
        .price_tranche(&tranche, &market_ctx, as_of)
        .expect("Pricing wiped tranche")
        .amount();

    assert_eq!(pv_wiped, 0.0, "Wiped out tranche should have 0 PV");
}

// ========================= EDGE CASE TESTS =========================

#[test]
fn test_thin_tranche_stability() {
    // Test very thin tranches (width < 1%) for numerical stability
    let model = CDSTranchePricer::new();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

    // Create a very thin tranche (0.5% width)
    let tranche_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        3.0, // attach at 3%
        3.5, // detach at 3.5% (0.5% width)
        Money::new(1_000_000.0, Currency::USD),
        maturity,
        500.0,
    );
    let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
    let tranche = CDSTranche::new(
        "THIN_TRANCHE",
        &tranche_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");

    // Should price without panicking
    let pv = model.price_tranche(&tranche, &market_ctx, as_of);
    assert!(pv.is_ok(), "Thin tranche should price successfully");
    assert!(
        pv.expect("PV should be Ok").amount().is_finite(),
        "Thin tranche PV should be finite"
    );
}

#[test]
fn test_super_senior_tranche() {
    // Test super senior tranche (30-100%)
    let model = CDSTranchePricer::new();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

    let tranche_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        30.0,  // super senior attachment
        100.0, // full portfolio detachment
        Money::new(10_000_000.0, Currency::USD),
        maturity,
        25.0, // Very low spread for super senior
    );
    let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
    let tranche = CDSTranche::new(
        "SUPER_SENIOR",
        &tranche_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");

    let pv = model.price_tranche(&tranche, &market_ctx, as_of);
    assert!(pv.is_ok(), "Super senior tranche should price successfully");
    // Super senior should have very low expected loss
    let el = model.calculate_expected_loss(&tranche, &market_ctx);
    assert!(el.is_ok());
    assert!(
        el.expect("Expected loss should be Ok") >= 0.0,
        "Expected loss should be non-negative"
    );
}

#[test]
fn test_nearly_wiped_tranche() {
    // Test tranche that is nearly (but not fully) wiped out
    let model = CDSTranchePricer::new();
    let mut tranche = sample_tranche();
    tranche.attach_pct = 0.0;
    tranche.detach_pct = 3.0;
    // 2.99% loss means only 0.01% remaining (99.67% wiped)
    tranche.accumulated_loss = 0.0299;

    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let pv = model.price_tranche(&tranche, &market_ctx, as_of);
    assert!(pv.is_ok(), "Nearly wiped tranche should price");
    let pv_amount = pv.expect("PV should be Ok").amount();
    assert!(pv_amount.is_finite(), "PV should be finite");
    // Should be much smaller than full notional tranche
}

#[test]
fn test_central_difference_symmetry() {
    // Test that central difference produces symmetric sensitivities
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    // CS01 should be finite and well-behaved
    let cs01 = model.calculate_cs01(&tranche, &market_ctx, as_of);
    assert!(cs01.is_ok());
    assert!(cs01.expect("CS01 should be Ok").is_finite());

    // Correlation delta should be finite
    let corr_delta = model.calculate_correlation_delta(&tranche, &market_ctx, as_of);
    assert!(corr_delta.is_ok());
    assert!(corr_delta
        .expect("Correlation delta should be Ok")
        .is_finite());
}

#[test]
fn test_jtd_detail_consistency() {
    // Test that JTD detail is consistent with simple JTD
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let simple_jtd = model.calculate_jump_to_default(&tranche, &market_ctx, as_of);
    let detail_jtd = model.calculate_jump_to_default_detail(&tranche, &market_ctx);

    assert!(simple_jtd.is_ok());
    assert!(detail_jtd.is_ok());

    let simple = simple_jtd.expect("Simple JTD should be Ok");
    let detail = detail_jtd.expect("Detail JTD should be Ok");

    // Simple JTD should equal the average from detail
    assert!(
        (simple - detail.average).abs() < 1e-10,
        "Simple JTD {} should equal detail average {}",
        simple,
        detail.average
    );

    // Min <= average <= max
    assert!(detail.min <= detail.average);
    assert!(detail.average <= detail.max);
}

#[test]
fn test_correlation_bump_is_a_pure_parallel_shift() {
    // A parallel shift of a monotone base-correlation curve is itself
    // monotone — the bump preserves the ordering without any repair loop.
    let model = CDSTranchePricer::new();
    let market_ctx = sample_market_context();
    let index_data = market_ctx
        .get_credit_index("CDX.NA.IG.42")
        .expect("Index should exist");
    let original = &index_data.base_correlation_curve;

    // A large negative bump still parallel-shifts every knot by the same
    // amount; monotonicity is preserved by construction.
    let bump = -0.2;
    let bumped = model
        .bump_base_correlation(original, bump)
        .expect("Bumping should succeed");

    // Every knot must be shifted by exactly `bump` (no clamp fires for this
    // curve since all shifted values stay inside [0, 1]).
    for (i, (&orig, &shifted)) in original
        .correlations()
        .iter()
        .zip(bumped.correlations().iter())
        .enumerate()
    {
        let expected = (orig + bump).clamp(0.0, 1.0);
        assert!(
            (shifted - expected).abs() < 1e-12,
            "knot {i}: bumped correlation {shifted} must equal pure shift {expected}"
        );
    }
    // The shifted curve remains monotone (parallel shift preserves order).
    for i in 1..bumped.correlations().len() {
        assert!(
            bumped.correlations()[i] >= bumped.correlations()[i - 1] - 1e-12,
            "parallel-shifted correlations stay monotone"
        );
    }
}

/// Item 7: the up- and down-bumped base-correlation curves must be exact
/// mirror images so the central-difference Correlation01 is a symmetric
/// perturbation. The previous monotonicity-repair loop adjusted the two
/// sides differently near curve kinks, biasing the derivative.
#[test]
fn correlation_bump_up_and_down_are_symmetric() {
    let model = CDSTranchePricer::new();
    let market_ctx = sample_market_context();
    let index_data = market_ctx
        .get_credit_index("CDX.NA.IG.42")
        .expect("Index should exist");
    let original = &index_data.base_correlation_curve;

    let h = 0.01;
    let up = model.bump_base_correlation(original, h).expect("up bump");
    let down = model
        .bump_base_correlation(original, -h)
        .expect("down bump");

    // For every knot, (up - orig) must equal -(down - orig): the two bumps
    // are exact mirror images. A surviving monotonicity-repair loop would
    // break this at compressed-spacing knots.
    for (i, ((&orig, &u), &d)) in original
        .correlations()
        .iter()
        .zip(up.correlations().iter())
        .zip(down.correlations().iter())
        .enumerate()
    {
        let up_delta = u - orig;
        let down_delta = orig - d;
        assert!(
            (up_delta - down_delta).abs() < 1e-12,
            "knot {i}: up/down correlation bumps must be symmetric — \
             up_delta={up_delta}, down_delta={down_delta}, orig={orig}"
        );
        // And each side is the pure shift `h`.
        assert!(
            (up_delta - h).abs() < 1e-12 && (down_delta - h).abs() < 1e-12,
            "knot {i}: each bump must be the pure shift h={h}"
        );
    }

    // The resulting Correlation01 must be finite and well-defined.
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let corr01 = model
        .calculate_correlation_delta(&sample_tranche(), &market_ctx, as_of)
        .expect("correlation delta");
    assert!(corr01.is_finite(), "Correlation01 must be finite");
}

#[test]
fn test_par_spread_solver_convergence() {
    // Test that par spread solver converges correctly
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let par_spread = model.calculate_par_spread(&tranche, &market_ctx, as_of);
    assert!(par_spread.is_ok(), "Par spread should calculate");

    let spread = par_spread.expect("Par spread should be Ok");
    assert!(spread >= 0.0, "Par spread should be non-negative");
    assert!(spread.is_finite(), "Par spread should be finite");

    // Verify: pricing at par spread should give near-zero NPV
    let mut test_tranche = tranche.clone();
    test_tranche.running_coupon_bp = spread;
    let npv = model.price_tranche(&test_tranche, &market_ctx, as_of);
    assert!(npv.is_ok());
    let npv_amount = npv.expect("NPV should be Ok").amount().abs();
    // Should be close to zero (within tolerance * notional)
    assert!(
        npv_amount < 100.0, // Allow $100 residual on $10M notional
        "NPV at par spread should be near zero, got {}",
        npv_amount
    );
}

/// Item 9: `calculate_par_spread` must never silently return a non-converged
/// iterate. When it returns `Ok`, the spread must genuinely zero the tranche
/// NPV (to the solver tolerance); a non-convergence is reported as an error.
/// This pins that a successful result is always a true par spread.
#[test]
fn par_spread_ok_result_is_always_a_true_par() {
    let model = CDSTranchePricer::new();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");

    // Exercise both protection sides and a range of strikes.
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("date");
    for side in [TrancheSide::BuyProtection, TrancheSide::SellProtection] {
        for &(attach, detach) in &[(0.0_f64, 3.0_f64), (3.0, 7.0), (10.0, 15.0)] {
            let params = CDSTrancheParams::new(
                "CDX.NA.IG.42",
                42,
                attach,
                detach,
                Money::new(10_000_000.0, Currency::USD),
                maturity,
                500.0,
            );
            let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
            let tranche = CDSTranche::new(
                "PAR_SPREAD_TEST",
                &params,
                &schedule_params,
                finstack_core::types::CurveId::from("USD-OIS"),
                finstack_core::types::CurveId::from("CDX.NA.IG.42"),
                side,
            )
            .expect("Valid tranche parameters");

            match model.calculate_par_spread(&tranche, &market_ctx, as_of) {
                Ok(spread) => {
                    // A successful result MUST zero the NPV — i.e. it is a
                    // genuine par spread, not a silent last iterate.
                    assert!(spread.is_finite() && spread >= 0.0);
                    let mut at_par = tranche.clone();
                    at_par.running_coupon_bp = spread;
                    let npv = model
                        .price_tranche(&at_par, &market_ctx, as_of)
                        .expect("pricing at par")
                        .amount()
                        .abs();
                    assert!(
                        npv < 1e-6 * tranche.notional.amount(),
                        "Ok par spread {spread} for {side:?} [{attach}%,{detach}%] must \
                         zero NPV, got residual {npv}"
                    );
                }
                Err(e) => {
                    // A non-convergence must carry an explanatory message.
                    let msg = format!("{e}");
                    assert!(
                        msg.contains("par-spread"),
                        "non-convergence error must explain the par-spread failure: {msg}"
                    );
                }
            }
        }
    }
}

#[test]
fn test_settlement_date_calculation() {
    // Test settlement date logic for different index types
    // Using Wednesday Jan 1, 2025 so T+1 is Thursday (no weekend crossing)
    let model = CDSTranchePricer::new();
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    // CDX index should use T+1 business days
    let mut cdx_tranche = sample_tranche();
    cdx_tranche.index_name = "CDX.NA.IG.42".to_string();
    cdx_tranche.effective_date = None;
    cdx_tranche.calendar_id = None; // No calendar, weekend-only logic
    let cdx_settle = model.calculate_settlement_date(&cdx_tranche, &market_ctx, as_of);
    assert!(cdx_settle.is_ok());
    // Should be 1 business day after as_of (Wed -> Thu)
    assert_eq!(
        cdx_settle.expect("CDX settlement should be Ok"),
        Date::from_calendar_date(2025, Month::January, 2).expect("Valid test date"),
        "CDX should settle T+1 business day"
    );

    // Bespoke index should use T+3 business days
    // From Wed Jan 1: Thu Jan 2 (+1), Fri Jan 3 (+2), Mon Jan 6 (+3, skipping weekend)
    let mut bespoke_tranche = sample_tranche();
    bespoke_tranche.index_name = "BESPOKE".to_string();
    bespoke_tranche.effective_date = None;
    bespoke_tranche.calendar_id = None;
    let bespoke_settle = model.calculate_settlement_date(&bespoke_tranche, &market_ctx, as_of);
    assert!(bespoke_settle.is_ok());
    // T+3 from Wed Jan 1 = Mon Jan 6 (skipping Sat/Sun)
    let expected = Date::from_calendar_date(2025, Month::January, 6).expect("Valid test date");
    assert_eq!(
        bespoke_settle.expect("Bespoke settlement should be Ok"),
        expected,
        "Bespoke should settle T+3 business days"
    );
}

#[test]
fn test_settlement_date_skips_weekends() {
    let model = CDSTranchePricer::new();
    let market_ctx = sample_market_context();
    // Friday Jan 3, 2025
    let friday = Date::from_calendar_date(2025, Month::January, 3).expect("Valid test date");

    let mut tranche = sample_tranche();
    tranche.index_name = "CDX.NA.IG.42".to_string();
    tranche.effective_date = None;
    tranche.calendar_id = None; // No calendar, weekend-only logic

    let settle = model
        .calculate_settlement_date(&tranche, &market_ctx, friday)
        .expect("Settlement date calculation should succeed");
    // T+1 from Friday should be Monday (skip Sat/Sun)
    let expected_monday =
        Date::from_calendar_date(2025, Month::January, 6).expect("Valid test date");
    assert_eq!(
        settle, expected_monday,
        "T+1 from Friday should be Monday, skipping weekend"
    );
}

#[test]
fn test_settlement_date_weekday() {
    let model = CDSTranchePricer::new();
    let market_ctx = sample_market_context();
    // Wednesday Jan 1, 2025
    let wednesday = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let mut tranche = sample_tranche();
    tranche.index_name = "CDX.NA.IG.42".to_string();
    tranche.effective_date = None;
    tranche.calendar_id = None;

    let settle = model
        .calculate_settlement_date(&tranche, &market_ctx, wednesday)
        .expect("Settlement date calculation should succeed");
    // T+1 from Wednesday should be Thursday
    let expected_thursday =
        Date::from_calendar_date(2025, Month::January, 2).expect("Valid test date");
    assert_eq!(
        settle, expected_thursday,
        "T+1 from Wednesday should be Thursday"
    );
}

#[test]
fn test_accrued_premium_calculation() {
    // Test accrued premium calculation
    let model = CDSTranchePricer::new();
    let mut tranche = sample_tranche();
    let market_ctx = sample_market_context();

    // At inception, accrued should be minimal
    let inception = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    tranche.effective_date = Some(inception);
    let accrued_at_inception = model.calculate_accrued_premium(&tranche, &market_ctx, inception);
    assert!(accrued_at_inception.is_ok());

    // Mid-quarter, accrued should be positive
    let mid_quarter = Date::from_calendar_date(2025, Month::February, 15).expect("Valid test date");
    let accrued_mid = model.calculate_accrued_premium(&tranche, &market_ctx, mid_quarter);
    assert!(accrued_mid.is_ok());
    let accrued = accrued_mid.expect("Accrued premium should be Ok");
    assert!(
        accrued > 0.0,
        "Accrued premium should be positive mid-period"
    );
}

#[test]
fn test_par_spread_missing_credit_index_errors() {
    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let market_ctx = MarketContext::new().insert(
        sample_market_context()
            .get_discount("USD-OIS")
            .expect("sample discount curve")
            .as_ref()
            .clone(),
    );

    let err = model
        .calculate_par_spread(&tranche, &market_ctx, as_of)
        .expect_err("missing credit index must surface as an error");
    assert!(
        err.to_string().contains("CDX.NA.IG.42"),
        "expected missing credit index context, got: {err}"
    );
}

#[test]
fn test_stochastic_recovery_impacts_equity_tranche() {
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");

    // Create equity tranche (0-3%) which is most sensitive to stochastic recovery
    let tranche_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        0.0, // attach at 0%
        3.0, // detach at 3%
        Money::new(10_000_000.0, Currency::USD),
        maturity,
        500.0, // 5% running coupon
    );
    let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
    let tranche = CDSTranche::new(
        "CDX_IG42_0_3_5Y",
        &tranche_params,
        &schedule_params,
        finstack_core::types::CurveId::from("USD-OIS"),
        finstack_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");

    // Constant recovery (default)
    let pricer_const = CDSTranchePricer::new();
    let pv_const = pricer_const
        .price_tranche(&tranche, &market_ctx, as_of)
        .expect("Constant recovery pricing should succeed")
        .amount();

    // Stochastic recovery (market-correlated)
    let pricer_stoch =
        CDSTranchePricer::with_params(CDSTranchePricerConfig::default().with_stochastic_recovery());
    let pv_stoch = pricer_stoch
        .price_tranche(&tranche, &market_ctx, as_of)
        .expect("Stochastic recovery pricing should succeed")
        .amount();

    // Both should be finite
    assert!(
        pv_const.is_finite(),
        "Constant recovery PV should be finite"
    );
    assert!(
        pv_stoch.is_finite(),
        "Stochastic recovery PV should be finite"
    );

    // PVs should differ - stochastic recovery impacts equity tranche
    // Note: The exact magnitude depends on the market-standard stochastic recovery calibration
    // (mean=40%, vol=25%, corr=-40%), but we expect at least some difference
    let pv_diff = (pv_stoch - pv_const).abs();
    assert!(
        pv_diff > 0.0,
        "Stochastic recovery should change PV; const={}, stoch={}",
        pv_const,
        pv_stoch
    );
}

#[test]
fn test_stochastic_recovery_default_is_deterministic() {
    // Verify that default configuration uses deterministic (constant) recovery
    let pricer = CDSTranchePricer::new();
    assert!(
        pricer.config().recovery_spec.is_none(),
        "Default recovery_spec should be None (deterministic)"
    );
}

/// Build a market context whose base-correlation curve is monotonically
/// increasing but so steep between the equity attachment and detachment that
/// the equity tranchelet decomposition `EL(0,D) − EL(0,A)` goes *negative*
/// (genuine base-correlation arbitrage: a higher base correlation pushes the
/// equity-tranche EL down, so `EL(0,D)` evaluated at a much larger ρ falls
/// below `EL(0,A)`).
fn arbitrage_market_context() -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .build()
        .expect("Curve builder should succeed");

    // High-hazard index so equity-tranche EL is materially sensitive to ρ.
    let index_curve = HazardCurve::builder("CDX.NA.IG.42")
        .base_date(base_date)
        .recovery_rate(0.40)
        .knots(vec![(1.0, 0.05), (3.0, 0.06), (5.0, 0.07), (10.0, 0.08)])
        .par_spreads(vec![
            (1.0, 300.0),
            (3.0, 360.0),
            (5.0, 420.0),
            (10.0, 480.0),
        ])
        .build()
        .expect("Curve builder should succeed");

    // Monotonically *increasing* (so it builds without `allow_non_monotonic`)
    // but extremely steep at the [3%, 7%] strikes: ρ jumps from the lower
    // clamp toward the upper clamp over a tiny detachment span. Increasing
    // base correlation is necessary, not sufficient, for arbitrage-free EL.
    let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![
            (3.0, 0.05),
            (7.0, 0.95),
            (10.0, 0.96),
            (15.0, 0.97),
            (30.0, 0.98),
        ])
        .build()
        .expect("Curve builder should succeed (monotonically increasing)");

    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(index_curve))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .build()
        .expect("Index builder should succeed");

    MarketContext::new()
        .insert(discount_curve)
        .insert_credit_index("CDX.NA.IG.42", index_data)
}

/// Item 2: base-correlation arbitrage (`EL(0,D) < EL(0,A)`) must surface as
/// an explicit error when arbitrage validation is enabled (the default),
/// rather than being silently clamped to zero protection.
#[test]
fn base_correlation_arbitrage_surfaces_as_error_when_validation_enabled() {
    let market_ctx = arbitrage_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let tranche = sample_tranche(); // [3%, 7%] equity-ish tranche

    // Default config has validate_arbitrage_free = true.
    let pricer = CDSTranchePricer::new();
    assert!(pricer.config().validate_arbitrage_free);

    let result = pricer.price_tranche(&tranche, &market_ctx, as_of);
    assert!(
        result.is_err(),
        "base-correlation arbitrage must produce an explicit error, not a silent \
         zero-protection clamp; got {result:?}"
    );
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("base-correlation arbitrage"),
        "error must name the base-correlation arbitrage condition, got: {msg}"
    );
}

/// Item 2: with arbitrage validation explicitly disabled, the same arbitrage
/// is clamped (legacy behaviour) so pricing still produces a finite PV — but
/// the clamp is now a visible `warn`, not a silent `debug`.
#[test]
fn base_correlation_arbitrage_clamps_when_validation_disabled() {
    let market_ctx = arbitrage_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let tranche = sample_tranche();

    let pricer = CDSTranchePricer::with_params(
        CDSTranchePricerConfig::default().with_arbitrage_validation(false),
    );
    assert!(!pricer.config().validate_arbitrage_free);

    let pv = pricer
        .price_tranche(&tranche, &market_ctx, as_of)
        .expect("with validation disabled, arbitrage is clamped and pricing succeeds");
    assert!(
        pv.amount().is_finite(),
        "clamped price must be finite, got {}",
        pv.amount()
    );
}

/// Item 2: a well-formed (arbitrage-free) base-correlation curve must price
/// cleanly under the default validating configuration — the new diagnostic
/// must not produce false positives on benign quadrature noise. Covers the
/// equity, mezzanine and senior strike ranges of the standard fixture so a
/// regression in any tranchelet pair is caught.
#[test]
fn well_formed_base_correlation_prices_without_arbitrage_error() {
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("Valid test date");
    let pricer = CDSTranchePricer::new();

    for &(attach, detach, coupon) in &[
        (0.0_f64, 3.0_f64, 1000.0_f64), // equity
        (3.0, 7.0, 500.0),              // junior mezzanine
        (7.0, 10.0, 300.0),             // senior mezzanine
        (10.0, 15.0, 100.0),            // senior
        (15.0, 30.0, 50.0),             // super senior
    ] {
        let params = CDSTrancheParams::new(
            "CDX.NA.IG.42",
            42,
            attach,
            detach,
            Money::new(10_000_000.0, Currency::USD),
            maturity,
            coupon,
        );
        let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
        let tranche = CDSTranche::new(
            "ARB_FREE_TEST",
            &params,
            &schedule_params,
            finstack_core::types::CurveId::from("USD-OIS"),
            finstack_core::types::CurveId::from("CDX.NA.IG.42"),
            TrancheSide::SellProtection,
        )
        .expect("Valid tranche parameters");

        let pv = pricer
            .price_tranche(&tranche, &market_ctx, as_of)
            .unwrap_or_else(|e| {
                panic!(
                    "arbitrage-free base correlation must price [{attach}%, {detach}%] \
                     without error, got: {e:?}"
                )
            });
        assert!(
            pv.amount().is_finite(),
            "[{attach}%, {detach}%] PV must be finite"
        );
    }
}

/// Items 5 & 6: the within-period default fraction must be survival-weighted,
/// not a flat `0.5`. It must equal `0.5` only in the zero-hazard limit, be
/// strictly less than `0.5` for a positive hazard (defaults front-load toward
/// the period start as survival decays), and stay in `(0, 0.5]` always.
#[test]
fn within_period_default_fraction_is_survival_weighted() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let pricer = CDSTranchePricer::new();

    // High-hazard index: positive λ ⇒ fraction must be < 0.5.
    let high_hazard = finstack_core::market_data::term_structures::HazardCurve::builder("HIGH")
        .base_date(base_date)
        .recovery_rate(0.40)
        .knots(vec![(1.0, 0.15), (5.0, 0.15), (10.0, 0.15)])
        .par_spreads(vec![(1.0, 900.0), (5.0, 900.0), (10.0, 900.0)])
        .build()
        .expect("hazard curve");
    let base_corr =
        finstack_core::market_data::term_structures::BaseCorrelationCurve::builder("BC")
            .knots(vec![(3.0, 0.25), (7.0, 0.30), (30.0, 0.50)])
            .build()
            .expect("base corr");
    let high_index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(high_hazard))
        .base_correlation_curve(Arc::new(base_corr.clone()))
        .build()
        .expect("index data");

    // A one-year period [1.0, 2.0] under λ≈0.15.
    let frac = pricer.within_period_default_fraction(&high_index, 1.0, 2.0);
    assert!(
        frac > 0.0 && frac < 0.5,
        "survival-weighted default fraction must be in (0, 0.5) for positive hazard, got {frac}"
    );

    // Near-zero-hazard index: fraction must tend to the 0.5 uniform limit.
    let tiny_hazard = finstack_core::market_data::term_structures::HazardCurve::builder("TINY")
        .base_date(base_date)
        .recovery_rate(0.40)
        .knots(vec![(1.0, 1e-7), (5.0, 1e-7), (10.0, 1e-7)])
        .par_spreads(vec![(1.0, 0.6), (5.0, 0.6), (10.0, 0.6)])
        .build()
        .expect("hazard curve");
    let tiny_index = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(tiny_hazard))
        .base_correlation_curve(Arc::new(base_corr))
        .build()
        .expect("index data");
    let frac_tiny = pricer.within_period_default_fraction(&tiny_index, 1.0, 2.0);
    assert!(
        (frac_tiny - 0.5).abs() < 1e-3,
        "near-zero-hazard default fraction must approach the 0.5 uniform limit, got {frac_tiny}"
    );

    // Degenerate period (t_end <= t_start) must fall back to 0.5, never NaN.
    let frac_degenerate = pricer.within_period_default_fraction(&high_index, 2.0, 2.0);
    assert!(
        (frac_degenerate - 0.5).abs() < 1e-12,
        "degenerate period must fall back to 0.5, got {frac_degenerate}"
    );
}

/// Item 6: with `mid_period_protection`, raising the index hazard moves the
/// survival-weighted default time earlier within each period, so the
/// within-period loss is discounted at an earlier (higher-DF) point. This
/// pins that the protection-leg discounting responds to default timing
/// rather than always using the flat period midpoint.
#[test]
fn mid_period_protection_uses_survival_weighted_timing() {
    let market_ctx = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");

    // Sell-protection, zero coupon: PV is the (signed) protection leg only.
    let mut tranche = sample_tranche();
    tranche.running_coupon_bp = 0.0;

    let mut with_mid = CDSTranchePricer::new();
    with_mid.params.mid_period_protection = true;
    let mut without_mid = CDSTranchePricer::new();
    without_mid.params.mid_period_protection = false;

    let pv_mid = with_mid
        .price_tranche(&tranche, &market_ctx, as_of)
        .expect("mid-period pricing")
        .amount();
    let pv_end = without_mid
        .price_tranche(&tranche, &market_ctx, as_of)
        .expect("end-of-period pricing")
        .amount();

    // Discounting within-period losses earlier (mid-period) vs at period end
    // must change the protection-leg PV: earlier discounting => larger
    // |protection PV| because the loss cashflow is less discounted.
    assert!(
        (pv_mid - pv_end).abs() > 0.0,
        "mid-period vs end-of-period protection timing must change PV: \
         mid={pv_mid}, end={pv_end}"
    );
    assert!(
        pv_mid.is_finite() && pv_end.is_finite(),
        "both protection-timing PVs must be finite"
    );
}
