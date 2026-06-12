//! Discount margin calculator tests.

use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_core::{Error, InputError};
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::fixed_income::bond::DiscountMarginCalculator;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::{MetricCalculator, MetricContext, MetricId};
use std::sync::Arc;
use time::macros::date;

#[test]
fn test_dm_fixed_bond_is_rejected_in_strict_mode() {
    let as_of = date!(2025 - 01 - 01);
    let bond = Bond::fixed(
        "DM1",
        Money::new(100.0, Currency::USD),
        0.05,
        as_of,
        date!(2030 - 01 - 01),
        "USD-OIS",
    )
    .unwrap();

    let curve = finstack_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.80)])
        .build()
        .unwrap();
    let market = finstack_core::market_data::context::MarketContext::new().insert(curve);

    let err = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DiscountMargin],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect_err("discount margin should not be available for fixed-rate bonds");

    match err {
        Error::MetricCalculationFailed { metric_id, .. } => {
            assert_eq!(metric_id, "discount_margin");
        }
        Error::Calibration { message, .. } => {
            assert!(
                message.contains("discount_margin"),
                "wrapped calibration error should mention discount_margin, got: {message}"
            );
            assert!(
                message.contains("DM1"),
                "wrapped calibration error should preserve instrument context, got: {message}"
            );
        }
        other => panic!("unexpected error type: {}", other),
    }
}

/// DM should surface a missing discount curve error instead of silently returning 0.0
/// when pricing fails inside the root-finding objective (e.g., missing discount curve).
#[test]
fn test_dm_missing_forward_curve_returns_error() {
    let as_of = date!(2025 - 01 - 01);

    // Floating-rate bond referencing a discount curve that will be missing in the market
    let bond = Bond::floating(
        "DM-FRN-MISSING-FWD",
        Money::new(100.0, Currency::USD),
        "USD-SOFR-3M",
        200,
        as_of,
        date!(2030 - 01 - 01),
        finstack_core::dates::Tenor::quarterly(),
        finstack_core::dates::DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();

    // Market with NO discount curves – any attempt to price from DM should fail
    let market = finstack_core::market_data::context::MarketContext::new();

    // Build a minimal metric context without relying on successful base pricing;
    // base_value is arbitrary here since we're testing failure in the DM objective.
    let base_value = Money::new(100.0, Currency::USD);
    let mut mctx = MetricContext::new(
        Arc::new(bond),
        Arc::new(market),
        as_of,
        base_value,
        MetricContext::default_config(),
    );

    // No need to pre-compute Accrued; DM calculator will treat missing accrued as 0.
    let calc = DiscountMarginCalculator::default();
    let result = calc.calculate(&mut mctx);

    // Expect a propagated input error (missing curve), never an apparent "perfect fit" DM of 0.0.
    // With FloatingRateFallback::Error (the default), the forward curve lookup fails first.
    match result {
        Err(Error::Input(InputError::MissingCurve { requested, .. })) => {
            assert!(
                requested.contains("USD-OIS") || requested.contains("USD-SOFR"),
                "expected missing curve id to mention USD-OIS or USD-SOFR, got {}",
                requested
            );
        }
        Err(Error::Input(InputError::NotFound { id })) => {
            assert!(
                id.contains("forward curve") || id.contains("USD-SOFR"),
                "expected missing forward curve error, got: {}",
                id
            );
        }
        Err(e) => panic!("expected InputError for missing curve, got {}", e),
        Ok(dm) => panic!(
            "expected DM calculation to fail for missing curve, but got DM={}",
            dm
        ),
    }
}

/// DM solver should converge robustly for IG, HY, and distressed FRNs with
/// realistic spread levels and maintain tight price residuals.
#[test]
fn test_dm_solver_convergence_across_spread_regimes() {
    use finstack_core::dates::{DayCount, Tenor};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::market_data::term_structures::ForwardCurve;
    use finstack_core::math::interp::InterpStyle;
    use finstack_valuations::instruments::PricingOverrides;

    let as_of = date!(2025 - 01 - 01);
    let maturity_ig = date!(2027 - 01 - 01); // short IG
    let maturity_hy = date!(2030 - 01 - 01); // medium HY
    let maturity_distressed = date!(2035 - 01 - 01); // longer distressed
    let notional = Money::new(1_000_000.0, Currency::USD);

    // Simple, monotonic curves suitable for FRN pricing.
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (10.0, 0.6)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (10.0, 0.03)])
        .build()
        .unwrap();
    let market = MarketContext::new().insert(disc).insert(fwd);

    // Base FRNs for different maturities.
    let frn_ig = Bond::floating(
        "DM-CONV-IG",
        notional,
        "USD-SOFR-3M",
        150,
        as_of,
        maturity_ig,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();
    let frn_hy = Bond::floating(
        "DM-CONV-HY",
        notional,
        "USD-SOFR-3M",
        300,
        as_of,
        maturity_hy,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();
    let frn_distressed = Bond::floating(
        "DM-CONV-DIST",
        notional,
        "USD-SOFR-3M",
        500,
        as_of,
        maturity_distressed,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();

    // (target DM, bond) pairs covering IG, HY, and distressed regimes.
    let scenarios: Vec<(f64, Bond)> = vec![
        (0.01, frn_ig),         // 100 bp IG
        (0.07, frn_hy),         // 700 bp HY
        (0.20, frn_distressed), // 2000 bp distressed
    ];

    for (target_dm, base_bond) in scenarios {
        // Price the FRN at the target DM to obtain a dirty price in currency.
        let dirty_target =
            finstack_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_dm(
                &base_bond, &market, as_of, target_dm,
            )
            .expect("pricing with target DM should succeed");

        // Convert to a clean price quote (% of par) assuming valuation on a
        // coupon date (zero accrual).
        let clean_px = dirty_target / notional.amount() * 100.0;

        let mut bond = base_bond.clone();
        bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(clean_px);

        // Run DM metric via the normal metrics pipeline.
        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::DiscountMargin],
                finstack_valuations::instruments::PricingOptions::default(),
            )
            .expect("DM metric should converge for realistic spreads");
        let dm = *result
            .measures
            .get("discount_margin")
            .expect("discount_margin measure should be present");

        // DM should be very close to the target value.
        assert!(
            (dm - target_dm).abs() < 5e-8,
            "DM solver should recover target DM (target={}, got={})",
            target_dm,
            dm
        );

        // Re-price using the solved DM and verify price residual is tiny.
        let dirty_repriced =
            finstack_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_dm(
                &bond, &market, as_of, dm,
            )
            .expect("repricing with solved DM should succeed");
        let price_error = (dirty_repriced - dirty_target).abs() / notional.amount();

        assert!(
            price_error < 1e-6,
            "Price residual should be < 1e-6 * notional, got {}",
            price_error
        );
    }
}

// NOTE: The old test "test_dm_requires_accrued_when_clean_price_present" was removed
// because DM now computes accrued internally via QuoteDateContext per the fix plan.
// DM no longer requires Accrued to be pre-populated in the metric context.
// The test was also using a fixed-rate bond which is not valid for DM anyway.

/// Issue B regression (integration): the DM solver's pricing-failure residual must
/// never change sign across `dm = 0`. This end-to-end test confirms that a solved DM
/// is still meaningful after the fix — the convergence test properties are validated
/// in the unit test `dm_failure_residual_must_not_change_sign_across_zero` inside
/// `dm.rs`.
///
/// This test verifies the happy path still works after the fix: a valid FRN with a
/// healthy market converges to the correct DM and the solved DM round-trips.
#[test]
fn test_dm_monotone_residual_does_not_break_valid_solve() {
    use finstack_core::dates::{DayCount, Tenor};
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_valuations::instruments::PricingOverrides;

    let as_of = date!(2025 - 01 - 01);
    let notional = Money::new(1_000_000.0, Currency::USD);

    let bond = Bond::floating(
        "DM-MONOTONE-HAPPY-PATH",
        notional,
        "USD-SOFR-3M",
        150,
        as_of,
        date!(2028 - 01 - 01),
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
    )
    .expect("bond should build");

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots([(0.0, 1.0), (5.0, 0.85)])
        .build()
        .expect("disc curve");
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (5.0, 0.03)])
        .build()
        .expect("fwd curve");
    let market = MarketContext::new().insert(disc).insert(fwd);

    // Set a clean price close to par so the DM is small and easy to solve.
    let target_dm = 0.015_f64; // 150 bp
    let dirty = finstack_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_dm(
        &bond, &market, as_of, target_dm,
    )
    .expect("price_from_dm with target DM should succeed");
    let clean_px = dirty / notional.amount() * 100.0;

    let mut priced_bond = bond;
    priced_bond.pricing_overrides = PricingOverrides::default().with_quoted_clean_price(clean_px);

    let result = priced_bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DiscountMargin],
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("DM solver should converge for a valid FRN after the monotone-residual fix");

    let dm = result.measures["discount_margin"];
    assert!(
        (dm - target_dm).abs() < 1e-6,
        "DM should round-trip after monotone-residual fix: target={target_dm}, got={dm}"
    );
}
