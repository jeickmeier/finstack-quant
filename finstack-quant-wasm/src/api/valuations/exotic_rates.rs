//! Deterministic coupon / payoff helpers for exotic rate products.
//!
//! Mirrors `finstack-quant-py`'s `valuations/exotic_rates.rs`: lightweight, market-
//! data-free helpers useful for building test fixtures and inspecting coupon
//! trajectories. Full MC / copula / LSMC pricers stay on the standard
//! `priceInstrument` / `priceInstrumentWithMetrics` pipeline.

use crate::utils::{to_js_err, to_js_value};
use finstack_quant_valuations::instruments::rates::exotics_shared::coupon_profiles;
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
/// Payments accumulate in a [`CumulativeCouponTracker`] configured with
/// `target_coupon`; once cumulative hits the target, the final coupon is
/// capped and the instrument is considered redeemed.
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

    let payload = serde_json::json!({
        "coupons_paid": profile.coupons_paid,
        "cumulative": profile.cumulative,
        "redemption_index": profile.redemption_index,
        "redeemed_early": profile.redeemed_early,
    });
    to_js_value(&payload)
}

/// Snowball / inverse-floater coupon schedule.
///
/// For snowball (`is_inverse_floater = false`):
///   `c_i = clip(c_{i-1} + fixed_rate - L_i, floor, cap)` with `c_0 = initial_coupon`.
///
/// For inverse floater (`is_inverse_floater = true`):
///   `c_i = clip(fixed_rate - leverage * L_i, floor, cap)` (path-independent).
#[wasm_bindgen(js_name = snowballCouponProfile)]
#[allow(clippy::too_many_arguments)]
pub fn snowball_coupon_profile(
    initial_coupon: f64,
    fixed_rate: f64,
    floating_fixings: Vec<f64>,
    floor: f64,
    cap: f64,
    is_inverse_floater: bool,
    leverage: Option<f64>,
) -> Result<Vec<f64>, JsValue> {
    let leverage = leverage.unwrap_or(1.0);
    coupon_profiles::snowball_coupon_profile(
        initial_coupon,
        fixed_rate,
        &floating_fixings,
        floor,
        cap,
        is_inverse_floater,
        leverage,
    )
    .map_err(to_js_err)
}

/// Intrinsic (undiscounted, unhedged) payoff of a CMS spread option.
///
/// `call:  notional * max(long_cms - short_cms - strike, 0)`
/// `put:   notional * max(strike - (long_cms - short_cms), 0)`
#[wasm_bindgen(js_name = cmsSpreadOptionIntrinsic)]
pub fn cms_spread_option_intrinsic(
    long_cms: f64,
    short_cms: f64,
    strike: f64,
    is_call: bool,
    notional: f64,
) -> Result<f64, JsValue> {
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
        let coupons =
            snowball_coupon_profile(0.02, 0.05, vec![0.01, 0.04, 0.03], 0.0, 0.10, false, None)
                .expect("snowball");
        assert_eq!(coupons.len(), 3);
        for c in coupons {
            assert!((0.0..=0.10).contains(&c));
        }
    }

    #[test]
    fn cms_spread_option_intrinsic_call_works() {
        let p = cms_spread_option_intrinsic(0.04, 0.02, 0.01, true, 1_000_000.0).expect("cms");
        assert!((p - 10_000.0).abs() < 1e-9);
    }
}
