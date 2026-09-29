//! Deterministic coupon / payoff helpers for exotic rate products.
//!
//! Mirrors `finstack-quant-py`'s `valuations/exotic_rates.rs`: lightweight, market-
//! data-free helpers useful for building test fixtures and inspecting coupon
//! trajectories. Full MC / copula / LSMC pricers stay on the standard
//! `priceInstrument` pipeline.

use crate::utils::input::js_bool;
use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::rates::hw1f::coupon_profiles;
use wasm_bindgen::prelude::*;

/// Simulated TARN coupon profile along a deterministic floating-rate path.
///
/// Returns a JSON object:
/// ```text
/// {
///   "coupons_paid": number[],
///   "cumulative":   number[],
///   "redemption_index": number | null,
///   "redeemed_early":   boolean
/// }
/// ```
///
/// Each period's coupon is `max(fixed_rate - L_i, coupon_floor) * day_count_fraction`.
/// Payments accumulate in a
/// [`CumulativeCouponTracker`](finstack_quant_valuations::instruments::rates::hw1f::cumulative_coupon::CumulativeCouponTracker) configured with
/// `target_coupon`; once cumulative hits the target, the final coupon is
/// capped and the instrument is considered redeemed.
///
/// # Errors
///
/// Throws a JavaScript exception if `fixed_rate`, `coupon_floor`,
/// `target_coupon`, `day_count_fraction`, or any fixing is non-finite;
/// `coupon_floor` is negative; `target_coupon` or `day_count_fraction` is
/// non-positive; or the result cannot be converted to a JavaScript object.
/// @param fixed_rate - Fixed coupon rate in decimal form before subtracting each floating fixing.
/// @param coupon_floor - Minimum period coupon rate in decimal form after the TARN rate calculation.
/// @param floating_fixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
/// @param target_coupon - Cumulative coupon target, as a fraction of notional, that redeems the TARN.
/// @param day_count_fraction - Accrual year fraction applied to each coupon period.
#[wasm_bindgen(js_name = tarnCouponProfile)]
pub fn tarn_coupon_profile(
    fixed_rate: f64,
    coupon_floor: f64,
    floating_fixings: Vec<f64>,
    target_coupon: f64,
    day_count_fraction: f64,
) -> Result<JsValue, JsValue> {
    let profile = coupon_profiles::tarn_coupon_profile(
        fixed_rate,
        coupon_floor,
        &floating_fixings,
        target_coupon,
        day_count_fraction,
    )
    .map_err(to_js_err)?;
    to_js_value(&profile)
}

/// Snowball coupon schedule.
///
///   `c_i = clip(c_{i-1} + fixed_rate - L_i, coupon_floor, coupon_cap)` with `c_0 = initial_coupon`.
///
/// # Errors
///
/// Throws a JavaScript exception if `initial_coupon` or `coupon_floor` is
/// negative; `initial_coupon`, `fixed_rate`, `coupon_floor`, or any fixing is
/// non-finite; or `coupon_cap` is set and is non-finite or not greater than
/// `coupon_floor`.
/// @param initial_coupon - Starting coupon rate before the first snowball update, in decimal form.
/// @param fixed_rate - Fixed coupon rate in decimal form added at each snowball step.
/// @param floating_fixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
/// @param coupon_floor - Minimum permitted coupon rate in decimal form.
/// @param coupon_cap - Optional maximum permitted coupon rate in decimal form; `null`/`undefined` leaves the coupon uncapped.
#[wasm_bindgen(js_name = snowballCouponProfile)]
pub fn snowball_coupon_profile(
    initial_coupon: f64,
    fixed_rate: f64,
    floating_fixings: Vec<f64>,
    coupon_floor: f64,
    coupon_cap: Option<f64>,
) -> Result<Box<[f64]>, JsValue> {
    coupon_profiles::snowball_coupon_profile(
        initial_coupon,
        fixed_rate,
        &floating_fixings,
        coupon_floor,
        coupon_cap,
    )
    .map(Vec::into_boxed_slice)
    .map_err(to_js_err)
}

/// Path-independent inverse-floater coupon schedule.
///
/// # Errors
///
/// Throws a JavaScript exception if `coupon_floor` is negative; `fixed_rate`,
/// `coupon_floor`, `gearing`, or any fixing is non-finite; `gearing` is
/// non-positive; or `coupon_cap` is set and is non-finite or not greater than
/// `coupon_floor`.
/// @param fixed_rate - Fixed coupon rate in decimal form before the geared floating deduction.
/// @param floating_fixings - Ordered floating-rate fixings in decimal form, one for each coupon period.
/// @param coupon_floor - Minimum permitted coupon rate in decimal form.
/// @param coupon_cap - Optional maximum permitted coupon rate in decimal form; `null`/`undefined` leaves the coupon uncapped.
/// @param gearing - Positive multiplier applied to each floating fixing in the inverse-floater coupon.
#[wasm_bindgen(js_name = inverseFloaterCouponProfile)]
pub fn inverse_floater_coupon_profile(
    fixed_rate: f64,
    floating_fixings: Vec<f64>,
    coupon_floor: f64,
    coupon_cap: Option<f64>,
    gearing: f64,
) -> Result<Box<[f64]>, JsValue> {
    coupon_profiles::inverse_floater_coupon_profile(
        fixed_rate,
        &floating_fixings,
        coupon_floor,
        coupon_cap,
        gearing,
    )
    .map(Vec::into_boxed_slice)
    .map_err(to_js_err)
}

/// Intrinsic (undiscounted, unhedged) payoff of a CMS spread option.
///
/// `call:  notional * max(long_cms - short_cms - strike, 0)`
/// `put:   notional * max(strike - (long_cms - short_cms), 0)`
///
/// # Errors
///
/// Throws a JavaScript exception if a CMS rate or `strike` is non-finite, or
/// if `notional` is negative or non-finite.
/// @param long_cms - Long-tenor CMS rate in decimal form.
/// @param short_cms - Short-tenor CMS rate in decimal form.
/// @param strike - CMS rate-spread strike in decimal form.
/// @param is_call - Whether to value a call (`true`) or put (`false`).
/// @param notional - Signed trade notional in the instrument's native currency units.
#[wasm_bindgen(js_name = cmsSpreadOptionIntrinsic)]
pub fn cms_spread_option_intrinsic(
    long_cms: f64,
    short_cms: f64,
    strike: f64,
    is_call: JsValue,
    notional: f64,
) -> Result<f64, JsValue> {
    let is_call = js_bool(&is_call, "isCall")?;
    coupon_profiles::cms_spread_option_intrinsic(long_cms, short_cms, strike, is_call, notional)
        .map_err(to_js_err)
}

/// Accrued coupon on a range-accrual leg over a set of observations.
///
/// Counts the fraction of observations with a rate in the inclusive interval
/// `[lower, upper]` and scales by the period day-count fraction:
///
/// `accrued = coupon_rate * day_count_fraction * (#in-range / #observations)`.
///
/// The call provision is not applied here.
///
/// # Errors
///
/// Throws a JavaScript exception if the range bounds are non-finite or not
/// strictly ordered; `observations` is empty or contains a non-finite value;
/// or `coupon_rate` or `day_count_fraction` is negative or non-finite.
/// @param lower - Inclusive lower bound of the observed-rate range, in decimal form.
/// @param upper - Inclusive upper bound of the observed-rate range, in decimal form.
/// @param observations - Observed floating rates in decimal form for the accrual period.
/// @param coupon_rate - Contractual coupon rate in decimal form before range weighting.
/// @param day_count_fraction - Accrual year fraction for the coupon period.
#[wasm_bindgen(js_name = callableRangeAccrualAccrued)]
pub fn callable_range_accrual_accrued(
    lower: f64,
    upper: f64,
    observations: Vec<f64>,
    coupon_rate: f64,
    day_count_fraction: f64,
) -> Result<f64, JsValue> {
    coupon_profiles::callable_range_accrual_accrued(
        lower,
        upper,
        &observations,
        coupon_rate,
        day_count_fraction,
    )
    .map_err(to_js_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snowball_honors_cap_and_floor() {
        let coupons = snowball_coupon_profile(0.02, 0.05, vec![0.01, 0.04, 0.03], 0.0, Some(0.10))
            .expect("snowball");
        assert_eq!(coupons.len(), 3);
        for c in coupons {
            assert!((0.0..=0.10).contains(&c));
        }
    }

    #[test]
    fn inverse_floater_uses_explicit_gearing() {
        let coupons = inverse_floater_coupon_profile(0.05, vec![0.01, 0.02], 0.0, Some(0.10), 2.0)
            .expect("inverse floater");
        assert!((coupons[0] - 0.03).abs() < 1e-12);
        assert!((coupons[1] - 0.01).abs() < 1e-12);
    }
}
