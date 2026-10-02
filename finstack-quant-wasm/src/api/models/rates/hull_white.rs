//! WASM bindings for the Hull-White one-factor short-rate kernels.
//!
//! Mirrors `finstack-quant-py/src/bindings/models/rates/hull_white.rs`. The JS
//! facade nests these exports under `models.rates.hullWhite`.

use std::sync::Arc;

use crate::api::core::market_data::JsDiscountCurve;
use crate::utils::input::{from_js_json, js_bool, js_f64, js_f64_seq};
use crate::utils::to_js_err;
use finstack_quant_core::math::piecewise::PiecewiseConstantCurve;
use finstack_quant_models::rates::hull_white::{self, HullWhiteCalibrationParams, HullWhiteParams};
use wasm_bindgen::prelude::*;

/// Hull-White one-factor parameters: constant mean reversion and a
/// piecewise-constant volatility schedule.
#[wasm_bindgen(js_name = HullWhiteParams)]
pub struct JsHullWhiteParams {
    pub(crate) inner: HullWhiteParams,
}

json_round_trip!(JsHullWhiteParams, HullWhiteParams);

#[wasm_bindgen(js_class = HullWhiteParams)]
impl JsHullWhiteParams {
    /// Parameters with a constant volatility.
    /// @param kappa - Mean-reversion speed per year; positive.
    /// @param sigma - Short-rate volatility per square-root year, as a decimal; non-negative.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `kappa` is not positive or `sigma` is
    /// negative or non-finite.
    #[wasm_bindgen(constructor)]
    pub fn new(kappa: JsValue, sigma: JsValue) -> Result<JsHullWhiteParams, JsValue> {
        HullWhiteParams::constant(js_f64(&kappa, "kappa")?, js_f64(&sigma, "sigma")?)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Parameters with a piecewise-constant volatility schedule.
    /// @param kappa - Mean-reversion speed per year; positive.
    /// @param times - Segment start times in years: the first must be 0 and the rest strictly increasing; the last segment extends to infinity.
    /// @param values - Short-rate volatility on each segment, as decimals; same length as `times`, non-negative.
    /// @returns The piecewise-volatility parameters.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the schedule is empty, the lengths
    /// differ, the first time is not 0, the times are not increasing, or a
    /// value is out of range.
    pub fn piecewise(
        kappa: JsValue,
        times: JsValue,
        values: JsValue,
    ) -> Result<JsHullWhiteParams, JsValue> {
        let schedule = PiecewiseConstantCurve::new(
            js_f64_seq(&times, "times")?,
            js_f64_seq(&values, "values")?,
        )
        .map_err(to_js_err)?;
        HullWhiteParams::new(js_f64(&kappa, "kappa")?, schedule)
            .map(|inner| Self { inner })
            .map_err(to_js_err)
    }

    /// Mean-reversion speed per year.
    #[wasm_bindgen(getter)]
    pub fn kappa(&self) -> f64 {
        self.inner.kappa
    }

    /// Segment start times of the volatility schedule, in years.
    #[wasm_bindgen(getter)]
    pub fn times(&self) -> Box<[f64]> {
        self.inner.volatility.times().into()
    }

    /// Volatility on each schedule segment, as decimals.
    #[wasm_bindgen(getter)]
    pub fn values(&self) -> Box<[f64]> {
        self.inner.volatility.values().into()
    }

    /// Short-rate volatility in force at a time.
    /// @param t - Time in years from the valuation date.
    /// @returns The volatility of the segment containing `t`.
    ///
    /// # Errors
    ///
    /// Throws a `TypeError` if `t` is not a number.
    pub fn sigma(&self, t: JsValue) -> Result<f64, JsValue> {
        Ok(self.inner.volatility.value_at(js_f64(&t, "t")?))
    }

    /// Variance of the short-rate state variable at a time.
    /// @param t - Time in years from the valuation date; non-negative.
    /// @returns `Var[x(t)]`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if `t` is negative or non-finite.
    #[wasm_bindgen(js_name = stateVariance)]
    pub fn state_variance(&self, t: JsValue) -> Result<f64, JsValue> {
        self.inner
            .state_variance(js_f64(&t, "t")?)
            .map_err(to_js_err)
    }

    /// Covariance of the short-rate state variable at two times.
    /// @param left_time - First time in years; non-negative.
    /// @param right_time - Second time in years; non-negative.
    /// @returns `Cov[x(leftTime), x(rightTime)]`.
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if a time is negative or non-finite.
    #[wasm_bindgen(js_name = stateCovariance)]
    pub fn state_covariance(
        &self,
        left_time: JsValue,
        right_time: JsValue,
    ) -> Result<f64, JsValue> {
        self.inner
            .state_covariance(
                js_f64(&left_time, "leftTime")?,
                js_f64(&right_time, "rightTime")?,
            )
            .map_err(to_js_err)
    }

    /// Volatility of the forward zero-coupon bond price under the volatility schedule.
    /// @param t - Valuation time in years.
    /// @param expiry - Option expiry in years; at or after `t`.
    /// @param maturity - Bond maturity in years; at or after `expiry`.
    /// @returns The integrated bond-price volatility (total standard deviation of the log bond price).
    ///
    /// # Errors
    ///
    /// Throws a `validation` error if the times are out of order or non-finite.
    #[wasm_bindgen(js_name = bondVol)]
    pub fn bond_vol(&self, t: JsValue, expiry: JsValue, maturity: JsValue) -> Result<f64, JsValue> {
        hull_white::hw_bond_vol_with_model(
            &self.inner,
            js_f64(&t, "t")?,
            js_f64(&expiry, "expiry")?,
            js_f64(&maturity, "maturity")?,
        )
        .map_err(to_js_err)
    }
}

/// Futures-forward convexity adjustment under Hull-White one-factor.
/// @param kappa - Mean-reversion speed per year.
/// @param sigma - Short-rate volatility per square-root year, as a decimal.
/// @param t_settle - Futures settlement time in years.
/// @param t_end - End of the underlying rate period in years.
/// @returns The futures rate minus the forward rate, as a decimal.
///
/// # Errors
///
/// Throws a `TypeError` if an argument is not a number.
#[wasm_bindgen(js_name = hw1fConvexityAdjustment)]
pub fn hw1f_convexity_adjustment(
    kappa: JsValue,
    sigma: JsValue,
    t_settle: JsValue,
    t_end: JsValue,
) -> Result<f64, JsValue> {
    Ok(hull_white::hw1f_convexity_adjustment(
        js_f64(&kappa, "kappa")?,
        js_f64(&sigma, "sigma")?,
        js_f64(&t_settle, "tSettle")?,
        js_f64(&t_end, "tEnd")?,
    ))
}

/// Forward zero-coupon bond price volatility under constant Hull-White parameters.
/// @param kappa - Mean-reversion speed per year.
/// @param sigma - Short-rate volatility per square-root year, as a decimal.
/// @param t - Valuation time in years.
/// @param expiry - Option expiry in years.
/// @param maturity - Bond maturity in years.
/// @returns The integrated bond-price volatility (total standard deviation of the log bond price).
///
/// # Errors
///
/// Throws a `TypeError` if an argument is not a number.
#[wasm_bindgen(js_name = hwBondVol)]
pub fn hw_bond_vol(
    kappa: JsValue,
    sigma: JsValue,
    t: JsValue,
    expiry: JsValue,
    maturity: JsValue,
) -> Result<f64, JsValue> {
    Ok(hull_white::hw_bond_vol(
        js_f64(&kappa, "kappa")?,
        js_f64(&sigma, "sigma")?,
        js_f64(&t, "t")?,
        js_f64(&expiry, "expiry")?,
        js_f64(&maturity, "maturity")?,
    ))
}

/// Price of a European option on a zero-coupon bond (Jamshidian 1989).
/// @param p0_expiry - Discount factor to the option expiry.
/// @param p0_maturity - Discount factor to the bond maturity.
/// @param strike - Strike price of the bond, per unit of face value.
/// @param bond_vol - Integrated bond-price volatility, as returned by `hwBondVol`.
/// @param is_call - `true` for a call on the bond, `false` for a put.
/// @returns The option price per unit of face value.
///
/// # Errors
///
/// Throws a `TypeError` if an argument has the wrong JavaScript type.
#[wasm_bindgen(js_name = hw1fZcbOptionPrice)]
pub fn hw1f_zcb_option_price(
    p0_expiry: JsValue,
    p0_maturity: JsValue,
    strike: JsValue,
    bond_vol: JsValue,
    is_call: JsValue,
) -> Result<f64, JsValue> {
    Ok(hull_white::hw1f_zcb_option_price(
        js_f64(&p0_expiry, "p0Expiry")?,
        js_f64(&p0_maturity, "p0Maturity")?,
        js_f64(&strike, "strike")?,
        js_f64(&bond_vol, "bondVol")?,
        js_bool(&is_call, "isCall")?,
    ))
}

/// Normal volatility of a caplet's forward rate implied by Hull-White one-factor.
/// @param kappa - Mean-reversion speed per year.
/// @param sigma - Short-rate volatility per square-root year, as a decimal.
/// @param t_fix - Fixing time of the forward rate in years.
/// @param accrual - Accrual year fraction of the forward-rate period.
/// @returns The annualized normal (Bachelier) volatility of the forward rate, as a decimal.
///
/// # Errors
///
/// Throws a `TypeError` if an argument is not a number.
#[wasm_bindgen(js_name = hw1fCapletForwardRateNormalVol)]
pub fn hw1f_caplet_forward_rate_normal_vol(
    kappa: JsValue,
    sigma: JsValue,
    t_fix: JsValue,
    accrual: JsValue,
) -> Result<f64, JsValue> {
    Ok(hull_white::hw1f_caplet_forward_rate_normal_vol(
        js_f64(&kappa, "kappa")?,
        js_f64(&sigma, "sigma")?,
        js_f64(&t_fix, "tFix")?,
        js_f64(&accrual, "accrual")?,
    ))
}

/// Price of a cap or floor under Hull-White one-factor, per unit notional.
///
/// The facade makes `forwardCurve` optional and passes the discount curve when
/// it is omitted, as Python does.
/// @param kappa - Mean-reversion speed per year; positive.
/// @param sigma - Short-rate volatility per square-root year, as a decimal; positive.
/// @param periods - Array of `[tFix, tPay, accrual]` triples in years, one per caplet or floorlet.
/// @param strike - Cap or floor strike rate, as a decimal.
/// @param is_cap - `true` for a cap, `false` for a floor.
/// @param discount_curve - `core.DiscountCurve` handle used to discount the payoffs.
/// @param forward_curve - `core.DiscountCurve` handle that projects the forward rates.
/// @returns The cap or floor price per unit notional.
///
/// # Errors
///
/// Throws a `validation` error if `kappa` or `sigma` is out of range or
/// `periods` is malformed.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = hw1fCapFloorPrice)]
pub fn hw1f_cap_floor_price(
    kappa: JsValue,
    sigma: JsValue,
    periods: JsValue,
    strike: JsValue,
    is_cap: JsValue,
    discount_curve: &JsDiscountCurve,
    forward_curve: &JsDiscountCurve,
) -> Result<f64, JsValue> {
    let params =
        HullWhiteCalibrationParams::new(js_f64(&kappa, "kappa")?, js_f64(&sigma, "sigma")?)
            .map_err(to_js_err)?;
    let periods: Vec<(f64, f64, f64)> = from_js_json(&periods, "periods")?;
    let strike = js_f64(&strike, "strike")?;
    let is_cap = js_bool(&is_cap, "isCap")?;
    let discount = Arc::clone(&discount_curve.inner);
    let forward = Arc::clone(&forward_curve.inner);
    Ok(hull_white::hw1f_cap_floor_price(
        params,
        &|t: f64| discount.df(t),
        &|t: f64| forward.df(t),
        &periods,
        strike,
        is_cap,
    ))
}
