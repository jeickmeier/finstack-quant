//! Deterministic coupon / payoff helpers for exotic rate products.
//!
//! These are lightweight, market-data-free helpers useful for building test
//! fixtures, inspecting coupon trajectories, and validating analytics. They
//! are **not** a substitute for the full Monte-Carlo / copula / LSMC pricers,
//! which require market data and run on the standard pricing pipeline.
//!
//! The functions here host the canonical implementation that the Python and
//! WASM binding crates wrap as thin adapters: the bindings perform only type
//! conversion and error mapping, while all coupon math and input validation
//! live in this module.

use crate::instruments::rates::exotics_shared::cumulative_coupon::CumulativeCouponTracker;

/// Result of a TARN coupon-profile simulation along a deterministic path.
#[derive(Debug, Clone, PartialEq)]
pub struct TarnCouponProfile {
    /// Actual coupon paid at each period (post floor, post target-cap). Zero
    /// for periods after knockout.
    pub coupons_paid: Vec<f64>,
    /// Running cumulative coupon through each period.
    pub cumulative: Vec<f64>,
    /// Zero-based index of the knockout period, or `None` if never reached.
    pub redemption_index: Option<usize>,
    /// `true` iff the target was hit before the final scheduled coupon.
    pub redeemed_early: bool,
}

/// Simulate a TARN coupon profile along a deterministic floating-rate path.
///
/// For each period, the coupon is `max(fixed_rate - L_i, coupon_floor)` scaled
/// by `day_count_fraction`. Payments accumulate in a
/// [`CumulativeCouponTracker`] configured with `target_coupon`; once the
/// cumulative hits the target, the final coupon is capped so the cumulative
/// equals the target exactly and the instrument is considered redeemed.
///
/// # Arguments
///
/// * `fixed_rate` - Fixed strike rate (decimal).
/// * `coupon_floor` - Per-period floor on `fixed_rate - L_i` (non-negative).
/// * `floating_fixings` - Floating rate fixings per period (decimal).
/// * `target_coupon` - Target cumulative coupon level triggering knockout
///   (strictly positive).
/// * `day_count_fraction` - Year-fraction applied to each period coupon
///   (strictly positive).
///
/// # Errors
///
/// Returns an error message string if any input is non-finite, the target
/// coupon or day-count fraction is non-positive, or the coupon floor is
/// negative.
pub fn tarn_coupon_profile(
    fixed_rate: f64,
    coupon_floor: f64,
    floating_fixings: &[f64],
    target_coupon: f64,
    day_count_fraction: f64,
) -> Result<TarnCouponProfile, String> {
    if !target_coupon.is_finite() || target_coupon <= 0.0 {
        return Err(format!(
            "target_coupon ({target_coupon}) must be positive and finite"
        ));
    }
    if !day_count_fraction.is_finite() || day_count_fraction <= 0.0 {
        return Err(format!(
            "day_count_fraction ({day_count_fraction}) must be positive and finite"
        ));
    }
    if !fixed_rate.is_finite() {
        return Err("fixed_rate must be finite".to_owned());
    }
    if !coupon_floor.is_finite() || coupon_floor < 0.0 {
        return Err(format!(
            "coupon_floor ({coupon_floor}) must be non-negative and finite"
        ));
    }

    let n = floating_fixings.len();
    let mut tracker = CumulativeCouponTracker::with_target(target_coupon);
    let mut coupons_paid: Vec<f64> = Vec::with_capacity(n);
    let mut cumulative: Vec<f64> = Vec::with_capacity(n);

    for &l_i in floating_fixings {
        if !l_i.is_finite() {
            return Err("floating_fixings must all be finite".to_owned());
        }
        let raw = (fixed_rate - l_i).max(coupon_floor);
        let period_coupon = raw * day_count_fraction;
        let actual = tracker.add_coupon(period_coupon);
        coupons_paid.push(actual);
        cumulative.push(tracker.cumulative());
    }

    let (redemption_index, redeemed_early) = match tracker.knockout_period() {
        Some(idx) => (Some(idx), idx + 1 < n),
        None => (None, false),
    };

    Ok(TarnCouponProfile {
        coupons_paid,
        cumulative,
        redemption_index,
        redeemed_early,
    })
}

/// Compute the coupon schedule for a snowball note or inverse floater.
///
/// For `is_inverse_floater = false` (snowball):
/// `c_i = clip(c_{i-1} + fixed_rate - L_i, floor, cap)` with
/// `c_0 = initial_coupon`.
///
/// For `is_inverse_floater = true`:
/// `c_i = clip(fixed_rate - leverage * L_i, floor, cap)` (path-independent;
/// `initial_coupon` is ignored).
///
/// # Arguments
///
/// * `initial_coupon` - Initial coupon `c_0` for the snowball variant (ignored
///   for the inverse floater; must be non-negative for snowball).
/// * `fixed_rate` - Fixed rate component.
/// * `floating_fixings` - Floating rate fixings (one per period).
/// * `floor` - Per-period floor (non-negative).
/// * `cap` - Per-period cap; must be strictly greater than `floor`. Pass
///   `f64::INFINITY` for an uncapped coupon.
/// * `is_inverse_floater` - If `true`, use the inverse-floater formula.
/// * `leverage` - Leverage on the floating rate (strictly positive).
///
/// # Errors
///
/// Returns an error message string if any input is non-finite, the floor is
/// negative, the cap is not strictly above the floor, the leverage is
/// non-positive, or the snowball initial coupon is negative.
pub fn snowball_coupon_profile(
    initial_coupon: f64,
    fixed_rate: f64,
    floating_fixings: &[f64],
    floor: f64,
    cap: f64,
    is_inverse_floater: bool,
    leverage: f64,
) -> Result<Vec<f64>, String> {
    if !fixed_rate.is_finite() {
        return Err("fixed_rate must be finite".to_owned());
    }
    if !floor.is_finite() || floor < 0.0 {
        return Err(format!("floor ({floor}) must be non-negative and finite"));
    }
    if cap.is_nan() || cap <= floor {
        return Err(format!(
            "cap ({cap}) must be strictly greater than floor ({floor})"
        ));
    }
    if !leverage.is_finite() || leverage <= 0.0 {
        return Err(format!("leverage ({leverage}) must be positive and finite"));
    }
    if !is_inverse_floater && initial_coupon < 0.0 {
        return Err(format!(
            "initial_coupon ({initial_coupon}) must be non-negative for snowball variant"
        ));
    }

    let mut prev = initial_coupon;
    let mut out: Vec<f64> = Vec::with_capacity(floating_fixings.len());
    for &l_i in floating_fixings {
        if !l_i.is_finite() {
            return Err("floating_fixings must all be finite".to_owned());
        }
        let raw = if is_inverse_floater {
            fixed_rate - leverage * l_i
        } else {
            prev + fixed_rate - l_i
        };
        let floored = raw.max(floor);
        let c = if cap.is_finite() {
            floored.min(cap)
        } else {
            floored
        };
        out.push(c);
        prev = c;
    }
    Ok(out)
}

/// Intrinsic (undiscounted, unhedged) payoff of a CMS spread option.
///
/// `call:  notional * max(long_cms - short_cms - strike, 0)`
/// `put:   notional * max(strike - (long_cms - short_cms), 0)`
///
/// This is the deterministic payoff given already-known CMS fixings; the full
/// instrument pricer applies SABR marginals, a Gaussian copula on the two CMS
/// rates, and CMS convexity adjustments on top.
///
/// # Arguments
///
/// * `long_cms` - Long-tenor CMS rate.
/// * `short_cms` - Short-tenor CMS rate.
/// * `strike` - Strike on the spread `long_cms - short_cms`.
/// * `is_call` - `true` for a call on the spread, `false` for a put.
/// * `notional` - Notional multiplier (non-negative and finite).
///
/// # Errors
///
/// Returns an error message string if any rate input is non-finite or the
/// notional is negative / non-finite.
pub fn cms_spread_option_intrinsic(
    long_cms: f64,
    short_cms: f64,
    strike: f64,
    is_call: bool,
    notional: f64,
) -> Result<f64, String> {
    if !long_cms.is_finite() || !short_cms.is_finite() || !strike.is_finite() {
        return Err("long_cms, short_cms, and strike must all be finite".to_owned());
    }
    if !notional.is_finite() || notional < 0.0 {
        return Err(format!(
            "notional ({notional}) must be non-negative and finite"
        ));
    }
    let spread = long_cms - short_cms;
    let payoff = if is_call {
        (spread - strike).max(0.0)
    } else {
        (strike - spread).max(0.0)
    };
    Ok(payoff * notional)
}

/// Accrued coupon on a range-accrual leg over a set of observations.
///
/// Counts the fraction of observations with a rate in the inclusive interval
/// `[lower, upper]` and scales the coupon by that fraction and the period
/// day-count fraction:
///
/// `accrued = coupon_rate * day_count_fraction * (#in-range / #observations)`.
///
/// The call provision is not applied here — this is the coupon that would
/// accrue assuming the note is not called before the period end.
///
/// # Arguments
///
/// * `lower` - Lower bound of the accrual range (inclusive).
/// * `upper` - Upper bound of the accrual range (inclusive); must be `> lower`.
/// * `observations` - Observed rates within the accrual period (non-empty).
/// * `coupon_rate` - Annualised coupon rate when fully in-range (non-negative).
/// * `day_count_fraction` - Year fraction for the accrual period (non-negative).
///
/// # Errors
///
/// Returns an error message string if the bounds are non-finite or not
/// strictly ordered, the coupon rate or day-count fraction is negative /
/// non-finite, the observations slice is empty, or any observation is
/// non-finite.
pub fn callable_range_accrual_accrued(
    lower: f64,
    upper: f64,
    observations: &[f64],
    coupon_rate: f64,
    day_count_fraction: f64,
) -> Result<f64, String> {
    if !lower.is_finite() || !upper.is_finite() || lower >= upper {
        return Err(format!(
            "lower ({lower}) must be strictly less than upper ({upper}), and both finite"
        ));
    }
    if !coupon_rate.is_finite() || coupon_rate < 0.0 {
        return Err(format!(
            "coupon_rate ({coupon_rate}) must be non-negative and finite"
        ));
    }
    if !day_count_fraction.is_finite() || day_count_fraction < 0.0 {
        return Err(format!(
            "day_count_fraction ({day_count_fraction}) must be non-negative and finite"
        ));
    }
    if observations.is_empty() {
        return Err("observations must contain at least one value".to_owned());
    }

    let mut in_range = 0usize;
    for &o in observations {
        if !o.is_finite() {
            return Err("observations must all be finite".to_owned());
        }
        if o >= lower && o <= upper {
            in_range += 1;
        }
    }
    let fraction = in_range as f64 / observations.len() as f64;
    Ok(coupon_rate * day_count_fraction * fraction)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tarn_accumulates_and_knocks_out_on_target() {
        // fixed 8%, no floating (L=0), dcf=1 => 8% per period; target 0.20.
        let profile = tarn_coupon_profile(0.08, 0.0, &[0.0, 0.0, 0.0, 0.0], 0.20, 1.0)
            .expect("valid tarn inputs");
        assert_eq!(profile.coupons_paid.len(), 4);
        // Cumulative is monotone non-decreasing and capped at the target.
        for w in profile.cumulative.windows(2) {
            assert!(w[1] >= w[0]);
        }
        let last = profile.cumulative.last().copied().expect("non-empty");
        assert!((last - 0.20).abs() < 1e-12);
        // Knockout occurs on period index 2 (0.08*3 = 0.24 >= 0.20).
        assert_eq!(profile.redemption_index, Some(2));
        assert!(profile.redeemed_early);
    }

    #[test]
    fn tarn_rejects_non_positive_target() {
        let err = tarn_coupon_profile(0.05, 0.0, &[0.01], 0.0, 0.5)
            .expect_err("zero target must be rejected");
        assert!(err.contains("target_coupon"));
    }

    #[test]
    fn snowball_honors_cap_and_floor() {
        let coupons =
            snowball_coupon_profile(0.02, 0.05, &[0.01, 0.04, 0.03], 0.0, 0.10, false, 1.0)
                .expect("valid snowball inputs");
        assert_eq!(coupons.len(), 3);
        for c in coupons {
            assert!((0.0..=0.10).contains(&c));
        }
    }

    #[test]
    fn snowball_inverse_floater_is_path_independent() {
        let coupons =
            snowball_coupon_profile(0.0, 0.06, &[0.01, 0.02], 0.0, f64::INFINITY, true, 2.0)
                .expect("valid inverse-floater inputs");
        // c_i = 0.06 - 2 * L_i
        assert!((coupons[0] - (0.06 - 2.0 * 0.01)).abs() < 1e-12);
        assert!((coupons[1] - (0.06 - 2.0 * 0.02)).abs() < 1e-12);
    }

    #[test]
    fn snowball_rejects_cap_below_floor() {
        let err = snowball_coupon_profile(0.0, 0.05, &[0.01], 0.10, 0.05, false, 1.0)
            .expect_err("cap <= floor must be rejected");
        assert!(err.contains("cap"));
    }

    #[test]
    fn cms_spread_intrinsic_call_and_put() {
        let call =
            cms_spread_option_intrinsic(0.04, 0.02, 0.01, true, 1_000_000.0).expect("call payoff");
        assert!((call - 10_000.0).abs() < 1e-9);
        // Put on the same spread (0.02) with strike 0.01 is out-of-the-money.
        let put =
            cms_spread_option_intrinsic(0.04, 0.02, 0.01, false, 1_000_000.0).expect("put payoff");
        assert!((put - 0.0).abs() < 1e-9);
    }

    #[test]
    fn cms_spread_rejects_negative_notional() {
        let err = cms_spread_option_intrinsic(0.04, 0.02, 0.01, true, -1.0)
            .expect_err("negative notional must be rejected");
        assert!(err.contains("notional"));
    }

    #[test]
    fn range_accrual_counts_in_range_fraction() {
        // 2 of 4 observations in [0.02, 0.04]; coupon 5%, dcf 0.25.
        let accrued =
            callable_range_accrual_accrued(0.02, 0.04, &[0.01, 0.03, 0.035, 0.05], 0.05, 0.25)
                .expect("valid range-accrual inputs");
        let expected = 0.05 * 0.25 * (2.0 / 4.0);
        assert!((accrued - expected).abs() < 1e-12);
    }

    #[test]
    fn range_accrual_rejects_empty_observations() {
        let err = callable_range_accrual_accrued(0.02, 0.04, &[], 0.05, 0.25)
            .expect_err("empty observations must be rejected");
        assert!(err.contains("observations"));
    }
}
