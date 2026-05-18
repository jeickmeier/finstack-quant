//! Python bindings for exotic rate products.
//!
//! Exposes deterministic coupon-computation helpers for the path-dependent
//! and callable exotic rate instruments defined in
//! [`finstack_valuations::instruments::rates`]:
//!
//! * [`tarn_coupon_profile`] — TARN coupon accrual with target-redemption
//!   knockout, using the shared
//!   [`CumulativeCouponTracker`](finstack_valuations::instruments::rates::exotics_shared::cumulative_coupon::CumulativeCouponTracker).
//! * [`snowball_coupon_profile`] — snowball / inverse-floater coupon
//!   schedule, mirroring the formula in
//!   [`Snowball::compute_coupon`](finstack_valuations::instruments::rates::snowball::Snowball::compute_coupon).
//! * [`cms_spread_option_intrinsic`] — intrinsic value of a CMS spread
//!   option (no convexity / correlation / vol adjustments).
//! * [`callable_range_accrual_accrued`] — accrued coupon on a range
//!   accrual, given an observed rate path.
//!
//! These are convenience helpers for building test fixtures, inspecting
//! coupon trajectories, and validating analytics — they are **not** a
//! substitute for the full Monte-Carlo / copula / LSMC pricers, which
//! require market data and are exposed via the standard
//! ``price_instrument`` / ``price_instrument_with_metrics`` pipeline.

use crate::errors::display_to_py;
use finstack_valuations::instruments::rates::exotics_shared::coupon_profiles;
use pyo3::prelude::*;
use pyo3::types::PyDict;

// ---------------------------------------------------------------------------
// TARN
// ---------------------------------------------------------------------------

/// Simulate a TARN coupon profile along a deterministic floating-rate path.
///
/// For each period, the coupon is ``max(fixed_rate - L_i, coupon_floor)``
/// scaled by ``day_count_fraction``.  Payments accumulate in a
/// [`CumulativeCouponTracker`] configured with ``target_coupon``; once the
/// cumulative hits the target, the final coupon is capped so the cumulative
/// equals the target exactly and the instrument is considered redeemed.
///
/// Parameters
/// ----------
/// fixed_rate : float
///     Fixed strike rate (decimal, e.g. ``0.08``).
/// coupon_floor : float
///     Per-period floor on ``fixed_rate - L_i`` (typically ``0.0``).
/// floating_fixings : list[float]
///     Floating rate fixings per period (decimal).  Length = number of
///     periods.
/// target_coupon : float
///     Target cumulative coupon level that triggers knockout.  Must be
///     strictly positive.
/// day_count_fraction : float
///     Year-fraction applied to each period coupon (e.g. ``0.5`` for
///     semi-annual on Act/360 ~= 0.5).
///
/// Returns
/// -------
/// dict
///     ``{"coupons_paid": list[float], "cumulative": list[float],
///     "redemption_index": int | None, "redeemed_early": bool}``.
///
///     * ``coupons_paid[i]`` — actual coupon at period ``i`` (post floor,
///       post target-cap).  Zero for periods after knockout.
///     * ``cumulative[i]`` — running cumulative coupon through period ``i``.
///     * ``redemption_index`` — zero-based index of the knockout period, or
///       ``None`` if never reached.
///     * ``redeemed_early`` — ``True`` iff the target was hit before the
///       final scheduled coupon.
#[pyfunction]
#[pyo3(signature = (fixed_rate, coupon_floor, floating_fixings, target_coupon, day_count_fraction))]
fn tarn_coupon_profile<'py>(
    py: Python<'py>,
    fixed_rate: f64,
    coupon_floor: f64,
    floating_fixings: Vec<f64>,
    target_coupon: f64,
    day_count_fraction: f64,
) -> PyResult<Bound<'py, PyDict>> {
    let profile = coupon_profiles::tarn_coupon_profile(
        fixed_rate,
        coupon_floor,
        &floating_fixings,
        target_coupon,
        day_count_fraction,
    )
    .map_err(display_to_py)?;

    let out = PyDict::new(py);
    out.set_item("coupons_paid", profile.coupons_paid)?;
    out.set_item("cumulative", profile.cumulative)?;
    match profile.redemption_index {
        Some(idx) => {
            out.set_item("redemption_index", idx)?;
            out.set_item("redeemed_early", profile.redeemed_early)?;
        }
        None => {
            out.set_item("redemption_index", py.None())?;
            out.set_item("redeemed_early", profile.redeemed_early)?;
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Snowball / Inverse Floater
// ---------------------------------------------------------------------------

/// Compute the coupon schedule for a snowball note or inverse floater.
///
/// For ``is_inverse_floater = False`` (snowball):
///
/// ```text
/// c_i = clip(c_{i-1} + fixed_rate - L_i, floor, cap)
/// ```
///
/// with ``c_0 = initial_coupon``.
///
/// For ``is_inverse_floater = True``:
///
/// ```text
/// c_i = clip(fixed_rate - leverage * L_i, floor, cap)
/// ```
///
/// (the path is not used — ``initial_coupon`` is ignored).
///
/// Parameters
/// ----------
/// initial_coupon : float
///     Initial coupon ``c_0`` for the snowball variant (ignored for
///     inverse floater).  Must be non-negative.
/// fixed_rate : float
///     Fixed rate component.
/// floating_fixings : list[float]
///     Floating rate fixings (one per period).
/// floor : float
///     Per-period floor (non-negative).
/// cap : float
///     Per-period cap; must be strictly greater than ``floor``.  Pass
///     ``float('inf')`` for an uncapped coupon.
/// is_inverse_floater : bool
///     If ``True``, use the inverse-floater formula; else snowball.
/// leverage : float
///     Leverage on the floating rate (used for the inverse floater;
///     typically ``1.0`` for snowball).  Must be strictly positive.
///
/// Returns
/// -------
/// list[float]
///     Coupon for each period in order.
#[pyfunction]
#[pyo3(signature = (initial_coupon, fixed_rate, floating_fixings, floor, cap, is_inverse_floater, leverage=1.0))]
fn snowball_coupon_profile(
    initial_coupon: f64,
    fixed_rate: f64,
    floating_fixings: Vec<f64>,
    floor: f64,
    cap: f64,
    is_inverse_floater: bool,
    leverage: f64,
) -> PyResult<Vec<f64>> {
    coupon_profiles::snowball_coupon_profile(
        initial_coupon,
        fixed_rate,
        &floating_fixings,
        floor,
        cap,
        is_inverse_floater,
        leverage,
    )
    .map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// CMS Spread Option (intrinsic)
// ---------------------------------------------------------------------------

/// Intrinsic (undiscounted, unhedged) payoff of a CMS spread option.
///
/// ```text
/// call:  notional * max(long_cms - short_cms - strike, 0)
/// put:   notional * max(strike - (long_cms - short_cms), 0)
/// ```
///
/// This is the deterministic payoff given already-known CMS fixings; the
/// full instrument pricer applies SABR marginals, a Gaussian copula on the
/// two CMS rates, and CMS convexity adjustments on top.
///
/// Parameters
/// ----------
/// long_cms : float
///     Long-tenor CMS rate (e.g. 10Y).
/// short_cms : float
///     Short-tenor CMS rate (e.g. 2Y).
/// strike : float
///     Strike on the spread ``long_cms - short_cms``.
/// is_call : bool
///     ``True`` for a call on the spread, ``False`` for a put.
/// notional : float
///     Notional multiplier (must be non-negative and finite).
#[pyfunction]
#[pyo3(signature = (long_cms, short_cms, strike, is_call, notional))]
fn cms_spread_option_intrinsic(
    long_cms: f64,
    short_cms: f64,
    strike: f64,
    is_call: bool,
    notional: f64,
) -> PyResult<f64> {
    coupon_profiles::cms_spread_option_intrinsic(long_cms, short_cms, strike, is_call, notional)
        .map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Callable Range Accrual (accrued coupon)
// ---------------------------------------------------------------------------

/// Accrued coupon on a range-accrual leg over a set of observations.
///
/// Counts the fraction of observations with a rate in the *inclusive*
/// interval ``[lower, upper]`` and scales the coupon by that fraction and
/// the period day-count fraction:
///
/// ```text
/// accrued = coupon_rate * day_count_fraction * (#in-range / #observations)
/// ```
///
/// The call provision is *not* applied here — this is the coupon that
/// would accrue assuming the note is not called before the period end.
///
/// Parameters
/// ----------
/// lower : float
///     Lower bound of the accrual range (inclusive).
/// upper : float
///     Upper bound of the accrual range (inclusive); must be ``> lower``.
/// observations : list[float]
///     Observed rates within the accrual period.  Must be non-empty.
/// coupon_rate : float
///     Annualised coupon rate when fully in-range (non-negative).
/// day_count_fraction : float
///     Year fraction for the accrual period (e.g. ~0.25 for quarterly
///     Act/360).  Must be non-negative and finite.
///
/// Returns
/// -------
/// float
///     Accrued coupon for the period.
#[pyfunction]
#[pyo3(signature = (lower, upper, observations, coupon_rate, day_count_fraction))]
fn callable_range_accrual_accrued(
    lower: f64,
    upper: f64,
    observations: Vec<f64>,
    coupon_rate: f64,
    day_count_fraction: f64,
) -> PyResult<f64> {
    coupon_profiles::callable_range_accrual_accrued(
        lower,
        upper,
        &observations,
        coupon_rate,
        day_count_fraction,
    )
    .map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// Register exotic rate helpers on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(tarn_coupon_profile, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(snowball_coupon_profile, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(cms_spread_option_intrinsic, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(callable_range_accrual_accrued, m)?)?;
    Ok(())
}
