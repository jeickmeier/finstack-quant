//! Numerical pricing, expected-loss, and sensitivity helpers for CDS tranches.
//!
use super::config::{DiscountAt, DEFAULT_QUADRATURE_ORDER};
use super::*;
use crate::cashflow::primitives::CFKind;
use crate::correlation::copula::CopulaSpec;
use crate::instruments::credit_derivatives::cds_tranche::parameters::CDSTrancheParams;
use crate::instruments::credit_derivatives::cds_tranche::{CDSTranche, TrancheSide};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, HazardCurve};
use finstack_quant_core::math::{binomial_probability, log_factorial, standard_normal_inv_cdf};
use finstack_quant_core::money::Money;
use std::sync::Arc;
use time::Month;

fn sample_market_context() -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    // Create discount curve
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
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

    let mut issuer_curves = finstack_quant_core::HashMap::default();
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
            finstack_quant_core::types::CurveId::from("USD-OIS"),
            finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
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
fn upfront_override_uses_protection_side_and_survives_wipeout() {
    let market = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let upfront = Money::new(125_000.0, Currency::USD);
    let pricer = CDSTranchePricer::new();

    let mut seller = sample_tranche();
    let seller_base = pricer
        .price_tranche(&seller, &market, as_of)
        .expect("seller base");
    seller
        .instrument_pricing_overrides
        .market_quotes
        .upfront_payment = Some(upfront);
    let seller_with = pricer
        .price_tranche(&seller, &market, as_of)
        .expect("seller upfront");
    assert!((seller_with.amount() - seller_base.amount() - upfront.amount()).abs() < 1e-9);

    let mut buyer = seller;
    buyer.side = TrancheSide::BuyProtection;
    buyer
        .instrument_pricing_overrides
        .market_quotes
        .upfront_payment = None;
    let buyer_base = pricer
        .price_tranche(&buyer, &market, as_of)
        .expect("buyer base");
    buyer
        .instrument_pricing_overrides
        .market_quotes
        .upfront_payment = Some(upfront);
    let buyer_with = pricer
        .price_tranche(&buyer, &market, as_of)
        .expect("buyer upfront");
    assert!((buyer_with.amount() - buyer_base.amount() + upfront.amount()).abs() < 1e-9);

    buyer.accumulated_loss = buyer.detach_pct / 100.0;
    assert_eq!(
        pricer
            .price_tranche(&buyer, &market, as_of)
            .expect("wiped-out upfront")
            .amount(),
        -upfront.amount()
    );
}

#[test]
fn projected_schedule_contains_premium_and_default_rows() {
    let tranche = sample_tranche();
    let market = sample_market_context();
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let schedule = CDSTranchePricer::new()
        .build_projected_schedule(&tranche, &market, as_of)
        .expect("projected tranche schedule");

    assert!(schedule
        .get_flows()
        .iter()
        .any(|cf| cf.kind == CFKind::Fixed));
    assert!(schedule
        .get_flows()
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
        finstack_quant_core::types::CurveId::from("USD-OIS"),
        finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
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
        finstack_quant_core::types::CurveId::from("USD-OIS"),
        finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
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
    let mut issuer_curves = finstack_quant_core::HashMap::default();
    for i in 0..10 {
        let id = format!("ISSUER-{:03}", i + 1);
        issuer_curves.insert(id, std::sync::Arc::clone(&index_data.index_credit_curve));
    }
    let hetero_index = CreditIndexData::builder()
        .num_constituents(10)
        .recovery_rate(index_data.recovery_rate)
        .index_credit_curve(std::sync::Arc::clone(&index_data.index_credit_curve))
        .base_correlation_curve(std::sync::Arc::clone(&index_data.base_correlation_curve))
        .issuer_curves(issuer_curves)
        .build()
        .expect("Curve builder should succeed with valid test data");
    let ctx = ctx_base.insert_credit_index("CDX.NA.IG.42", hetero_index);

    let mut homo = CDSTranchePricer::new();
    homo.params.use_issuer_curves = false;
    let mut hetero = CDSTranchePricer::new();
    hetero.params.use_issuer_curves = true;
    hetero.params.hetero_method = HeteroMethod::NormalApprox;

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
        finstack_quant_core::types::CurveId::from("USD-OIS"),
        finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");

    let mut spa = CDSTranchePricer::new();
    spa.params.use_issuer_curves = true;
    spa.params.hetero_method = HeteroMethod::NormalApprox;
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

/// Helper for the audit-M2 regression tests below: price one tranche under a
/// given pricer configuration.
fn price_hetero_tranche(
    ctx: &MarketContext,
    as_of: Date,
    attach: f64,
    detach: f64,
    configure: impl FnOnce(&mut CDSTranchePricer),
) -> f64 {
    let tranche_params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        attach,
        detach,
        Money::new(10_000_000.0, Currency::USD),
        as_of.add_months(60),
        0.0,
    );
    let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
    let tranche = CDSTranche::new(
        "AUDIT_M2_TRANCHE",
        &tranche_params,
        &schedule_params,
        finstack_quant_core::types::CurveId::from("USD-OIS"),
        finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");
    let mut pricer = CDSTranchePricer::new();
    pricer.params.use_issuer_curves = true;
    configure(&mut pricer);
    pricer
        .price_tranche(&tranche, ctx, as_of)
        .expect("Tranche pricing should succeed in test")
        .amount()
}

/// Regression (2026-07 credit-derivatives audit M2): the moment-matched
/// normal approximation mis-prices bespoke pools below
/// `SMALL_POOL_THRESHOLD` (64) by >1% of PV at junior strikes (measured:
/// 1.55% at 24 names on the [3,7] tranche). Such pools must be routed to
/// exact convolution regardless of the configured `hetero_method`, so the
/// default configuration must reproduce the exact-convolution PV to
/// within grid/quadrature noise.
#[test]
fn test_hetero_default_routes_small_pools_to_exact_convolution() {
    let ctx = sample_market_context_with_issuers(24);
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    for (attach, detach) in [(3.0, 7.0), (15.0, 30.0)] {
        // hetero_method deliberately left at its default.
        let pv_default = price_hetero_tranche(&ctx, as_of, attach, detach, |_| {});
        let pv_exact = price_hetero_tranche(&ctx, as_of, attach, detach, |p| {
            p.params.hetero_method = HeteroMethod::ExactConvolution;
        });

        // Same method, same grid → agreement to numerical noise. The old
        // normal-approx path differed by 1.55% ($72k) at [3,7].
        let tol = 1e-6 * pv_exact.abs().max(1.0);
        assert!(
            (pv_default - pv_exact).abs() < tol,
            "default config must route a 24-name pool to exact convolution \
             at [{attach},{detach}]: default={pv_default:.2}, exact={pv_exact:.2}"
        );
    }
}

/// Pin the measured accuracy of the normal (CLT) approximation on the pools
/// it still prices (> `SMALL_POOL_THRESHOLD` names): the audit bias study
/// found 0.03% at 125 names on the junior [3,7] tranche. Allow 0.2% so the
/// pin is robust to fixture drift while still catching a regression to the
/// small-pool bias regime (>1%).
#[test]
fn test_hetero_normal_approx_bias_bound_large_pool() {
    let ctx = sample_market_context_with_issuers(125);
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");

    let pv_normal = price_hetero_tranche(&ctx, as_of, 3.0, 7.0, |p| {
        p.params.hetero_method = HeteroMethod::NormalApprox;
    });
    let pv_exact = price_hetero_tranche(&ctx, as_of, 3.0, 7.0, |p| {
        p.params.hetero_method = HeteroMethod::ExactConvolution;
    });

    let rel = (pv_normal - pv_exact).abs() / pv_exact.abs().max(1.0);
    assert!(
        rel < 0.002,
        "normal-approximation bias on a 125-name pool must stay within the \
         measured bound: normal={pv_normal:.2}, exact={pv_exact:.2}, rel={:.4}%",
        rel * 100.0
    );
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
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
            .build()
            .expect("discount curve");
    let index_curve =
        finstack_quant_core::market_data::term_structures::HazardCurve::builder("CDX.NA.IG.42")
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
        finstack_quant_core::market_data::term_structures::BaseCorrelationCurve::builder("BC")
            .knots(vec![(3.0, 0.25), (7.0, 0.30), (10.0, 0.34), (30.0, 0.50)])
            .build()
            .expect("base corr");

    // Build an index whose 20 issuer curves differ from the index curve by a
    // PD perturbation of `pd_shift` (applied as a hazard-knot bump). With
    // `pd_shift = 0` the pool is exactly homogeneous; with a sub-tolerance
    // shift it must still be classified homogeneous under the unified
    // tolerance.
    let build_ctx = |hazard_shift: f64| {
        let mut issuer_curves = finstack_quant_core::HashMap::default();
        for i in 0..20 {
            let id = format!("ISSUER-{:03}", i + 1);
            let hz = finstack_quant_core::market_data::term_structures::HazardCurve::builder(
                id.as_str(),
            )
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
        finstack_quant_core::types::CurveId::from("USD-OIS"),
        finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters");

    let mut pricer = CDSTranchePricer::new();
    pricer.params.use_issuer_curves = true;
    pricer.params.hetero_method = HeteroMethod::NormalApprox;

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
        finstack_quant_core::types::CurveId::from("USD-OIS"),
        finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
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
            .all(|d| finstack_quant_core::dates::is_cds_date(*d)),
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
            .any(|d| !finstack_quant_core::dates::is_cds_date(*d)),
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

/// `calculate_cs01` is a par-spread (re-bootstrap) bump; without stored par
/// spreads a silent fallback to a hazard-λ bump would mislabel units by
/// ≈1/(1−R), so the solve must fail loudly instead.
#[test]
fn test_cs01_errors_without_par_spreads() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .build()
        .expect("Curve builder should succeed with valid test data");
    // Hazard curve WITHOUT par-spread points.
    let index_curve = HazardCurve::builder("CDX.NA.IG.42")
        .base_date(base_date)
        .recovery_rate(0.40)
        .knots(vec![(1.0, 0.01), (3.0, 0.015), (5.0, 0.02), (10.0, 0.025)])
        .build()
        .expect("Curve builder should succeed with valid test data");
    let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![(3.0, 0.25), (7.0, 0.30), (15.0, 0.40), (30.0, 0.50)])
        .build()
        .expect("Curve builder should succeed with valid test data");
    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(index_curve))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .build()
        .expect("CreditIndexData builder should succeed");
    let market_ctx = MarketContext::new()
        .insert(discount_curve)
        .insert_credit_index("CDX.NA.IG.42", index_data);

    let model = CDSTranchePricer::new();
    let tranche = sample_tranche();
    let as_of = base_date;

    let err = model
        .calculate_cs01(&tranche, &market_ctx, as_of)
        .expect_err("CS01 without par spreads must error, not fall back to a hazard bump");
    assert!(
        err.to_string().contains("par-spread"),
        "error should explain the par-spread requirement: {err}"
    );
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
            finstack_quant_core::market_data::term_structures::BaseCorrelationCurve::builder(
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
            .index_credit_curve(std::sync::Arc::clone(&index_data_arc.index_credit_curve))
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

/// The soft clamp must be value- and slope-continuous (C¹) at both seams:
/// exactly the identity at `min+w` / `max−w`, with unit one-sided slope.
/// The previous tanh construction jumped by ≈ 0.12·w at each seam, which
/// correlation finite differences straddling the seam picked up as a
/// spurious sensitivity.
#[test]
fn test_smooth_correlation_boundary_c1_at_seams() {
    let model = CDSTranchePricer::new();
    let cfg = model.config();
    let (min_c, max_c, w) = (
        cfg.min_correlation,
        cfg.max_correlation,
        cfg.corr_boundary_width,
    );

    for seam in [min_c + w, max_c - w] {
        // Value continuity: at the seam the map must equal the identity.
        let at_seam = model.smooth_correlation_boundary(seam);
        assert!(
            (at_seam - seam).abs() < 1e-12,
            "seam {seam}: smoothing must equal identity, got {at_seam}"
        );

        // C¹: one-sided difference quotients on both sides must both be ~1.
        let h = 1e-7;
        let slope_out = (model.smooth_correlation_boundary(seam)
            - model.smooth_correlation_boundary(seam - h))
            / h;
        let slope_in = (model.smooth_correlation_boundary(seam + h)
            - model.smooth_correlation_boundary(seam))
            / h;
        assert!(
            (slope_out - 1.0).abs() < 1e-4 && (slope_in - 1.0).abs() < 1e-4,
            "seam {seam}: one-sided slopes must both be ~1 (got {slope_out}, {slope_in})"
        );
    }

    // Wings decay toward the bounds and never overshoot them. Far in the
    // wing the exponential underflows to exactly the bound in f64, so the
    // mathematical strict inequality is asserted only near the seam.
    assert!(model.smooth_correlation_boundary(min_c) > min_c);
    assert!(model.smooth_correlation_boundary(max_c) < max_c);
    assert!(model.smooth_correlation_boundary(-0.5) >= min_c);
    assert!(model.smooth_correlation_boundary(0.999_9) <= max_c);
    assert!(model.smooth_correlation_boundary(1.5) <= max_c);
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
        finstack_quant_core::types::CurveId::from("USD-OIS"),
        finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
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
        finstack_quant_core::types::CurveId::from("USD-OIS"),
        finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
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
    let mut test_tranche = tranche;
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
                finstack_quant_core::types::CurveId::from("USD-OIS"),
                finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
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
        finstack_quant_core::types::CurveId::from("USD-OIS"),
        finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
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
            finstack_quant_core::types::CurveId::from("USD-OIS"),
            finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
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

/// C2: homogeneous Gaussian path must not panic when `default_prob` is outside
/// the open interval `(0, 1)`.
///
/// ## Root cause
///
/// `get_default_probability` returns `1.0 - sp(t)` with **no clamp**. If the
/// credit curve's `sp` implementation returns a value marginally above `1.0`
/// (floating-point rounding near the base date is the most common cause), the
/// resulting `default_prob` is marginally **negative**. That negative value
/// flows directly into `standard_normal_inv_cdf(default_prob)`, which calls
/// `statrs::Normal::inverse_cdf`, which **panics** with `"x must be in [0, 1]"`
/// for any argument strictly outside `[0, 1]`.
///
/// The heterogeneous path is safe because it routes probabilities through
/// `default_threshold_for_copula`, which clamps with `PROBABILITY_CLIP`.
/// The homogeneous Gaussian branch lacked that guard.
///
/// ## Fix
///
/// 1. `get_default_probability` clamps its result to `[0.0, 1.0]`.
/// 2. The homogeneous-Gaussian branch clamps `default_prob` to
///    `[PROBABILITY_CLIP, 1 − PROBABILITY_CLIP]` before passing it to
///    `standard_normal_inv_cdf`, matching the heterogeneous branch exactly.
///
/// ## What this test asserts (regression gate)
///
/// The underlying panic source is `statrs::Normal::inverse_cdf` — it panics for
/// any `p ∉ [0, 1]`.  We verify that:
///
/// 1. `standard_normal_inv_cdf` still panics for a negative input (confirming
///    the `statrs` contract has not changed and the guard is load-bearing).
/// 2. The homogeneous pricer path does **not** panic when it internally
///    computes `default_prob = 0.0` (from `sp(0) = 1.0` at the base date).
///    Before the fix the integration returned a non-finite EL (because
///    `standard_normal_inv_cdf(0.0) = −∞` propagated through the quadrature);
///    after the fix the EL is finite.
#[test]
fn homogeneous_path_boundary_default_prob_does_not_panic() {
    // ── Part 1: confirm the probit saturates (no panic) on a negative input ──
    //
    // This is the precise condition that would be triggered in the unfixed pricer
    // when `sp > 1.0` (floating-point rounding near t = 0).
    //
    // `standard_normal_inv_cdf` now guards the
    // raw `statrs` panic itself and saturates out-of-domain inputs
    // (p <= 0 → −∞). The pricer-side clamp remains
    // load-bearing for *numerics* (−∞ would still poison the quadrature), but
    // the panic itself is gone by design.
    assert_eq!(
        standard_normal_inv_cdf(-1e-15_f64),
        f64::NEG_INFINITY,
        "standard_normal_inv_cdf must saturate to -inf (not panic) for p < 0"
    );

    // ── Part 2: homogeneous pricer must not panic and must return finite EL ──
    //
    // When maturity == curve base_date, `years_from_base` = 0, `sp(0)` = 1.0,
    // `default_prob` = 0.0.  Before the fix `standard_normal_inv_cdf(0.0) = −∞`
    // and the Gauss-Hermite integrand evaluates `norm_cdf((−∞ − ρ·z)/√(1−ρ²))`
    // → 0 for all z, collapsing the EL to a finite (but trivial) zero.
    //
    // A more dangerous case — not exercisable with a real HazardCurve in this
    // unit test — is `sp` returning `> 1.0` due to fp-rounding, giving a
    // negative `default_prob`.  Part 1 proves that would panic.  The clamp in
    // `get_default_probability` (`(1.0 - sp).clamp(0.0, 1.0)`) cuts both tails.
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("date");

    let index_curve =
        finstack_quant_core::market_data::term_structures::HazardCurve::builder("CDX.NA.IG.42")
            .base_date(base_date)
            .recovery_rate(0.40)
            .knots(vec![(1.0, 0.01), (5.0, 0.02), (10.0, 0.025)])
            .build()
            .expect("hazard curve");

    let base_corr =
        finstack_quant_core::market_data::term_structures::BaseCorrelationCurve::builder("BC")
            .knots(vec![(3.0, 0.25), (7.0, 0.30), (30.0, 0.50)])
            .build()
            .expect("base corr");

    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(index_curve))
        .base_correlation_curve(Arc::new(base_corr))
        .build()
        .expect("index data");

    // maturity == base_date  ⇒  maturity_years = 0.0  ⇒  sp(0) = 1.0
    // ⇒  default_prob = 0.0  ⇒  without clamp: standard_normal_inv_cdf(0.0) = −∞.
    let maturity = base_date;

    // Drive the homogeneous Gaussian branch directly (disable issuer-curve path).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut p = CDSTranchePricer::new();
        p.params.use_issuer_curves = false;
        p.calculate_equity_tranche_loss(7.0, 0.30, &index_data, maturity)
    }));

    // `Err` here is a `catch_unwind` panic payload, not a domain error.
    #[allow(clippy::match_wild_err_arm)]
    match result {
        Err(_) => panic!(
            "homogeneous Gaussian path panicked for default_prob=0.0 (sp at t=0); \
             the PROBABILITY_CLIP clamp before `standard_normal_inv_cdf` is missing"
        ),
        Ok(Err(e)) => panic!(
            "homogeneous path returned an unexpected error for boundary default_prob=0.0: {e}"
        ),
        Ok(Ok(el)) => {
            assert!(
                el.is_finite(),
                "expected loss must be finite after clamping default_prob to PROBABILITY_CLIP; \
                 if non-finite, the clamp before standard_normal_inv_cdf is absent, got {el}"
            );
            assert!(
                (0.0..=1.0).contains(&el),
                "expected loss must be in [0,1] for boundary default_prob=0.0, got {el}"
            );
        }
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
    let high_hazard =
        finstack_quant_core::market_data::term_structures::HazardCurve::builder("HIGH")
            .base_date(base_date)
            .recovery_rate(0.40)
            .knots(vec![(1.0, 0.15), (5.0, 0.15), (10.0, 0.15)])
            .par_spreads(vec![(1.0, 900.0), (5.0, 900.0), (10.0, 900.0)])
            .build()
            .expect("hazard curve");
    let base_corr =
        finstack_quant_core::market_data::term_structures::BaseCorrelationCurve::builder("BC")
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
    let tiny_hazard =
        finstack_quant_core::market_data::term_structures::HazardCurve::builder("TINY")
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

/// discounting must be RELATIVE to `as_of` on the discount
/// curve's own time axis, never an absolute `df(t)` with `t` measured on the
/// hazard curve's axis. Pins this via re-basing invariance: two discount
/// curves describing the SAME flat continuously-compounded rate but with
/// base dates one year apart must produce identical tranche PVs. Under the
/// old absolute-DF lookup the early-based curve's stale year of discounting
/// leaked into every cashflow.
#[test]
fn discounting_is_invariant_under_curve_rebasing() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let early_base = Date::from_calendar_date(2024, Month::January, 1).expect("date");
    let rate = 0.04_f64;

    // Flat-rate curve as exp(-r t) knots on each curve's own axis.
    let make_disc = |base: Date| {
        let knots: Vec<(f64, f64)> = (0..=12)
            .map(|y| (y as f64, (-rate * y as f64).exp()))
            .collect();
        DiscountCurve::builder("USD-OIS")
            .base_date(base)
            .knots(knots)
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .build()
            .expect("discount curve")
    };

    let index_curve = HazardCurve::builder("CDX.NA.IG.42")
        .base_date(as_of)
        .recovery_rate(0.40)
        .knots(vec![(1.0, 0.01), (3.0, 0.015), (5.0, 0.02), (10.0, 0.025)])
        .par_spreads(vec![(1.0, 60.0), (3.0, 80.0), (5.0, 100.0), (10.0, 140.0)])
        .build()
        .expect("hazard curve");
    let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![
            (3.0, 0.25),
            (7.0, 0.30),
            (10.0, 0.34),
            (15.0, 0.40),
            (30.0, 0.50),
        ])
        .build()
        .expect("base corr");
    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(0.40)
        .index_credit_curve(Arc::new(index_curve))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .build()
        .expect("index data");

    let make_ctx = |base: Date| {
        MarketContext::new()
            .insert(make_disc(base))
            .insert_credit_index("CDX.NA.IG.42", index_data.clone())
    };

    let pricer = CDSTranchePricer::new();
    let tranche = sample_tranche();

    let pv_asof_based = pricer
        .price_tranche(&tranche, &make_ctx(as_of), as_of)
        .expect("pricing with as_of-based curve")
        .amount();
    let pv_early_based = pricer
        .price_tranche(&tranche, &make_ctx(early_base), as_of)
        .expect("pricing with early-based curve")
        .amount();

    let tol = 1e-9 * pv_asof_based.abs().max(1.0);
    assert!(
        (pv_asof_based - pv_early_based).abs() < tol,
        "tranche PV must be invariant under discount-curve re-basing: \
         as_of-based={pv_asof_based}, early-based={pv_early_based}"
    );
}

/// Premium leg PV (sell-protection sign: positive) for a given pricer config.
fn premium_leg_pv(pricer: &CDSTranchePricer, tranche: &CDSTranche, ctx: &MarketContext) -> f64 {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let discount = ctx
        .get_discount(tranche.discount_curve_id.as_ref())
        .expect("discount curve");
    let rows = pricer
        .project_discountable_rows(tranche, ctx, as_of)
        .expect("projected rows");
    pricer
        .discount_projected_rows(
            &rows
                .iter()
                .filter(|row| row.cashflow.kind == CFKind::Fixed)
                .cloned()
                .collect::<Vec<_>>(),
            discount.as_ref(),
            as_of,
        )
        .expect("premium PV")
}

/// the accrual-on-default adjustment must be the COMPLEMENT
/// of the survival-weighted default fraction. A name defaulting at fraction
/// `f` of the period pays accrued `f·Δ`, so the premium notional lost on the
/// defaulted slice is `(1−f)·ΔEL` when AoD is enabled and the full `ΔEL`
/// when disabled (defaulted names pay nothing for the period).
///
/// Pins the ordering: premium(AoD enabled) > premium(AoD disabled), because
/// the AoD credit recovers the `f·Δ` accrued by defaulters.
#[test]
fn aod_enabled_premium_exceeds_disabled_premium() {
    let market_ctx = sample_market_context();
    let tranche = sample_tranche(); // sell protection, 500bp coupon

    let mut enabled = CDSTranchePricer::new();
    enabled.params.accrual_on_default_enabled = true;
    let mut disabled = CDSTranchePricer::new();
    disabled.params.accrual_on_default_enabled = false;

    let pv_enabled = premium_leg_pv(&enabled, &tranche, &market_ctx);
    let pv_disabled = premium_leg_pv(&disabled, &tranche, &market_ctx);

    assert!(
        pv_enabled > 0.0 && pv_disabled > 0.0,
        "sell-protection premium legs must be positive: enabled={pv_enabled}, \
         disabled={pv_disabled}"
    );
    assert!(
        pv_enabled > pv_disabled,
        "AoD-enabled premium must exceed AoD-disabled premium (defaulters pay \
         partial accrual): enabled={pv_enabled}, disabled={pv_disabled}"
    );
}

/// rising index hazard must LOWER the premium-leg PV — more
/// defaults mean less premium notional. Before the fix the complement-swapped
/// adjustment under-reduced (AoD on) or never reduced (AoD off) the premium,
/// muting this direction.
#[test]
fn rising_hazard_lowers_premium_leg_pv() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let tranche = sample_tranche();

    let build_ctx = |hazard_scale: f64| {
        let discount_curve = DiscountCurve::builder("USD-OIS")
            .base_date(base_date)
            .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .build()
            .expect("discount curve");
        let index_curve = HazardCurve::builder("CDX.NA.IG.42")
            .base_date(base_date)
            .recovery_rate(0.40)
            .knots(vec![
                (1.0, 0.01 * hazard_scale),
                (3.0, 0.015 * hazard_scale),
                (5.0, 0.02 * hazard_scale),
                (10.0, 0.025 * hazard_scale),
            ])
            .par_spreads(vec![(1.0, 60.0), (3.0, 80.0), (5.0, 100.0), (10.0, 140.0)])
            .build()
            .expect("hazard curve");
        let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
            .knots(vec![
                (3.0, 0.25),
                (7.0, 0.30),
                (10.0, 0.34),
                (15.0, 0.40),
                (30.0, 0.50),
            ])
            .build()
            .expect("base corr");
        let index_data = CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.40)
            .index_credit_curve(Arc::new(index_curve))
            .base_correlation_curve(Arc::new(base_corr_curve))
            .build()
            .expect("index data");
        MarketContext::new()
            .insert(discount_curve)
            .insert_credit_index("CDX.NA.IG.42", index_data)
    };

    for aod_enabled in [true, false] {
        let mut pricer = CDSTranchePricer::new();
        pricer.params.accrual_on_default_enabled = aod_enabled;

        let pv_low = premium_leg_pv(&pricer, &tranche, &build_ctx(1.0));
        let pv_high = premium_leg_pv(&pricer, &tranche, &build_ctx(3.0));
        assert!(
            pv_high < pv_low,
            "rising hazard must lower premium PV (aod_enabled={aod_enabled}): \
             low-hazard={pv_low}, high-hazard={pv_high}"
        );
    }
}

/// Helper: market context with configurable index recovery
/// and hazard scale.
fn recovery_market_context(recovery: f64, hazard_scale: f64) -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .build()
        .expect("discount curve");
    let index_curve = HazardCurve::builder("CDX.NA.IG.42")
        .base_date(base_date)
        .recovery_rate(recovery)
        .knots(vec![
            (1.0, 0.01 * hazard_scale),
            (3.0, 0.015 * hazard_scale),
            (5.0, 0.02 * hazard_scale),
            (10.0, 0.025 * hazard_scale),
        ])
        .par_spreads(vec![(1.0, 60.0), (3.0, 80.0), (5.0, 100.0), (10.0, 140.0)])
        .build()
        .expect("hazard curve");
    let base_corr_curve = BaseCorrelationCurve::builder("CDX.NA.IG.42_5Y")
        .knots(vec![
            (3.0, 0.25),
            (7.0, 0.30),
            (10.0, 0.34),
            (15.0, 0.40),
            (30.0, 0.50),
        ])
        .build()
        .expect("base corr");
    let index_data = CreditIndexData::builder()
        .num_constituents(125)
        .recovery_rate(recovery)
        .index_credit_curve(Arc::new(index_curve))
        .base_correlation_curve(Arc::new(base_corr_curve))
        .build()
        .expect("index data");
    MarketContext::new()
        .insert(discount_curve)
        .insert_credit_index("CDX.NA.IG.42", index_data)
}

fn super_senior_tranche(attach: f64, detach: f64) -> CDSTranche {
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("date");
    let params = CDSTrancheParams::new(
        "CDX.NA.IG.42",
        42,
        attach,
        detach,
        Money::new(10_000_000.0, Currency::USD),
        maturity,
        25.0,
    );
    let schedule_params = crate::cashflow::builder::ScheduleParams::quarterly_act360();
    CDSTranche::new(
        "SS_TEST",
        &params,
        &schedule_params,
        finstack_quant_core::types::CurveId::from("USD-OIS"),
        finstack_quant_core::types::CurveId::from("CDX.NA.IG.42"),
        TrancheSide::SellProtection,
    )
    .expect("Valid tranche parameters")
}

/// At zero recovery there is no recovered notional: senior writedown is zero
/// and the defaulted fraction equals the loss (`X = L`).
#[test]
fn zero_recovery_has_no_senior_writedown() {
    let ctx = recovery_market_context(0.0, 1.0);
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let pricer = CDSTranchePricer::new();
    let tranche = super_senior_tranche(60.0, 100.0);

    let index_data = ctx.get_credit_index("CDX.NA.IG.42").expect("credit index");
    let dates = pricer
        .generate_payment_schedule(&tranche, as_of)
        .expect("schedule");
    let curve = pricer
        .build_el_wd_curve(&tranche, &index_data, &dates)
        .expect("EL/WD curve");
    for p in &curve {
        assert!(
            p.wd_fraction.abs() < 1e-15,
            "R=0 must produce zero senior writedown, got {} at {:?}",
            p.wd_fraction,
            p.date
        );
    }

    // Realized state: X = L when R = 0.
    let mut seasoned = tranche;
    seasoned.accumulated_loss = 0.05;
    let (defaulted, recovered) = pricer.realized_default_state(&seasoned, 0.0);
    assert!((defaulted - 0.05).abs() < 1e-12, "X must equal L at R=0");
    assert!(recovered.abs() < 1e-15, "G must be zero at R=0");
}

/// Regression: a super-senior tranche has near-zero
/// expected LOSS, but rising hazard amortizes its notional from the top via
/// recoveries — so its premium leg must FALL as hazard rises. Before the fix
/// the detachment was never written down and the super-senior kept paying
/// full premium after defaults.
#[test]
fn super_senior_premium_falls_as_hazard_rises() {
    let tranche = super_senior_tranche(60.0, 100.0);
    let pricer = CDSTranchePricer::new();

    let pv_low = premium_leg_pv(&pricer, &tranche, &recovery_market_context(0.40, 1.0));
    let pv_high = premium_leg_pv(&pricer, &tranche, &recovery_market_context(0.40, 5.0));

    assert!(
        pv_low > 0.0 && pv_high > 0.0,
        "super-senior premium legs must be positive: low={pv_low}, high={pv_high}"
    );
    assert!(
        pv_high < pv_low,
        "rising hazard must erode super-senior premium notional via recovery \
         writedown: low-hazard={pv_low}, high-hazard={pv_high}"
    );

    // The writedown curve itself must be positive and monotone for R > 0.
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let ctx = recovery_market_context(0.40, 5.0);
    let index_data = ctx.get_credit_index("CDX.NA.IG.42").expect("index");
    let dates = pricer
        .generate_payment_schedule(&tranche, as_of)
        .expect("schedule");
    let curve = pricer
        .build_el_wd_curve(&tranche, &index_data, &dates)
        .expect("EL/WD curve");
    let final_point = curve.last().expect("non-empty curve");
    assert!(
        final_point.wd_fraction > 0.0,
        "super-senior writedown at maturity must be positive for R>0, got {}",
        final_point.wd_fraction
    );
    for w in curve.windows(2) {
        assert!(
            w[1].wd_fraction >= w[0].wd_fraction - 1e-12,
            "writedown curve must be monotone"
        );
        assert!(
            w[1].el_fraction + w[1].wd_fraction <= 1.0 + 1e-12,
            "el + wd must never exceed 1"
        );
    }
}

/// Regression: hand-computed seasoned case. With
/// `accumulated_loss = 6%` and `R = 40%`: defaulted `X = 0.06/0.6 = 10%`,
/// recovered `G = 4%`, pool factor `0.9`. A `[95%, 100%]` super-senior is
/// written down from the top by `G = 4%` of the pool = 80% of its 5% width;
/// effective detach erodes to `1 − G`.
#[test]
fn seasoned_recovery_writedown_matches_hand_computation() {
    let pricer = CDSTranchePricer::new();
    let mut tranche = super_senior_tranche(95.0, 100.0);
    tranche.accumulated_loss = 0.06;
    let recovery = 0.40;

    let (defaulted, recovered) = pricer.realized_default_state(&tranche, recovery);
    assert!((defaulted - 0.10).abs() < 1e-12, "X = L/(1−R) = 0.10");
    assert!((recovered - 0.04).abs() < 1e-12, "G = X·R = 0.04");

    let prior_wd = pricer.calculate_prior_tranche_writedown(&tranche, recovery);
    assert!(
        (prior_wd - 0.80).abs() < 1e-12,
        "writedown fraction must be (0.04 − 0)/0.05 = 0.80, got {prior_wd}"
    );
    // No loss reaches a 95% attachment at L = 6%.
    assert!(pricer.calculate_prior_tranche_loss(&tranche).abs() < 1e-15);

    let eff = pricer.calculate_effective_structure(&tranche, recovery);
    assert!((eff.pool_factor - 0.90).abs() < 1e-12, "pool factor 1 − X");
    let expected_detach = ((1.0 - recovered).min(1.0) - 0.06) / 0.90;
    assert!(
        (eff.eff_detach - expected_detach).abs() < 1e-12,
        "effective detach must erode by recovered notional: got {}, want {}",
        eff.eff_detach,
        expected_detach
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

#[test]
fn seasoned_first_period_default_timing_starts_at_valuation_date() {
    let market_ctx = sample_market_context();
    let index_data = market_ctx
        .get_credit_index("CDX.NA.IG.42")
        .expect("test index data");
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
    let effective_date = Date::from_calendar_date(2024, Month::December, 20).expect("date");
    let mut tranche = sample_tranche();
    tranche.effective_date = Some(effective_date);
    tranche.running_coupon_bp = 0.0;

    let pricer = CDSTranchePricer::new();
    let rows = pricer
        .project_discountable_rows(&tranche, &market_ctx, as_of)
        .expect("projected rows");
    let default_row = rows
        .iter()
        .find(|row| row.cashflow.kind == CFKind::DefaultedNotional)
        .expect("default row");

    let DiscountAt::WithinPeriod { start, fraction } = default_row.discount_at else {
        panic!("mid-period protection should discount default row within period");
    };
    assert_eq!(start, as_of);

    let t_start = pricer
        .years_from_base(&index_data, as_of)
        .expect("valuation-date time");
    let t_end = pricer
        .years_from_base(&index_data, default_row.cashflow.date)
        .expect("payment-date time");
    let expected = pricer.within_period_default_fraction(&index_data, t_start, t_end);
    assert!(
        (fraction - expected).abs() < 1e-12,
        "seasoned first-period default fraction must start from valuation date: \
         got={fraction}, expected={expected}"
    );
}

#[test]
fn student_t_recovery_driver_uses_scaled_market_factor() {
    let student_t = CDSTranchePricer::with_params(
        CDSTranchePricerConfig::default()
            .with_student_t_copula(5.0)
            .expect("valid Student-t copula"),
    );
    let gaussian = CDSTranchePricer::new();

    assert_eq!(
        student_t.params.copula_spec,
        CopulaSpec::student_t(5.0).expect("valid")
    );
    assert!(
        (student_t.recovery_driver_for_factors(&[2.0, 4.0]) - 1.0).abs() < 1e-12,
        "Student-t recovery driver must use Z/sqrt(W)"
    );
    assert!(
        (gaussian.recovery_driver_for_factors(&[2.0, 4.0]) - 2.0).abs() < 1e-12,
        "Gaussian recovery driver must remain the first factor"
    );
}

/// EL-consistency of the stochastic-recovery override with the bootstrapped
/// index curve: the 0–100% equity tranche must reproduce the index expected
/// loss `p · (1 − R_base)` even when recovery varies with the systematic
/// factor. Without renormalization, the factor-dependence of `R(Z)` makes
/// `E_Z[p(Z)·(1−R(Z))] ≠ p·(1−R_base)` and the sum of all tranches drifts
/// away from the index.
#[test]
fn test_stochastic_recovery_full_pool_el_matches_index() {
    let market_ctx = sample_market_context();
    let index_data = market_ctx
        .get_credit_index("CDX.NA.IG.42")
        .expect("test index data");
    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("date");

    let pricer_stoch =
        CDSTranchePricer::with_params(CDSTranchePricerConfig::default().with_stochastic_recovery());

    // Index-implied expected loss with the flat bootstrapped recovery.
    let t = pricer_stoch
        .years_from_base(&index_data, maturity)
        .expect("year fraction");
    let p = pricer_stoch
        .get_default_probability(&index_data, t)
        .expect("default probability");
    let index_el = p * (1.0 - index_data.recovery_rate);

    // 0–100% equity tranche: cap never binds, so this is the full pool EL.
    let full_pool_el = pricer_stoch
        .calculate_equity_tranche_loss(100.0, 0.30, &index_data, maturity)
        .expect("full-pool equity tranche EL");

    let rel_err = (full_pool_el - index_el).abs() / index_el;
    assert!(
        rel_err < 1e-6,
        "0–100% tranche EL with stochastic recovery must match the index EL: \
         tranche={full_pool_el}, index={index_el}, rel_err={rel_err}"
    );

    // The renormalization must not flatten the override into constant
    // recovery: a strict sub-pool tranche (where the cap binds and the
    // z-shape matters) must still differ from the constant-recovery pricer.
    let pricer_const = CDSTranchePricer::new();
    let equity_stoch = pricer_stoch
        .calculate_equity_tranche_loss(3.0, 0.25, &index_data, maturity)
        .expect("stochastic equity EL");
    let equity_const = pricer_const
        .calculate_equity_tranche_loss(3.0, 0.25, &index_data, maturity)
        .expect("constant equity EL");
    assert!(
        (equity_stoch - equity_const).abs() > 1e-12,
        "stochastic recovery should still reshape sub-pool tranche EL: \
         stoch={equity_stoch}, const={equity_const}"
    );
}
