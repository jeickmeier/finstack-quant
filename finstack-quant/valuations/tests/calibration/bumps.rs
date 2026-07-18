//! Invariant tests for the curve/surface bump paths.
//!
//! These tests pin the three properties that any bump implementation must
//! satisfy, and whose absence allowed a silent correctness defect in the
//! discount-curve synthetic bump path:
//!
//! 1. **Zero-shock identity** — a `0 bp` bump reproduces the original object
//!    exactly, at knot *and* non-knot tenors.
//! 2. **Faithfulness** — an `X bp` parallel bump moves the shocked quantity by
//!    exactly `X bp`.
//! 3. **Additivity** — `+X bp` followed by `−X bp` returns to the base.
//!
//! The historical defect: `bump_discount_curve_synthetic` implied synthetic
//! deposit quotes with a hardcoded `Act365F` day count and re-bootstrapped them
//! through an index registered as `Act360`, on a maturity grid
//! (`base + round(t · 365.25)` days) that did not map back to the original knot
//! times. The round trip was not the identity, so a `0 bp` shock moved
//! continuously-compounded zeros by roughly +4.4 bp to +5.5 bp.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, HazardCurve, InflationCurve,
};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_valuations::calibration::bumps::{
    bump_discount_curve_synthetic, bump_hazard_shift, bump_inflation_rates, bump_vol_surface,
    BumpRequest, VolBumpRequest,
};
use time::Month;

/// Tolerance for quantities that must agree to floating-point round-off.
const EXACT_TOL: f64 = 1e-12;

/// Knot and non-knot probe tenors. 0.75/1.5/4.0/7.5 fall strictly between
/// knots, so they exercise the interpolation path rather than stored knots.
const PROBE_TENORS: [f64; 11] = [0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 7.5, 10.0];

const KNOT_TENORS: [f64; 6] = [0.5, 1.0, 2.0, 3.0, 5.0, 10.0];

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).expect("valid base date")
}

/// The exact curve from the defect reproduction.
///
/// Built with log-linear interpolation so that a parallel shift of the
/// continuously-compounded zero curve is exactly representable between knots:
/// log-DF is affine in `t`, and the shift `−δt` is affine, so the shift
/// survives interpolation unchanged.
fn sample_discount_curve() -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(base_date())
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .knots([
            (0.0, 1.0),
            (0.5, 0.980),
            (1.0, 0.960),
            (2.0, 0.920),
            (3.0, 0.880),
            (5.0, 0.800),
            (10.0, 0.650),
        ])
        .build()
        .expect("sample discount curve")
}

fn empty_market() -> MarketContext {
    MarketContext::new()
}

fn bump_discount(curve: &DiscountCurve, bump: &BumpRequest) -> DiscountCurve {
    bump_discount_curve_synthetic(curve, &empty_market(), bump, base_date(), Currency::USD)
        .expect("discount bump succeeds")
}

// ---------------------------------------------------------------------------
// Discount curve — the path that carried the defect
// ---------------------------------------------------------------------------

/// A `0 bp` bump must be a no-op at every tenor, knot and non-knot.
///
/// This is the test that would have caught the original defect: before the
/// fix, this shifted zeros by +4.42 bp (0.5y) through +5.55 bp (1y).
#[test]
fn discount_zero_shock_is_identity_at_knot_and_non_knot_tenors() {
    let base = sample_discount_curve();
    let bumped = bump_discount(&base, &BumpRequest::Parallel(0.0));

    for &t in &PROBE_TENORS {
        let shift_bp = (bumped.zero(t) - base.zero(t)) * 1e4;
        assert!(
            shift_bp.abs() < EXACT_TOL,
            "0bp bump must not move the zero curve at t={t}, got {shift_bp} bp",
        );
        assert!(
            (bumped.df(t) - base.df(t)).abs() < EXACT_TOL,
            "0bp bump must not move DF at t={t}",
        );
    }

    // The knot grid itself must be preserved: the defect also relocated knots
    // (t=0.5 became 0.50137, t=2.0 became 2.01096, ...).
    assert_eq!(
        base.knots(),
        bumped.knots(),
        "0bp bump must not relocate curve knots",
    );
}

/// An `X bp` parallel bump must move continuously-compounded zeros by exactly
/// `X bp` — that is what "parallel" means.
#[test]
fn discount_parallel_bump_is_faithful_at_knot_and_non_knot_tenors() {
    let base = sample_discount_curve();

    for requested_bp in [-25.0, -5.0, 1.0, 5.0, 50.0] {
        let bumped = bump_discount(&base, &BumpRequest::Parallel(requested_bp));
        for &t in &PROBE_TENORS {
            let realized_bp = (bumped.zero(t) - base.zero(t)) * 1e4;
            assert!(
                (realized_bp - requested_bp).abs() < 1e-9,
                "requested {requested_bp} bp at t={t}, realized {realized_bp} bp",
            );
        }
    }
}

/// `+X bp` then `−X bp` must return to the base curve.
#[test]
fn discount_bump_is_additive() {
    let base = sample_discount_curve();
    let up = bump_discount(&base, &BumpRequest::Parallel(5.0));
    let round_trip = bump_discount(&up, &BumpRequest::Parallel(-5.0));

    for &t in &PROBE_TENORS {
        assert!(
            (round_trip.zero(t) - base.zero(t)).abs() < EXACT_TOL,
            "+5bp then -5bp must return to base at t={t}",
        );
    }
}

/// A tenor-targeted bump moves the targeted knot and leaves the others alone.
#[test]
fn discount_tenor_bump_targets_closest_knot_only() {
    let base = sample_discount_curve();
    let bumped = bump_discount(&base, &BumpRequest::Tenors(vec![(5.0, 10.0)]));

    for &t in &KNOT_TENORS {
        let realized_bp = (bumped.zero(t) - base.zero(t)) * 1e4;
        let expected_bp = if (t - 5.0).abs() < EXACT_TOL {
            10.0
        } else {
            0.0
        };
        assert!(
            (realized_bp - expected_bp).abs() < 1e-9,
            "tenor bump at 5y: expected {expected_bp} bp at t={t}, got {realized_bp} bp",
        );
    }
}

/// A zero-shock tenor bump is also an exact identity.
#[test]
fn discount_zero_shock_tenor_bump_is_identity() {
    let base = sample_discount_curve();
    let bumped = bump_discount(&base, &BumpRequest::Tenors(vec![(1.0, 0.0), (5.0, 0.0)]));

    for &t in &PROBE_TENORS {
        assert!(
            (bumped.zero(t) - base.zero(t)).abs() < EXACT_TOL,
            "0bp tenor bump must not move the zero curve at t={t}",
        );
    }
}

/// The bumped curve keeps the original identifier so it can replace the base
/// curve in a `MarketContext` (the scenarios engine relies on this).
#[test]
fn discount_bump_preserves_curve_id_and_metadata() {
    let base = sample_discount_curve();
    let bumped = bump_discount(&base, &BumpRequest::Parallel(5.0));

    assert_eq!(base.id(), bumped.id(), "bump must preserve the curve id");
    assert_eq!(base.day_count(), bumped.day_count());
    assert_eq!(base.interp_style(), bumped.interp_style());
    assert_eq!(base.base_date(), bumped.base_date());
}

/// Bumping a rolled curve must work for *every* roll length, including the
/// lengths that leave a near-zero residual knot at the front.
///
/// `roll_forward` shifts knot times to `t' = t − dt` and drops only the knots
/// with `t' ≤ 0`. A knot sitting exactly on the roll tenor therefore survives as
/// a tiny positive residual whenever the roll's day count does not divide the
/// tenor exactly — e.g. a 3M business-day roll of 89/90/91 days leaves the
/// `t = 0.25` knot at `t' = 0.006164 / 0.003425 / 0.000685`.
///
/// The old synthetic path fabricated a deposit maturity at
/// `base + round(t' · 365.25)` days, which for those residuals is 2, 1, or 0
/// days out. The synthetic money-market index carries a two-business-day
/// settlement lag, so the deposit's accrual start landed on or after its
/// maturity and schedule construction failed with
/// `Invalid date range: start must be before end`.
///
/// Shifting zero rates directly never constructs a date, so a residual knot is
/// simply scaled by `exp(−δ · t') ≈ 1`.
#[test]
fn discount_bump_survives_every_roll_length() {
    // Knots sitting exactly on the 1M, 3M, 6M and 1Y roll tenors, so that every
    // common roll length produces a coincidence somewhere on the grid.
    let curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date())
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .knots([
            (0.0, 1.0),
            (1.0 / 12.0, 0.9963),
            (0.25, 0.9888),
            (0.5, 0.9775),
            (1.0, 0.955),
            (2.0, 0.91),
            (3.0, 0.87),
            (5.0, 0.80),
            (10.0, 0.65),
        ])
        .build()
        .expect("rolled-curve fixture");

    // Sweep every roll length through a year: this covers all four tenor
    // coincidences and, crucially, the near-zero-residual window on each side.
    for days in 0..=370i64 {
        let rolled = curve
            .roll_forward(days)
            .unwrap_or_else(|e| panic!("roll_forward({days}) failed: {e}"));

        let bumped = bump_discount_curve_synthetic(
            &rolled,
            &empty_market(),
            &BumpRequest::Parallel(100.0),
            rolled.base_date(),
            Currency::USD,
        )
        .unwrap_or_else(|e| panic!("bump after {days}d roll failed: {e}"));

        // The bump must remain faithful on the rolled grid, including at the
        // residual front knot.
        for &t in rolled.knots() {
            if t <= 0.0 {
                continue;
            }
            let realized_bp = (bumped.zero(t) - rolled.zero(t)) * 1e4;
            assert!(
                (realized_bp - 100.0).abs() < 1e-6,
                "after {days}d roll: requested 100 bp at t={t}, realized {realized_bp} bp",
            );
        }
    }
}

/// A zero-shock bump on a rolled curve is still an exact identity, including
/// when the roll leaves a near-zero residual knot.
#[test]
fn discount_zero_shock_on_rolled_curve_is_identity() {
    let curve = sample_discount_curve();

    for days in [88i64, 89, 90, 91, 92, 181, 182, 365] {
        let rolled = curve
            .roll_forward(days)
            .unwrap_or_else(|e| panic!("roll_forward({days}) failed: {e}"));
        let bumped = bump_discount_curve_synthetic(
            &rolled,
            &empty_market(),
            &BumpRequest::Parallel(0.0),
            rolled.base_date(),
            Currency::USD,
        )
        .unwrap_or_else(|e| panic!("0bp bump after {days}d roll failed: {e}"));

        assert_eq!(rolled.knots(), bumped.knots());
        for &t in rolled.knots() {
            assert!(
                (bumped.df(t) - rolled.df(t)).abs() < EXACT_TOL,
                "0bp bump after {days}d roll moved DF at t={t}",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Sibling bump paths
// ---------------------------------------------------------------------------

fn sample_inflation_curve() -> InflationCurve {
    InflationCurve::builder("USD-CPI")
        .base_date(base_date())
        .base_cpi(300.0)
        .indexation_lag_months(3)
        .knots([
            (0.0, 300.0),
            (1.0, 307.5),
            (2.0, 315.2),
            (5.0, 339.6),
            (10.0, 384.2),
        ])
        .build()
        .expect("sample inflation curve")
}

fn inflation_market() -> MarketContext {
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date())
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (1.0, 0.960),
            (2.0, 0.920),
            (5.0, 0.800),
            (10.0, 0.650),
        ])
        .build()
        .expect("inflation discount curve");
    MarketContext::new().insert(discount)
}

/// A `0 bp` inflation bump must reproduce the original CPI curve.
#[test]
fn inflation_zero_shock_is_identity() {
    let base = sample_inflation_curve();
    let market = inflation_market();
    let discount_id = finstack_quant_core::types::CurveId::new("USD-OIS");

    let bumped = bump_inflation_rates(
        &base,
        &market,
        &BumpRequest::Parallel(0.0),
        &discount_id,
        base_date(),
        Currency::USD,
        "3M",
    )
    .expect("inflation bump succeeds");

    for &t in &[1.0, 2.0, 5.0, 10.0] {
        let rel = (bumped.cpi(t) - base.cpi(t)).abs() / base.cpi(t);
        assert!(
            rel < 1e-10,
            "0bp inflation bump must not move CPI at t={t}: base={}, bumped={}",
            base.cpi(t),
            bumped.cpi(t),
        );
    }
}

/// An `X bp` inflation bump must move the implied zero-coupon inflation rate by
/// exactly `X bp`.
#[test]
fn inflation_parallel_bump_is_faithful() {
    let base = sample_inflation_curve();
    let market = inflation_market();
    let discount_id = finstack_quant_core::types::CurveId::new("USD-OIS");
    let requested_bp = 25.0;

    let bumped = bump_inflation_rates(
        &base,
        &market,
        &BumpRequest::Parallel(requested_bp),
        &discount_id,
        base_date(),
        Currency::USD,
        "3M",
    )
    .expect("inflation bump succeeds");

    let implied = |curve: &InflationCurve, t: f64| (curve.cpi(t) / curve.base_cpi()).powf(1.0 / t);
    for &t in &[1.0, 2.0, 5.0, 10.0] {
        let realized_bp = (implied(&bumped, t) - implied(&base, t)) * 1e4;
        assert!(
            (realized_bp - requested_bp).abs() < 1e-6,
            "requested {requested_bp} bp at t={t}, realized {realized_bp} bp",
        );
    }
}

fn sample_hazard_curve() -> HazardCurve {
    HazardCurve::builder("ACME-SEN-USD")
        .base_date(base_date())
        .recovery_rate(0.4)
        .knots([(1.0, 0.010), (3.0, 0.015), (5.0, 0.020), (10.0, 0.025)])
        .build()
        .expect("sample hazard curve")
}

/// A `0 bp` model hazard shift must reproduce the original hazard curve.
#[test]
fn hazard_shift_zero_shock_is_identity() {
    let base = sample_hazard_curve();
    let bumped = bump_hazard_shift(&base, &BumpRequest::Parallel(0.0)).expect("hazard shift");

    for &t in &[0.5, 1.0, 2.0, 3.0, 5.0, 7.5, 10.0] {
        assert!(
            (bumped.sp(t) - base.sp(t)).abs() < EXACT_TOL,
            "0bp hazard shift must not move survival at t={t}",
        );
    }
}

/// An `X bp` model hazard shift must move hazard rates by exactly `X bp`.
#[test]
fn hazard_shift_is_faithful_and_additive() {
    let base = sample_hazard_curve();
    let requested_bp = 25.0;
    let up =
        bump_hazard_shift(&base, &BumpRequest::Parallel(requested_bp)).expect("hazard shift up");

    for &t in &[1.0, 3.0, 5.0, 10.0] {
        let realized_bp = (up.hazard_rate(t) - base.hazard_rate(t)) * 1e4;
        assert!(
            (realized_bp - requested_bp).abs() < 1e-9,
            "requested {requested_bp} bp at t={t}, realized {realized_bp} bp",
        );
    }

    let round_trip =
        bump_hazard_shift(&up, &BumpRequest::Parallel(-requested_bp)).expect("hazard shift down");
    for &t in &[1.0, 3.0, 5.0, 10.0] {
        assert!(
            (round_trip.hazard_rate(t) - base.hazard_rate(t)).abs() < EXACT_TOL,
            "hazard shift must be additive at t={t}",
        );
    }
}

fn sample_vol_surface() -> VolSurface {
    VolSurface::from_grid(
        "SPX-VOL",
        &[0.25, 1.0, 2.0],
        &[80.0, 100.0, 120.0],
        &[
            0.28, 0.22, 0.20, //
            0.26, 0.21, 0.20, //
            0.25, 0.21, 0.21, //
        ],
    )
    .expect("sample vol surface")
}

/// A zero vol bump must reproduce the original surface.
#[test]
fn vol_zero_shock_is_identity() {
    let base = sample_vol_surface();
    let bumped = bump_vol_surface(&base, &VolBumpRequest::Parallel(0.0)).expect("vol bump");

    for &expiry in &[0.25, 0.5, 1.0, 2.0] {
        for &strike in &[80.0, 90.0, 100.0, 120.0] {
            assert!(
                (bumped.value_clamped(expiry, strike) - base.value_clamped(expiry, strike)).abs()
                    < EXACT_TOL,
                "0 vol bump must not move vol at ({expiry}, {strike})",
            );
        }
    }
}

/// A parallel vol bump is exactly additive in vol points and reverses cleanly.
#[test]
fn vol_parallel_bump_is_faithful_and_additive() {
    let base = sample_vol_surface();
    let shift = 0.01;
    let up = bump_vol_surface(&base, &VolBumpRequest::Parallel(shift)).expect("vol bump up");

    for &expiry in &[0.25, 1.0, 2.0] {
        for &strike in &[80.0, 100.0, 120.0] {
            let realized = up.value_clamped(expiry, strike) - base.value_clamped(expiry, strike);
            assert!(
                (realized - shift).abs() < EXACT_TOL,
                "requested {shift} vol at ({expiry}, {strike}), realized {realized}",
            );
        }
    }

    let round_trip = bump_vol_surface(&up, &VolBumpRequest::Parallel(-shift)).expect("vol bump dn");
    for &expiry in &[0.25, 1.0, 2.0] {
        for &strike in &[80.0, 100.0, 120.0] {
            assert!(
                (round_trip.value_clamped(expiry, strike) - base.value_clamped(expiry, strike))
                    .abs()
                    < EXACT_TOL,
                "vol bump must be additive at ({expiry}, {strike})",
            );
        }
    }
}
