//! Product-independent Hull-White one-factor parameters and pricing kernels.
//!
//! This module owns the reusable equations of the Hull-White one-factor
//! short-rate model. Quote preparation and fitting live in the calibration
//! crate; fitted-input resolution and instrument pricing live in valuations.

use finstack_quant_core::math::piecewise::PiecewiseConstantCurve;
use finstack_quant_core::math::special_functions::norm_cdf;
use finstack_quant_core::{Error, Result};

/// Market-scalar keys for swaption-calibrated HW1F mean reversion and volatility.
///
/// # Arguments
///
/// * `curve_id` - Discount or projection curve identifier prefixed into both keys.
#[must_use]
pub fn hw1f_scalar_keys(curve_id: &str) -> (String, String) {
    (
        format!("{curve_id}_HW1F_KAPPA"),
        format!("{curve_id}_HW1F_SIGMA"),
    )
}

/// Market-scalar keys for cap/floor-calibrated HW1F mean reversion and volatility.
///
/// # Arguments
///
/// * `curve_id` - Discount or projection curve identifier prefixed into both keys.
#[must_use]
pub fn capfloor_hw1f_scalar_keys(curve_id: &str) -> (String, String) {
    (
        format!("{curve_id}_CAPFLOOR_HW1F_KAPPA"),
        format!("{curve_id}_CAPFLOOR_HW1F_SIGMA"),
    )
}

/// Market-series key for a cap/floor-calibrated piecewise HW1F sigma schedule.
///
/// # Arguments
///
/// * `curve_id` - Discount or projection curve identifier prefixed into the key.
#[must_use]
pub fn capfloor_hw1f_sigma_schedule_key(curve_id: &str) -> String {
    format!("{curve_id}_CAPFLOOR_HW1F_SIGMA_SCHEDULE")
}

/// Validated constant-parameter Hull-White one-factor model.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::rates::hull_white::HullWhiteCalibrationParams;
///
/// let params = HullWhiteCalibrationParams::new(0.05, 0.01).unwrap();
/// assert!(!params.is_uncalibrated_default());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawHullWhiteCalibrationParams")]
pub struct HullWhiteCalibrationParams {
    /// Mean-reversion speed κ in inverse years; strictly positive and finite.
    pub kappa: f64,
    /// Short-rate volatility σ in absolute rate units per square-root year;
    /// strictly positive and finite.
    pub sigma: f64,
}

#[derive(serde::Deserialize)]
struct RawHullWhiteCalibrationParams {
    kappa: f64,
    sigma: f64,
}

impl TryFrom<RawHullWhiteCalibrationParams> for HullWhiteCalibrationParams {
    type Error = Error;

    fn try_from(raw: RawHullWhiteCalibrationParams) -> Result<Self> {
        Self::new(raw.kappa, raw.sigma)
    }
}

impl Default for HullWhiteCalibrationParams {
    /// Return generic uncalibrated initialization parameters.
    ///
    /// The defaults κ=3% and σ=1% are suitable for tests and initialization,
    /// but callers should calibrate or explicitly choose parameters for
    /// production pricing.
    fn default() -> Self {
        Self {
            kappa: 0.03,
            sigma: 0.01,
        }
    }
}

impl HullWhiteCalibrationParams {
    /// Construct validated constant Hull-White parameters.
    ///
    /// # Arguments
    ///
    /// * `kappa` - Mean-reversion speed κ in inverse years; must be positive
    ///   and finite.
    /// * `sigma` - Short-rate volatility σ in absolute rate units per
    ///   square-root year; must be positive and finite.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when either input is non-finite or not
    /// strictly positive.
    pub fn new(kappa: f64, sigma: f64) -> Result<Self> {
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(Error::Validation(format!(
                "Hull-White kappa (mean reversion) must be positive, got {kappa}"
            )));
        }
        if sigma <= 0.0 || !sigma.is_finite() {
            return Err(Error::Validation(format!(
                "Hull-White sigma (short rate volatility) must be positive, got {sigma}"
            )));
        }
        Ok(Self { kappa, sigma })
    }

    /// Return whether the parameters equal the generic uncalibrated defaults.
    #[must_use]
    pub fn is_uncalibrated_default(&self) -> bool {
        (self.kappa - 0.03).abs() < f64::EPSILON && (self.sigma - 0.01).abs() < f64::EPSILON
    }
}

/// Hull-White parameters with piecewise-constant short-rate volatility.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawHullWhiteParams")]
pub struct HullWhiteParams {
    /// Mean-reversion speed κ in inverse years; strictly positive and finite.
    pub kappa: f64,
    /// Left-continuous short-rate volatility σ(t), in absolute rate units per
    /// square-root year.
    pub volatility: PiecewiseConstantCurve,
}

#[derive(serde::Deserialize)]
struct RawHullWhiteParams {
    kappa: f64,
    volatility: PiecewiseConstantCurve,
}

impl TryFrom<RawHullWhiteParams> for HullWhiteParams {
    type Error = Error;

    fn try_from(raw: RawHullWhiteParams) -> Result<Self> {
        Self::new(raw.kappa, raw.volatility)
    }
}

impl HullWhiteParams {
    /// Construct a model from mean reversion and a validated volatility schedule.
    ///
    /// # Arguments
    ///
    /// * `kappa` - Mean-reversion speed κ in inverse years; must be positive
    ///   and finite.
    /// * `volatility` - Left-continuous piecewise-constant σ(t) schedule.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when `kappa` is non-finite or not
    /// strictly positive.
    pub fn new(kappa: f64, volatility: PiecewiseConstantCurve) -> Result<Self> {
        if !kappa.is_finite() || kappa <= 0.0 {
            return Err(Error::Validation(format!(
                "Hull-White kappa must be positive and finite, got {kappa}"
            )));
        }
        Ok(Self { kappa, volatility })
    }

    /// Construct a constant-volatility model.
    ///
    /// # Arguments
    ///
    /// * `kappa` - Mean-reversion speed κ in inverse years; must be positive
    ///   and finite.
    /// * `sigma` - Constant short-rate volatility in absolute rate units per
    ///   square-root year; must be positive and finite.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] when either parameter violates its
    /// positivity or finiteness contract.
    pub fn constant(kappa: f64, sigma: f64) -> Result<Self> {
        Self::new(kappa, PiecewiseConstantCurve::constant(sigma)?)
    }

    /// Compute centered short-rate state variance `Var[x(t)]`.
    ///
    /// # Arguments
    ///
    /// * `t` - Model time in years at which to evaluate the variance.
    ///
    /// # Errors
    ///
    /// Returns an error when the integration interval or volatility schedule
    /// is invalid.
    pub fn state_variance(&self, t: f64) -> Result<f64> {
        self.volatility
            .integrate_squared_exp_weight(self.kappa, t, 0.0, t)
    }

    /// Compute covariance of the centered short-rate state at two times.
    ///
    /// # Arguments
    ///
    /// * `left_time` - First model time in years.
    /// * `right_time` - Second model time in years.
    ///
    /// # Errors
    ///
    /// Returns an error when evaluation of the earlier-time variance fails.
    pub fn state_covariance(&self, left_time: f64, right_time: f64) -> Result<f64> {
        let earlier = left_time.min(right_time);
        if earlier <= 0.0 {
            return Ok(0.0);
        }
        let variance = self.state_variance(earlier)?;
        Ok(variance * (-self.kappa * (left_time - right_time).abs()).exp())
    }
}

impl TryFrom<HullWhiteCalibrationParams> for HullWhiteParams {
    type Error = Error;

    fn try_from(params: HullWhiteCalibrationParams) -> Result<Self> {
        Self::constant(params.kappa, params.sigma)
    }
}

/// Evaluate `B(t1,t2) = (1 - exp(-κ(t2-t1))) / κ`.
///
/// Uses the continuous Ho-Lee limit when κ is close to zero.
///
/// # Arguments
///
/// * `kappa` - Mean-reversion speed κ in inverse years.
/// * `t1` - Earlier model time in years.
/// * `t2` - Later model time in years.
#[must_use]
pub fn hw_b(kappa: f64, t1: f64, t2: f64) -> f64 {
    let tau = t2 - t1;
    if kappa.abs() < 1e-10 {
        tau
    } else {
        (1.0 - (-kappa * tau).exp()) / kappa
    }
}

/// Compute the Hull-White futures-to-forward convexity adjustment.
///
/// The returned decimal-rate adjustment satisfies
/// `forward = futures_rate - adjustment` and uses the Ho-Lee limit for small κ.
///
/// # Arguments
///
/// * `kappa` - Mean-reversion speed κ in inverse years.
/// * `sigma` - Short-rate volatility σ in absolute rate units per square-root year.
/// * `t_settle` - Futures settlement time in years from valuation.
/// * `t_end` - Futures underlying end time in years from valuation.
#[must_use]
pub fn hw1f_convexity_adjustment(kappa: f64, sigma: f64, t_settle: f64, t_end: f64) -> f64 {
    let tau = t_end - t_settle;
    if t_settle <= 0.0 || tau <= 0.0 {
        return 0.0;
    }
    const SMALL_KAPPA: f64 = 1e-8;
    if kappa.abs() < SMALL_KAPPA {
        return 0.5 * sigma * sigma * t_settle * t_end;
    }
    let b_0s = hw_b(kappa, 0.0, t_settle);
    let b_se = hw_b(kappa, t_settle, t_end);
    let bracket = b_se * (1.0 - (-2.0 * kappa * t_settle).exp()) + 2.0 * kappa * b_0s * b_0s;
    sigma * sigma / (4.0 * kappa) * (b_se / tau) * bracket
}

/// Compute zero-coupon bond-option volatility under constant σ.
///
/// # Arguments
///
/// * `kappa` - Mean-reversion speed κ in inverse years.
/// * `sigma` - Constant short-rate volatility σ in absolute rate units per
///   square-root year.
/// * `t` - Current model time in years.
/// * `expiry` - Bond-option expiry time in years.
/// * `maturity` - Underlying bond maturity time in years.
#[must_use]
pub fn hw_bond_vol(kappa: f64, sigma: f64, t: f64, expiry: f64, maturity: f64) -> f64 {
    let b = hw_b(kappa, expiry, maturity);
    let var_factor = if kappa.abs() < 1e-10 {
        expiry - t
    } else {
        (1.0 - (-2.0 * kappa * (expiry - t)).exp()) / (2.0 * kappa)
    };
    b * sigma * var_factor.max(0.0).sqrt()
}

/// Compute zero-coupon bond-option volatility under scheduled σ(t).
///
/// # Arguments
///
/// * `params` - Validated mean reversion and piecewise volatility schedule.
/// * `t` - Current model time in years.
/// * `expiry` - Bond-option expiry time in years.
/// * `maturity` - Underlying bond maturity time in years.
///
/// # Errors
///
/// Returns [`Error::Validation`] for non-finite, negative, or incorrectly
/// ordered times, or when schedule integration fails.
pub fn hw_bond_vol_with_model(
    params: &HullWhiteParams,
    t: f64,
    expiry: f64,
    maturity: f64,
) -> Result<f64> {
    if !t.is_finite()
        || !expiry.is_finite()
        || !maturity.is_finite()
        || t < 0.0
        || expiry < t
        || maturity < expiry
    {
        return Err(Error::Validation(format!(
            "invalid HW bond option times t={t}, T={expiry}, S={maturity}"
        )));
    }
    let b = hw_b(params.kappa, expiry, maturity);
    let variance =
        params
            .volatility
            .integrate_squared_exp_weight(params.kappa, expiry, t, expiry)?;
    Ok(b * variance.max(0.0).sqrt())
}

/// Compute `ln A(t,T)` for the affine Hull-White zero-coupon bond price.
///
/// # Arguments
///
/// * `kappa` - Mean-reversion speed κ in inverse years.
/// * `sigma` - Constant short-rate volatility σ in absolute rate units per
///   square-root year.
/// * `t` - Bond valuation time in years.
/// * `maturity` - Bond maturity time in years.
/// * `df` - Initial discount-factor function returning `P(0,u)` for time `u`
///   in years.
#[must_use]
pub fn hw_ln_a(kappa: f64, sigma: f64, t: f64, maturity: f64, df: &dyn Fn(f64) -> f64) -> f64 {
    let p0t = df(t);
    let p0_maturity = df(maturity);
    let b = hw_b(kappa, t, maturity);
    let f0t = fd_instantaneous_forward(df, t).unwrap_or(0.0);
    let var_term = if kappa.abs() < 1e-10 {
        sigma * sigma * t * b * b / 2.0
    } else {
        sigma * sigma / (4.0 * kappa) * (1.0 - (-2.0 * kappa * t).exp()) * b * b
    };
    (p0_maturity / p0t).ln() + b * f0t - var_term
}

/// Price a zero-coupon bond option from discount factors and bond volatility.
///
/// # Arguments
///
/// * `p0_expiry` - Initial discount factor to the option expiry.
/// * `p0_maturity` - Initial discount factor to the underlying bond maturity.
/// * `strike` - Bond-price strike paid at option expiry.
/// * `bond_vol` - Integrated lognormal bond-price volatility through expiry.
/// * `is_call` - `true` for a bond call and `false` for a bond put.
#[must_use]
pub fn hw1f_zcb_option_price(
    p0_expiry: f64,
    p0_maturity: f64,
    strike: f64,
    bond_vol: f64,
    is_call: bool,
) -> f64 {
    if bond_vol < 1e-15 {
        return if is_call {
            (p0_maturity - strike * p0_expiry).max(0.0)
        } else {
            (strike * p0_expiry - p0_maturity).max(0.0)
        };
    }
    let d1 = (p0_maturity / (strike * p0_expiry)).ln() / bond_vol + 0.5 * bond_vol;
    let d2 = d1 - bond_vol;
    let value = if is_call {
        p0_maturity * norm_cdf(d1) - strike * p0_expiry * norm_cdf(d2)
    } else {
        strike * p0_expiry * norm_cdf(-d2) - p0_maturity * norm_cdf(-d1)
    };
    if value < 0.0 {
        0.0
    } else {
        value
    }
}

/// Price an exact constant-parameter term-index caplet or floorlet from DFs.
///
/// The result is per unit notional and uses the zero-coupon bond-option
/// equivalence with deterministic discount/projection basis scaling.
///
/// # Arguments
///
/// * `kappa` - Mean-reversion speed κ in inverse years.
/// * `sigma` - Constant short-rate volatility σ in absolute rate units per
///   square-root year.
/// * `pf_fix` - Projection-curve discount factor to fixing.
/// * `pf_pay` - Projection-curve discount factor to payment.
/// * `pd_pay` - Discount-curve discount factor to payment.
/// * `t_fix` - Fixing time in years.
/// * `t_pay` - Payment time in years.
/// * `accrual` - Positive coupon accrual fraction in years.
/// * `strike` - Caplet or floorlet strike as a decimal rate.
/// * `is_cap` - `true` for a caplet and `false` for a floorlet.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn hw1f_caplet_price_from_dfs(
    kappa: f64,
    sigma: f64,
    pf_fix: f64,
    pf_pay: f64,
    pd_pay: f64,
    t_fix: f64,
    t_pay: f64,
    accrual: f64,
    strike: f64,
    is_cap: bool,
) -> f64 {
    let valid_df = |value: f64| value.is_finite() && value > 0.0;
    if !valid_df(pf_fix) || !valid_df(pf_pay) || !valid_df(pd_pay) {
        return f64::NAN;
    }
    let basis = pd_pay / pf_pay;
    let gearing = 1.0 + accrual * strike;
    if gearing <= 0.0 {
        if is_cap {
            let forward = (pf_fix / pf_pay - 1.0) / accrual;
            return basis * pf_pay * accrual * (forward - strike);
        }
        return 0.0;
    }
    let bond_strike = 1.0 / gearing;
    let bond_vol = hw_bond_vol(kappa, sigma, 0.0, t_fix, t_pay);
    let zcb_option = hw1f_zcb_option_price(pf_fix, pf_pay, bond_strike, bond_vol, !is_cap);
    basis * gearing * zcb_option
}

/// Price an exact term-index caplet or floorlet under scheduled σ(t).
///
/// # Arguments
///
/// * `params` - Validated mean reversion and piecewise volatility schedule.
/// * `pf_start` - Projection-curve discount factor to coupon-period start.
/// * `pf_end` - Projection-curve discount factor to coupon-period end.
/// * `pd_pay` - Discount-curve discount factor to payment.
/// * `t_fix` - Contractual fixing time in years.
/// * `t_start` - Coupon-period start time in years.
/// * `t_end` - Coupon-period end time in years.
/// * `accrual` - Positive coupon accrual fraction in years.
/// * `strike` - Caplet or floorlet strike as a decimal rate.
/// * `is_cap` - `true` for a caplet and `false` for a floorlet.
///
/// # Errors
///
/// Returns [`Error::Validation`] when discount factors, times, or accrual are
/// invalid, or when scheduled bond-volatility integration fails.
#[allow(clippy::too_many_arguments)]
pub fn hw1f_term_caplet_price_from_dfs_with_model(
    params: &HullWhiteParams,
    pf_start: f64,
    pf_end: f64,
    pd_pay: f64,
    t_fix: f64,
    t_start: f64,
    t_end: f64,
    accrual: f64,
    strike: f64,
    is_cap: bool,
) -> Result<f64> {
    let valid_df = |value: f64| value.is_finite() && value > 0.0;
    if !valid_df(pf_start)
        || !valid_df(pf_end)
        || !valid_df(pd_pay)
        || accrual <= 0.0
        || t_end <= t_start
        || t_start < t_fix
    {
        return Err(Error::Validation(
            "invalid term caplet discount factors or times".into(),
        ));
    }
    let ratio_forward = pf_start / pf_end;
    let ratio_strike = 1.0 + accrual * strike;
    if ratio_strike <= 0.0 {
        return if is_cap {
            Ok(pd_pay * (ratio_forward - ratio_strike))
        } else {
            Ok(0.0)
        };
    }
    let ratio_vol = (hw_bond_vol_with_model(params, 0.0, t_fix, t_end)?
        - hw_bond_vol_with_model(params, 0.0, t_fix, t_start)?)
    .abs();
    if ratio_vol < 1.0e-15 {
        let intrinsic = if is_cap {
            (ratio_forward - ratio_strike).max(0.0)
        } else {
            (ratio_strike - ratio_forward).max(0.0)
        };
        return Ok(pd_pay * intrinsic);
    }
    let d1 = (ratio_forward / ratio_strike).ln() / ratio_vol + 0.5 * ratio_vol;
    let d2 = d1 - ratio_vol;
    let option = if is_cap {
        ratio_forward * norm_cdf(d1) - ratio_strike * norm_cdf(d2)
    } else {
        ratio_strike * norm_cdf(-d2) - ratio_forward * norm_cdf(-d1)
    };
    Ok(pd_pay * option.max(0.0))
}

/// Price a cap or floor under constant Hull-White parameters.
///
/// # Arguments
///
/// * `params` - Validated constant Hull-White parameters.
/// * `discount_df` - Discount-factor function for payment discounting.
/// * `forward_df` - Projection-curve discount-factor function.
/// * `periods` - Coupon periods as `(fixing_time, payment_time, accrual)` in years.
/// * `strike` - Cap or floor strike as a decimal rate.
/// * `is_cap` - `true` for a cap and `false` for a floor.
#[must_use]
pub fn hw1f_cap_floor_price(
    params: HullWhiteCalibrationParams,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    periods: &[(f64, f64, f64)],
    strike: f64,
    is_cap: bool,
) -> f64 {
    periods
        .iter()
        .map(|&(t_fix, t_pay, accrual)| {
            hw1f_caplet_price_from_dfs(
                params.kappa,
                params.sigma,
                forward_df(t_fix),
                forward_df(t_pay),
                discount_df(t_pay),
                t_fix,
                t_pay,
                accrual,
                strike,
                is_cap,
            )
        })
        .sum()
}

/// Price a cap or floor under scheduled Hull-White volatility.
///
/// # Arguments
///
/// * `params` - Validated mean reversion and piecewise volatility schedule.
/// * `discount_df` - Discount-factor function for payment discounting.
/// * `forward_df` - Projection-curve discount-factor function.
/// * `periods` - Coupon periods as `(fixing_time, start_time, end_time,
///   accrual)` in years.
/// * `strike` - Cap or floor strike as a decimal rate.
/// * `is_cap` - `true` for a cap and `false` for a floor.
///
/// # Errors
///
/// Returns an error when any caplet input is invalid or scheduled volatility
/// integration fails.
pub fn hw1f_cap_floor_price_with_model(
    params: &HullWhiteParams,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    periods: &[(f64, f64, f64, f64)],
    strike: f64,
    is_cap: bool,
) -> Result<f64> {
    periods
        .iter()
        .map(|&(t_fix, t_start, t_end, accrual)| {
            hw1f_term_caplet_price_from_dfs_with_model(
                params,
                forward_df(t_start),
                forward_df(t_end),
                discount_df(t_end),
                t_fix,
                t_start,
                t_end,
                accrual,
                strike,
                is_cap,
            )
        })
        .sum()
}

/// Convert constant Hull-White σ to the approximate caplet normal volatility.
///
/// # Arguments
///
/// * `kappa` - Mean-reversion speed κ in inverse years.
/// * `sigma` - Constant short-rate volatility σ in absolute rate units per
///   square-root year.
/// * `t_fix` - Caplet fixing time in years.
/// * `accrual` - Positive coupon accrual fraction in years.
#[must_use]
pub fn hw1f_caplet_forward_rate_normal_vol(
    kappa: f64,
    sigma: f64,
    t_fix: f64,
    accrual: f64,
) -> f64 {
    if sigma <= 0.0 || t_fix <= 0.0 || accrual <= 0.0 {
        return 0.0;
    }
    const SMALL_KAPPA: f64 = 1e-8;
    let accrual_factor = if kappa.abs() < SMALL_KAPPA {
        1.0
    } else {
        (1.0 - (-kappa * accrual).exp()) / (kappa * accrual)
    };
    let integrated_variance_time = if kappa.abs() < SMALL_KAPPA {
        t_fix
    } else {
        (1.0 - (-2.0 * kappa * t_fix).exp()) / (2.0 * kappa)
    };
    sigma * accrual_factor * (integrated_variance_time / t_fix).sqrt()
}

/// Instantaneous forward `f(0,t) = -d/dt ln P(0,t)` by finite difference.
///
/// Uses a central difference with step `h = clamp(t * 1e-3, 1e-6, 1e-3)`
/// where there is room, and a one-sided forward difference against
/// `P(0,0) = 1` for `t <= h`.
///
/// # Arguments
///
/// * `df` - Initial discount-factor function returning `P(0,u)` for time `u`
///   in years.
/// * `t` - Time in years at which the forward is evaluated; `t < 0` is
///   treated like `t = 0`.
///
/// # Returns
///
/// `None` when a sampled discount factor is not strictly positive (a
/// corrupted or badly extrapolated curve), so callers choose their own
/// fallback instead of receiving a silent `NaN`.
#[must_use]
pub fn fd_instantaneous_forward(df: &dyn Fn(f64) -> f64, t: f64) -> Option<f64> {
    let h = (t * 1e-3).clamp(1e-6, 1e-3);
    if t > h {
        let dfp = df(t + h);
        let dfm = df(t - h);
        (dfp > 0.0 && dfm > 0.0).then(|| -(dfp.ln() - dfm.ln()) / (2.0 * h))
    } else {
        let dfh = df(h);
        (dfh > 0.0).then(|| -dfh.ln() / h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_and_scheduled_variance_agree() {
        let scalar = HullWhiteCalibrationParams::new(0.05, 0.01).expect("scalar parameters");
        let model = HullWhiteParams::try_from(scalar).expect("constant model");
        let expected = scalar.sigma * scalar.sigma * (1.0 - (-0.1_f64).exp()) / 0.1;
        let actual = model.state_variance(1.0).expect("state variance");
        assert!((actual - expected).abs() < 1.0e-14);
    }

    #[test]
    fn constant_and_scheduled_bond_vol_agree() {
        let scalar = HullWhiteCalibrationParams::new(0.05, 0.01).expect("scalar parameters");
        let model = HullWhiteParams::try_from(scalar).expect("constant model");
        let expected = hw_bond_vol(scalar.kappa, scalar.sigma, 0.0, 1.0, 2.0);
        let actual = hw_bond_vol_with_model(&model, 0.0, 1.0, 2.0).expect("model vol");
        assert!((actual - expected).abs() < 1.0e-14);
    }

    #[test]
    fn convexity_uses_ho_lee_limit() {
        let sigma = 0.01;
        let t1 = 1.0;
        let t2 = 1.25;
        let expected = 0.5 * sigma * sigma * t1 * t2;
        let actual = hw1f_convexity_adjustment(1.0e-12, sigma, t1, t2);
        assert!((actual - expected).abs() < 1.0e-14);
    }

    #[test]
    fn zcb_option_satisfies_put_call_parity() {
        let p_expiry = 0.98;
        let p_maturity = 0.94;
        let strike = 0.96;
        let vol = 0.03;
        let call = hw1f_zcb_option_price(p_expiry, p_maturity, strike, vol, true);
        let put = hw1f_zcb_option_price(p_expiry, p_maturity, strike, vol, false);
        assert!((call - put - (p_maturity - strike * p_expiry)).abs() < 1.0e-14);
    }

    #[test]
    fn caplet_and_floorlet_satisfy_parity() {
        let kappa = 0.05;
        let sigma = 0.01;
        let pf_fix = (-0.03_f64).exp();
        let pf_pay = (-0.06_f64).exp();
        let pd_pay = pf_pay;
        let accrual = 1.0;
        let strike = 0.025;
        let caplet = hw1f_caplet_price_from_dfs(
            kappa, sigma, pf_fix, pf_pay, pd_pay, 1.0, 2.0, accrual, strike, true,
        );
        let floorlet = hw1f_caplet_price_from_dfs(
            kappa, sigma, pf_fix, pf_pay, pd_pay, 1.0, 2.0, accrual, strike, false,
        );
        let forward = (pf_fix / pf_pay - 1.0) / accrual;
        assert!((caplet - floorlet - pd_pay * accrual * (forward - strike)).abs() < 1.0e-14);
    }
}
