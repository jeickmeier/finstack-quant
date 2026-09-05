//! Analytical self-consistency tests for P&L attribution.
//!
//! **Scope clarification** (audit MO6): these tests are NOT an external
//! QuantLib parity. They cross-check attribution output against the library's
//! own analytical DV01 / Convexity formulas — useful as a self-consistency
//! guard against internal regression, but not a substitute for vendor
//! validation. A real QuantLib parity (with a committed fixture of
//! QuantLib-computed DV01 / Convexity / Carry for a canonical bond) is a
//! separate deferred effort.
//!
//! For each bond instrument, we use the analytical DV01 formula to verify
//! that rates P&L matches expected sensitivity.
//!
//! ## Reference Formulas
//!
//! ### Bond DV01 (Central Bump)
//!
//! For a bond with price P, DV01 is approximated by a 1bp central difference:
//!   DV01 ≈ (P_down - P_up) / 2
//!
//! Convexity is approximated with a second-difference:
//!   Convexity_cash ≈ (P_up + P_down - 2P_base) / (Δr)^2
//!
//! ### Rates P&L Attribution
//!
//! For parallel rate shift Δr (in decimal), with DV01 = (P_down − P_up)/2 > 0
//! for a long bond, the signed second-order reference is:
//!   Rates_PnL ≈ −DV01 × (Δr × 10,000) + ½ × Convexity_cash × (Δr)²
//!
//! ## Tolerances
//!
//! Per-case, calibrated so the test fails if the convexity term is dropped
//! from the reference (tolerance < convexity term's relative contribution);
//! see the `AnalyticalParityTestCase` constructors for measured values.

use finstack_quant_attribution::{
    attribute_pnl, AttributionMethod, AttributionRequest, ExecutionPolicy,
};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::create_date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use std::sync::Arc;
use time::Month;

/// Helper to compute discount factor from rate and time.
fn df_from_rate(rate: f64, years: f64) -> f64 {
    (-rate * years).exp()
}

/// Test case configuration for analytical parity.
struct AnalyticalParityTestCase {
    name: &'static str,
    notional: f64,
    coupon_rate: f64,
    maturity_years: u32,
    rate_t0: f64,
    rate_t1: f64,
    /// Expected relative error tolerance for first-order approximation
    tolerance_pct: f64,
}

impl AnalyticalParityTestCase {
    // Tolerances are calibrated (2026-08-12 measurements) so that DELETING the
    // convexity term from the signed reference would fail the test: each
    // tolerance sits strictly between the measured agreement and the convexity
    // term's relative contribution.

    fn new_small_rate_increase() -> Self {
        Self {
            name: "5Y bond, 10bp rate increase",
            notional: 1_000_000.0,
            coupon_rate: 0.05,
            maturity_years: 5,
            rate_t0: 0.04,
            rate_t1: 0.041, // 10bp increase
            // Measured rel_diff 0.027%; convexity term is 0.24% of expected,
            // so 0.1% passes today and fails a convexity-less reference.
            tolerance_pct: 0.1,
        }
    }

    fn new_large_rate_increase() -> Self {
        Self {
            name: "5Y bond, 100bp rate increase",
            notional: 1_000_000.0,
            coupon_rate: 0.05,
            maturity_years: 5,
            rate_t0: 0.04,
            rate_t1: 0.05, // 100bp increase
            // Measured rel_diff 0.043%; convexity term is 2.5% of expected,
            // so 1% passes today and fails a convexity-less reference by >2×.
            tolerance_pct: 1.0,
        }
    }

    fn new_rate_decrease() -> Self {
        Self {
            name: "5Y bond, 50bp rate decrease",
            notional: 1_000_000.0,
            coupon_rate: 0.05,
            maturity_years: 5,
            rate_t0: 0.04,
            rate_t1: 0.035, // 50bp decrease
            // Measured rel_diff 0.005%; convexity term is 1.2% of expected,
            // so 0.5% passes today and fails a convexity-less reference.
            tolerance_pct: 0.5,
        }
    }
}

/// Compute DV01 and convexity using a 1bp central bump.
fn compute_bumped_sensitivities(
    instrument: &dyn Instrument,
    as_of: time::Date,
    curve_id: &str,
    base_rate: f64,
) -> (f64, f64, f64) {
    let bump = 0.0001; // 1bp
    let curve_base = build_flat_curve(curve_id, as_of, base_rate);
    let curve_up = build_flat_curve(curve_id, as_of, base_rate + bump);
    let curve_down = build_flat_curve(curve_id, as_of, base_rate - bump);

    let market_base = MarketContext::new().insert(curve_base);
    let market_up = MarketContext::new().insert(curve_up);
    let market_down = MarketContext::new().insert(curve_down);

    let price_base = instrument.value(&market_base, as_of).unwrap().amount();
    let price_up = instrument.value(&market_up, as_of).unwrap().amount();
    let price_down = instrument.value(&market_down, as_of).unwrap().amount();

    let dv01 = (price_down - price_up) * 0.5;
    let convexity_cash = (price_up + price_down - 2.0 * price_base) / (bump * bump);

    (price_base, dv01, convexity_cash)
}

/// Build a flat discount curve at the given rate.
fn build_flat_curve(curve_id: &str, as_of: time::Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0, 20.0, 30.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, df_from_rate(rate, t))).collect();

    DiscountCurve::builder(curve_id)
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

fn run_analytical_parity_test(tc: &AnalyticalParityTestCase) {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    // Avoid New Year's Day so USNY Modified Following does not roll coupons
    // and distort the Taylor DV01/convexity reference.
    let issue = create_date(2025, Month::January, 15).unwrap();
    let maturity = create_date(2025 + tc.maturity_years as i32, Month::January, 15).unwrap();

    let bond = Bond::fixed(
        "PARITY-TEST-BOND",
        Money::new(tc.notional, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(tc.coupon_rate).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();

    // Build markets at T0 and T1 with different rates
    let curve_t0 = build_flat_curve("USD-OIS", as_of_t0, tc.rate_t0);
    let curve_t1 = build_flat_curve("USD-OIS", as_of_t1, tc.rate_t1);

    let market_t0 = MarketContext::new().insert(curve_t0);
    let market_t1 = MarketContext::new().insert(curve_t1);

    let config = FinstackConfig::default();

    let (_price_base, dv01, convexity_cash) =
        compute_bumped_sensitivities(&bond, as_of_t1, "USD-OIS", tc.rate_t0);

    // Run attribution
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);
    let attribution = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &bond_instrument,
                &market_t0,
                &market_t1,
                as_of_t0,
                as_of_t1,
                &config,
            )
        },
    )
    .unwrap();

    let rate_change_decimal = tc.rate_t1 - tc.rate_t0;
    let rate_change_bp = rate_change_decimal * 10_000.0;
    // Signed second-order reference: dP ≈ −DV01·Δr_bp + ½·Γ_cash·Δr².
    // `dv01 = (P_down − P_up)/2` is positive for a long bond, so the
    // first-order price move for a rate INCREASE is −dv01·Δr_bp; the
    // (always-positive for a vanilla bond) convexity term then cushions
    // losses on the way up and amplifies gains on the way down. The old
    // reference added the convexity term to the wrong side (`+dv01·Δr_bp +
    // convexity`), which under the magnitude-only comparison shifted the
    // reference AWAY from the true value by 2× the convexity term — the 5%/10%
    // tolerances existed to absorb that self-inflicted error.
    let convexity_term = 0.5 * convexity_cash * rate_change_decimal * rate_change_decimal;
    let expected_rates_pnl = -dv01 * rate_change_bp + convexity_term;

    let actual_rates_pnl = attribution.rates_curves_pnl.amount();

    // Verify directionality: rates up → bond value down → negative P&L
    if tc.rate_t1 > tc.rate_t0 {
        assert!(
            actual_rates_pnl < 0.0,
            "{}: Rates P&L should be negative when rates increase, got {}",
            tc.name,
            actual_rates_pnl
        );
    } else {
        assert!(
            actual_rates_pnl > 0.0,
            "{}: Rates P&L should be positive when rates decrease, got {}",
            tc.name,
            actual_rates_pnl
        );
    }

    // Signed relative error against the second-order analytical reference.
    // Comparing SIGNED values (not magnitudes) so a sign flip fails outright.
    let actual_abs = actual_rates_pnl.abs();
    let expected_abs = expected_rates_pnl.abs();
    let rel_diff = ((actual_rates_pnl - expected_rates_pnl) / expected_rates_pnl).abs() * 100.0;

    // Log for debugging
    eprintln!(
        "{}: rate_change={}bp, expected_pnl={:.2}, actual_pnl={:.2}, rel_diff={:.4}%, convexity_term={:.2} ({:.4}% of expected)",
        tc.name,
        rate_change_bp,
        expected_rates_pnl,
        actual_rates_pnl,
        rel_diff,
        convexity_term,
        (convexity_term / expected_rates_pnl).abs() * 100.0
    );

    // Any "skip for small values" gate must key on the
    // INDEPENDENT analytical estimate, never on the value under test — the
    // old `|| actual_abs < 200.0` escape let a collapsed-magnitude rates P&L
    // (e.g. a 100× unit error shrinking $4,600 to $150) pass all three
    // magnitude pins as long as the sign survived.
    if expected_abs >= 200.0 {
        assert!(
            actual_abs > 0.5 * expected_abs,
            "{}: Rates P&L magnitude collapsed: actual {:.2} < 50% of analytical estimate {:.2}",
            tc.name,
            actual_rates_pnl,
            expected_rates_pnl
        );
        assert!(
            rel_diff < tc.tolerance_pct,
            "{}: Rates P&L ({:.2}) differs from analytical estimate ({:.2}) by {:.2}% (tolerance: {}%)",
            tc.name,
            actual_rates_pnl,
            expected_rates_pnl,
            rel_diff,
            tc.tolerance_pct
        );
    }
}

#[test]
fn test_analytical_parity_small_rate_increase() {
    let tc = AnalyticalParityTestCase::new_small_rate_increase();
    run_analytical_parity_test(&tc);
}

#[test]
fn test_analytical_parity_large_rate_increase() {
    let tc = AnalyticalParityTestCase::new_large_rate_increase();
    run_analytical_parity_test(&tc);
}

#[test]
fn test_analytical_parity_rate_decrease() {
    let tc = AnalyticalParityTestCase::new_rate_decrease();
    run_analytical_parity_test(&tc);
}

/// Test that attribution method is correctly identified.
#[test]
fn test_attribution_method_metadata() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "METADATA-TEST",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();

    let curve = build_flat_curve("USD-OIS", as_of_t0, 0.04);
    let market = MarketContext::new().insert(curve);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    let attribution = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &bond_instrument,
                &market,
                &market,
                as_of_t0,
                as_of_t1,
                &config,
            )
        },
    )
    .unwrap();

    // Verify metadata
    assert!(matches!(
        attribution.meta.method,
        AttributionMethod::Parallel
    ));
    assert_eq!(attribution.meta.instrument_id, "METADATA-TEST");
    assert_eq!(attribution.meta.t0, as_of_t0);
    assert_eq!(attribution.meta.t1, as_of_t1);
}

/// Test convexity benefit: for equal magnitude rate moves,
/// the gain from rate decrease should exceed the loss from rate increase.
///
/// This is a fundamental property of positive convexity instruments (bonds).
#[test]
fn test_convexity_benefit_symmetric_moves() {
    let as_of_t0 = create_date(2025, Month::January, 15).unwrap();
    let as_of_t1 = create_date(2025, Month::January, 16).unwrap();

    let issue = create_date(2025, Month::January, 1).unwrap();
    let maturity = create_date(2030, Month::January, 1).unwrap();

    let bond = Bond::fixed(
        "CONVEXITY-BENEFIT-TEST",
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
        issue,
        maturity,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .unwrap();

    let rate_base = 0.04;
    let rate_shift = 0.01; // 100bp

    let curve_base = build_flat_curve("USD-OIS", as_of_t0, rate_base);
    let curve_up = build_flat_curve("USD-OIS", as_of_t1, rate_base + rate_shift);
    let curve_down = build_flat_curve("USD-OIS", as_of_t1, rate_base - rate_shift);

    let market_base = MarketContext::new().insert(curve_base);
    let market_up = MarketContext::new().insert(curve_up);
    let market_down = MarketContext::new().insert(curve_down);

    let config = FinstackConfig::default();
    let bond_instrument: Arc<dyn Instrument> = Arc::new(bond);

    // Attribution for rate increase
    let attr_up = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &bond_instrument,
                &market_base,
                &market_up,
                as_of_t0,
                as_of_t1,
                &config,
            )
        },
    )
    .unwrap();

    // Attribution for rate decrease
    let attr_down = attribute_pnl(
        &AttributionMethod::Parallel,
        &AttributionRequest {
            execution_policy: ExecutionPolicy::Parallel,
            ..AttributionRequest::new(
                &bond_instrument,
                &market_base,
                &market_down,
                as_of_t0,
                as_of_t1,
                &config,
            )
        },
    )
    .unwrap();

    let loss_from_rate_increase = -attr_up.rates_curves_pnl.amount(); // Make positive
    let gain_from_rate_decrease = attr_down.rates_curves_pnl.amount();

    // Convexity benefit: gain > loss for equal magnitude moves
    assert!(
        gain_from_rate_decrease > loss_from_rate_increase,
        "Convexity benefit: gain from rate decrease ({:.2}) should exceed loss from rate increase ({:.2})",
        gain_from_rate_decrease,
        loss_from_rate_increase
    );

    // The convexity benefit should be roughly 2 × ½ × Convexity × P × (Δr)²
    // For a 5Y bond, this is typically a few hundred dollars on $1M notional with 100bp move
    let convexity_benefit = gain_from_rate_decrease - loss_from_rate_increase;
    assert!(
        convexity_benefit > 0.0,
        "Convexity benefit should be positive, got {}",
        convexity_benefit
    );

    eprintln!(
        "Convexity benefit: gain={:.2}, loss={:.2}, benefit={:.2}",
        gain_from_rate_decrease, loss_from_rate_increase, convexity_benefit
    );
}
