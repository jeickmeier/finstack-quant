use super::*;

/// Price a full cap/floor with a flat normal volatility quote.
pub(crate) fn bachelier_cap_floor_price(
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    maturity: f64,
    strike: f64,
    normal_vol: f64,
    is_cap: bool,
    frequency: SwapFrequency,
) -> f64 {
    cap_floor_periods(maturity, frequency)
        .map(|(t_start, t_end, accrual)| {
            let forward = forward_rate_from_df(forward_df, t_start, t_end);
            let df = discount_df(t_end);
            // Option expiry is the fixing time `t_start`, not the payment
            // time `t_end`: the caplet's rate is fixed at the period start
            // and accrues no vol afterwards.
            normal_caplet_price(forward, strike, normal_vol, t_start, accrual, df, is_cap)
        })
        .sum()
}

pub(super) fn cap_floor_bachelier_vega(
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    maturity: f64,
    strike: f64,
    normal_vol: f64,
    frequency: SwapFrequency,
) -> f64 {
    cap_floor_periods(maturity, frequency)
        .map(|(t_start, t_end, accrual)| {
            let forward = forward_rate_from_df(forward_df, t_start, t_end);
            let df = discount_df(t_end);
            // Vol accrues only to the fixing time `t_start` (see
            // `bachelier_cap_floor_price`).
            normal_caplet_vega(forward, strike, normal_vol, t_start) * accrual * df
        })
        .sum()
}

/// Cap/floor shape used by HW1F pricing helpers.
#[derive(Clone, Copy)]
pub(crate) struct CapFloorPriceSpec {
    pub(super) maturity: f64,
    pub(super) strike: f64,
    pub(super) is_cap: bool,
    pub(super) frequency: SwapFrequency,
}

impl CapFloorPriceSpec {
    pub(crate) fn new(maturity: f64, strike: f64, is_cap: bool, frequency: SwapFrequency) -> Self {
        Self {
            maturity,
            strike,
            is_cap,
            frequency,
        }
    }

    pub(super) fn from_quote(quote: &CapFloorQuote, frequency: SwapFrequency) -> Self {
        Self::new(quote.maturity, quote.strike, quote.is_cap, frequency)
    }
}

/// Price a full cap/floor exactly under HW1F by pricing each caplet as a
/// zero-coupon bond option.
///
/// A caplet fixing at `T`, paying `τ·max(L(T,S) − K, 0)` at `S`, equals
/// `(1 + τK)` zero-coupon bond **puts** with strike `X = 1/(1 + τK)` on
/// `P(T,S)`, expiring at `T`; a floorlet is the corresponding bond **call**
/// (Brigo–Mercurio §2.6 / Hull §31). The ZCB option is priced with the same
/// HW1F bond-option formula used by the Jamshidian swaption decomposition
/// ([`hw_bond_vol`]). This replaces the earlier mapping of HW bond vol to an
/// approximate forward-rate normal vol, which understated the caplet vol by
/// a `(1 + τF)` factor.
///
/// Dual-curve handling: the ZCB option is evaluated on the forward
/// (projection) curve and scaled by the deterministic discount/projection
/// basis `P_d(0,S)/P_f(0,S)`; for single-curve calibration the factor is 1
/// and the price is exact.
pub(crate) fn hw1f_cap_floor_price(
    kappa: f64,
    sigma: f64,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    spec: CapFloorPriceSpec,
) -> f64 {
    let periods: Vec<_> = cap_floor_periods(spec.maturity, spec.frequency).collect();
    finstack_quant_models::rates::hull_white::hw1f_cap_floor_price(
        HullWhiteCalibrationParams { kappa, sigma },
        discount_df,
        forward_df,
        &periods,
        spec.strike,
        spec.is_cap,
    )
}

/// Price a full cap/floor under a scheduled HW1F short-rate volatility.
pub(crate) fn hw1f_cap_floor_price_with_model(
    params: &HullWhiteParams,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    spec: CapFloorPriceSpec,
) -> finstack_quant_core::Result<f64> {
    let periods: Vec<_> = cap_floor_periods(spec.maturity, spec.frequency)
        .map(|(t_start, t_end, accrual)| (t_start, t_start, t_end, accrual))
        .collect();
    finstack_quant_models::rates::hull_white::hw1f_cap_floor_price_with_model(
        params,
        discount_df,
        forward_df,
        &periods,
        spec.strike,
        spec.is_cap,
    )
}

/// Return the flat normal vol that reproduces the HW1F cap/floor model price.
#[cfg(test)]
pub(crate) fn hw1f_cap_floor_implied_normal_vol(
    kappa: f64,
    sigma: f64,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    spec: CapFloorPriceSpec,
) -> finstack_quant_core::Result<f64> {
    let target = hw1f_cap_floor_price(kappa, sigma, discount_df, forward_df, spec);
    cap_floor_implied_normal_vol(target, discount_df, forward_df, spec)
}

pub(super) fn cap_floor_implied_normal_vol(
    target: f64,
    discount_df: &dyn Fn(f64) -> f64,
    forward_df: &dyn Fn(f64) -> f64,
    spec: CapFloorPriceSpec,
) -> finstack_quant_core::Result<f64> {
    if !target.is_finite() || target < 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "Hull-White cap price must be finite and non-negative for quote inversion".into(),
        ));
    }
    let residual = |vol: f64| {
        bachelier_cap_floor_price(
            discount_df,
            forward_df,
            spec.maturity,
            spec.strike,
            vol,
            spec.is_cap,
            spec.frequency,
        ) - target
    };
    let mut hi = 0.01;
    while residual(hi) < 0.0 && hi < 100.0 {
        hi *= 2.0;
    }
    BrentSolver::new()
        .tolerance(1e-14)
        .solve_in_bracket(residual, 0.0, hi)
}

/// Caplet periods `(t_start, t_end, accrual)` for a spot-start cap quote.
///
/// The first (spot-start) caplet is **excluded**: its rate fixes at `t = 0`,
/// so it carries no optionality, and standard market cap quotes exclude it.
/// Both the market (Bachelier) and model (HW1F) legs use this iterator, so
/// the convention is applied consistently to both sides of the calibration.
pub(super) fn cap_floor_periods(
    maturity: f64,
    frequency: SwapFrequency,
) -> impl Iterator<Item = (f64, f64, f64)> {
    let periods = (maturity * frequency.periods_per_year() as f64)
        .round()
        .max(1.0) as usize;
    let accrual = maturity / periods as f64;
    (1..periods).map(move |idx| {
        let start = idx as f64 * accrual;
        let end = (idx + 1) as f64 * accrual;
        (start, end, accrual)
    })
}

/// Simple forward rate between `start` and `end` from a discount-factor
/// function.
///
/// Non-finite or non-positive discount factors propagate as `NaN` instead of
/// being clamped: callers (`HullWhiteCapFloorTarget::calculate_residuals`,
/// `solve_cap_floor_sigma_for_fixed_kappa`) rely on the non-finite-price
/// check to detect broken curves, and `f64::max` would silently absorb a NaN
/// (`NaN.max(1e-12) == 1e-12`), defeating that error contract.
pub(super) fn forward_rate_from_df(df: &dyn Fn(f64) -> f64, start: f64, end: f64) -> f64 {
    let accrual = (end - start).max(1e-12);
    let p_start = df(start);
    let p_end = df(end);
    if !p_start.is_finite() || !p_end.is_finite() || p_start <= 0.0 || p_end <= 0.0 {
        return f64::NAN;
    }
    (p_start / p_end - 1.0) / accrual
}

pub(super) fn normal_caplet_price(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    accrual: f64,
    df: f64,
    is_cap: bool,
) -> f64 {
    let annuity = accrual * df;
    if vol <= 0.0 || expiry <= 0.0 {
        let intrinsic = if is_cap {
            (forward - strike).max(0.0)
        } else {
            (strike - forward).max(0.0)
        };
        return intrinsic * annuity;
    }
    let sqrt_t = expiry.sqrt();
    let d = (forward - strike) / (vol * sqrt_t);
    let undiscounted = if is_cap {
        (forward - strike) * norm_cdf(d) + vol * sqrt_t * norm_pdf(d)
    } else {
        (strike - forward) * norm_cdf(-d) + vol * sqrt_t * norm_pdf(d)
    };
    undiscounted * annuity
}

fn normal_caplet_vega(forward: f64, strike: f64, vol: f64, expiry: f64) -> f64 {
    if vol <= 0.0 || expiry <= 0.0 {
        return 0.0;
    }
    let d = (forward - strike) / (vol * expiry.sqrt());
    expiry.sqrt() * norm_pdf(d)
}
