//! Deterministic coupon / payoff helpers for exotic rate products.
//!
//! Mirrors `finstack-quant-py`'s `valuations/exotic_rates.rs`: lightweight, market-
//! data-free helpers useful for building test fixtures and inspecting coupon
//! trajectories. Full MC / copula / LSMC pricers stay on the standard
//! `priceInstrument` pipeline.

use crate::utils::input::{js_bool, js_f64, js_f64_seq, js_opt_f64};
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
    fixed_rate: JsValue,
    coupon_floor: JsValue,
    floating_fixings: JsValue,
    target_coupon: JsValue,
    day_count_fraction: JsValue,
) -> Result<JsValue, JsValue> {
    let fixed_rate = js_f64(&fixed_rate, "fixedRate")?;
    let coupon_floor = js_f64(&coupon_floor, "couponFloor")?;
    let floating_fixings = js_f64_seq(&floating_fixings, "floatingFixings")?;
    let target_coupon = js_f64(&target_coupon, "targetCoupon")?;
    let day_count_fraction = js_f64(&day_count_fraction, "dayCountFraction")?;
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
    initial_coupon: JsValue,
    fixed_rate: JsValue,
    floating_fixings: JsValue,
    coupon_floor: JsValue,
    coupon_cap: Option<JsValue>,
) -> Result<Box<[f64]>, JsValue> {
    let initial_coupon = js_f64(&initial_coupon, "initialCoupon")?;
    let fixed_rate = js_f64(&fixed_rate, "fixedRate")?;
    let floating_fixings = js_f64_seq(&floating_fixings, "floatingFixings")?;
    let coupon_floor = js_f64(&coupon_floor, "couponFloor")?;
    let coupon_cap = js_opt_f64(coupon_cap.as_ref(), "couponCap")?;
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
    fixed_rate: JsValue,
    floating_fixings: JsValue,
    coupon_floor: JsValue,
    coupon_cap: Option<JsValue>,
    gearing: JsValue,
) -> Result<Box<[f64]>, JsValue> {
    let fixed_rate = js_f64(&fixed_rate, "fixedRate")?;
    let floating_fixings = js_f64_seq(&floating_fixings, "floatingFixings")?;
    let coupon_floor = js_f64(&coupon_floor, "couponFloor")?;
    let coupon_cap = js_opt_f64(coupon_cap.as_ref(), "couponCap")?;
    let gearing = js_f64(&gearing, "gearing")?;
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
    long_cms: JsValue,
    short_cms: JsValue,
    strike: JsValue,
    is_call: JsValue,
    notional: JsValue,
) -> Result<f64, JsValue> {
    let long_cms = js_f64(&long_cms, "longCms")?;
    let short_cms = js_f64(&short_cms, "shortCms")?;
    let strike = js_f64(&strike, "strike")?;
    let notional = js_f64(&notional, "notional")?;
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
    lower: JsValue,
    upper: JsValue,
    observations: JsValue,
    coupon_rate: JsValue,
    day_count_fraction: JsValue,
) -> Result<f64, JsValue> {
    let lower = js_f64(&lower, "lower")?;
    let upper = js_f64(&upper, "upper")?;
    let observations = js_f64_seq(&observations, "observations")?;
    let coupon_rate = js_f64(&coupon_rate, "couponRate")?;
    let day_count_fraction = js_f64(&day_count_fraction, "dayCountFraction")?;
    coupon_profiles::callable_range_accrual_accrued(
        lower,
        upper,
        &observations,
        coupon_rate,
        day_count_fraction,
    )
    .map_err(to_js_err)
}
