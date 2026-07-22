//! Discount margin calculator tests.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::{Error, InputError};
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::bond::DiscountMarginCalculator;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::{MetricCalculator, MetricContext, MetricId};
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

    let curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .build()
            .unwrap();
    let market = finstack_quant_core::market_data::context::MarketContext::new().insert(curve);

    let err = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DiscountMargin],
            finstack_quant_valuations::instruments::PricingOptions::default(),
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
        finstack_quant_core::dates::Tenor::quarterly(),
        finstack_quant_core::dates::DayCount::Act360,
        "USD-OIS",
    )
    .unwrap();

    // Market with NO discount curves – any attempt to price from DM should fail
    let market = finstack_quant_core::market_data::context::MarketContext::new();

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
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_valuations::instruments::InstrumentPricingOverrides;

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
        .base_date(date!(2024 - 12 - 30))
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
            finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_dm(
                &base_bond, &market, as_of, target_dm,
            )
            .expect("pricing with target DM should succeed");

        // Convert to a clean price quote (% of par) assuming valuation on a
        // coupon date (zero accrual).
        let clean_px = dirty_target / notional.amount() * 100.0;

        let mut bond = base_bond.clone();
        bond.instrument_pricing_overrides =
            InstrumentPricingOverrides::default().with_quoted_clean_price(clean_px);

        // Run DM metric via the normal metrics pipeline.
        let result = bond
            .price_with_metrics(
                &market,
                as_of,
                &[MetricId::DiscountMargin],
                finstack_quant_valuations::instruments::PricingOptions::default(),
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
            finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_dm(
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

/// Build a flat, self-consistent FRN market: the discount curve and the
/// projection curve both sit at `rate` (quarterly compounding, Act/360), so the
/// solved curve DM of an FRN priced at par must equal its quoted margin.
fn flat_frn_market(
    as_of: time::Date,
    rate: f64,
) -> finstack_quant_core::market_data::context::MarketContext {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_core::math::interp::InterpStyle;
    // DF(t) = (1 + rate/4)^(-4t) on the Act/360 axis. With log-linear
    // interpolation two knots reproduce the exponential exactly, so the
    // periodically-compounded zero rate at m=4 equals `rate` at every t.
    let df_10y = (1.0 + rate / 4.0_f64).powf(-40.0);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .interp(InterpStyle::LogLinear)
        .knots([(0.0, 1.0), (10.0, df_10y)])
        .build()
        .expect("flat discount curve should build");
    // Base the projection curve a few days before `as_of` so the first
    // coupon's lagged reset date is still projectable (no fixings needed).
    let fwd = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(date!(2024 - 12 - 28))
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (10.0, rate)])
        .build()
        .expect("flat forward curve should build");
    MarketContext::new().insert(disc).insert(fwd)
}

fn flat_market_frn(as_of: time::Date, maturity: time::Date, margin_bp: i32) -> Bond {
    Bond::floating(
        "DM-PAR-PIN",
        Money::new(1_000_000.0, Currency::USD),
        "USD-SOFR-3M",
        margin_bp,
        as_of,
        maturity,
        finstack_quant_core::dates::Tenor::quarterly(),
        finstack_quant_core::dates::DayCount::Act360,
        "USD-OIS",
    )
    .expect("FRN construction should succeed")
}

fn solve_dm_at_clean_price(as_of: time::Date, margin_bp: i32, clean_px: f64) -> f64 {
    use finstack_quant_valuations::instruments::InstrumentPricingOverrides;
    let mut bond = flat_market_frn(as_of, date!(2027 - 01 - 01), margin_bp);
    bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(clean_px);
    let market = flat_frn_market(as_of, 0.03);
    let result = bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DiscountMargin],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("DM metric should converge on a flat consistent market");
    result.measures["discount_margin"]
}

/// Pinned external value (Fabozzi / Bloomberg YAS convention): an FRN quoted
/// at par on a flat curve, with the discount curve equal to the projection
/// curve, must solve to a DM equal to its quoted margin — not zero and not
/// its negative. Small residual tolerance covers day-count/period-compounding
/// effects (Act/360 quarterly periods vs the m=4 zero-rate shift).
#[test]
fn test_dm_at_par_equals_quoted_margin_on_flat_consistent_curve() {
    let as_of = date!(2025 - 01 - 01);
    let quoted_margin = 0.01; // 100 bp
    let dm = solve_dm_at_clean_price(as_of, 100, 100.0);
    assert!(
        (dm - quoted_margin).abs() < 2e-4,
        "FRN at par on a flat consistent curve must have DM ~= quoted margin \
         (expected ~{quoted_margin}, got {dm})"
    );
}

/// Sign test: PV must be decreasing in DM. An FRN quoted below par must solve
/// to a DM strictly above its quoted margin; above par, strictly below.
#[test]
fn test_dm_sign_convention_below_and_above_par() {
    let as_of = date!(2025 - 01 - 01);
    let quoted_margin = 0.01; // 100 bp

    let dm_discount = solve_dm_at_clean_price(as_of, 100, 98.0);
    assert!(
        dm_discount > quoted_margin,
        "FRN quoted below par must carry DM strictly above its quoted margin \
         (quoted margin {quoted_margin}, got {dm_discount})"
    );

    let dm_premium = solve_dm_at_clean_price(as_of, 100, 102.0);
    assert!(
        dm_premium < quoted_margin,
        "FRN quoted above par must carry DM strictly below its quoted margin \
         (quoted margin {quoted_margin}, got {dm_premium})"
    );

    // And the two must straddle the at-par DM with economically meaningful width.
    assert!(
        dm_discount - dm_premium > 1e-3,
        "DM spread between 98 and 102 quotes should exceed 10 bp, got {}",
        dm_discount - dm_premium
    );
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
    use finstack_quant_core::dates::{DayCount, Tenor};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use finstack_quant_valuations::instruments::InstrumentPricingOverrides;

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
        .base_date(date!(2024 - 12 - 30))
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (5.0, 0.03)])
        .build()
        .expect("fwd curve");
    let market = MarketContext::new().insert(disc).insert(fwd);

    // Set a clean price close to par so the DM is small and easy to solve.
    let target_dm = 0.015_f64; // 150 bp
    let dirty = finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::price_from_dm(
        &bond, &market, as_of, target_dm,
    )
    .expect("price_from_dm with target DM should succeed");
    let clean_px = dirty / notional.amount() * 100.0;

    let mut priced_bond = bond;
    priced_bond.instrument_pricing_overrides =
        InstrumentPricingOverrides::default().with_quoted_clean_price(clean_px);

    let result = priced_bond
        .price_with_metrics(
            &market,
            as_of,
            &[MetricId::DiscountMargin],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("DM solver should converge for a valid FRN after the monotone-residual fix");

    let dm = result.measures["discount_margin"];
    assert!(
        (dm - target_dm).abs() < 1e-6,
        "DM should round-trip after monotone-residual fix: target={target_dm}, got={dm}"
    );
}
