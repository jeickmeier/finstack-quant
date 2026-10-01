use super::*;

/// Price a full cap/floor with a flat normal volatility quote.
#[cfg(test)]
pub(crate) fn bachelier_cap_floor_price(
    discount_df: &(dyn Fn(f64) -> f64 + Sync),
    forward_df: &(dyn Fn(f64) -> f64 + Sync),
    maturity: f64,
    strike: f64,
    normal_vol: f64,
    is_cap: bool,
    frequency: SwapFrequency,
) -> f64 {
    scheduled_bachelier_cap_floor_price(
        discount_df,
        forward_df,
        &CapFloorSchedule::synthetic(maturity, frequency),
        strike,
        normal_vol,
        is_cap,
    )
}

pub(super) fn scheduled_bachelier_cap_floor_price(
    discount_df: &(dyn Fn(f64) -> f64 + Sync),
    forward_df: &(dyn Fn(f64) -> f64 + Sync),
    schedule: &CapFloorSchedule,
    strike: f64,
    normal_vol: f64,
    is_cap: bool,
) -> f64 {
    schedule
        .periods
        .iter()
        .map(|period| {
            normal_caplet_price(
                scheduled_forward_rate(forward_df, period),
                strike,
                normal_vol,
                period.fixing_time,
                period.accrual,
                discount_df(period.payment_time),
                is_cap,
            )
        })
        .sum()
}

pub(super) fn scheduled_cap_floor_bachelier_vega(
    discount_df: &(dyn Fn(f64) -> f64 + Sync),
    forward_df: &(dyn Fn(f64) -> f64 + Sync),
    schedule: &CapFloorSchedule,
    strike: f64,
    normal_vol: f64,
) -> f64 {
    schedule
        .periods
        .iter()
        .map(|period| {
            normal_caplet_vega(
                scheduled_forward_rate(forward_df, period),
                strike,
                normal_vol,
                period.fixing_time,
            ) * period.accrual
                * discount_df(period.payment_time)
        })
        .sum()
}

pub(super) fn scheduled_forward_rate(
    forward_df: &(dyn Fn(f64) -> f64 + Sync),
    period: &CapletSchedule,
) -> f64 {
    let p_start = forward_df(period.start_time);
    let p_end = forward_df(period.end_time);
    if !p_start.is_finite() || !p_end.is_finite() || p_start <= 0.0 || p_end <= 0.0 {
        return f64::NAN;
    }
    (p_start / p_end - 1.0) / period.accrual
}

/// Price contractual term-index coupons with the same scheduled-HW kernel as
/// production cap/floor valuation. Projection/discount basis is deterministic,
/// while payment delays retain their discount factors and payment-measure
/// convexity adjustment without changing the projected accrual-period end.
pub(crate) fn scheduled_cap_floor_price(
    params: &HullWhiteParams,
    discount_df: &(dyn Fn(f64) -> f64 + Sync),
    forward_df: &(dyn Fn(f64) -> f64 + Sync),
    schedule: &CapFloorSchedule,
    strike: f64,
    is_cap: bool,
) -> finstack_quant_core::Result<f64> {
    schedule
        .periods
        .iter()
        .map(|period| {
            finstack_quant_models::rates::hull_white::hw1f_term_caplet_price_from_dfs_with_model(
                params,
                forward_df(period.start_time),
                forward_df(period.end_time),
                discount_df(period.payment_time),
                period.fixing_time,
                period.start_time,
                period.end_time,
                period.payment_time,
                period.accrual,
                strike,
                is_cap,
            )
        })
        .sum()
}

/// Cap/floor shape used by HW1F pricing helpers.
#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) struct CapFloorPriceSpec {
    pub(super) maturity: f64,
    pub(super) strike: f64,
    pub(super) is_cap: bool,
    pub(super) frequency: SwapFrequency,
}

#[cfg(test)]
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
#[cfg(test)]
pub(crate) fn hw1f_cap_floor_price(
    kappa: f64,
    sigma: f64,
    discount_df: &(dyn Fn(f64) -> f64 + Sync),
    forward_df: &(dyn Fn(f64) -> f64 + Sync),
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
#[cfg(test)]
pub(crate) fn hw1f_cap_floor_price_with_model(
    params: &HullWhiteParams,
    discount_df: &(dyn Fn(f64) -> f64 + Sync),
    forward_df: &(dyn Fn(f64) -> f64 + Sync),
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
    discount_df: &(dyn Fn(f64) -> f64 + Sync),
    forward_df: &(dyn Fn(f64) -> f64 + Sync),
    spec: CapFloorPriceSpec,
) -> finstack_quant_core::Result<f64> {
    let target = hw1f_cap_floor_price(kappa, sigma, discount_df, forward_df, spec);
    cap_floor_implied_normal_vol(target, discount_df, forward_df, spec)
}

#[cfg(test)]
pub(super) fn cap_floor_implied_normal_vol(
    target: f64,
    discount_df: &(dyn Fn(f64) -> f64 + Sync),
    forward_df: &(dyn Fn(f64) -> f64 + Sync),
    spec: CapFloorPriceSpec,
) -> finstack_quant_core::Result<f64> {
    scheduled_cap_floor_implied_normal_vol(
        target,
        discount_df,
        forward_df,
        &CapFloorSchedule::synthetic(spec.maturity, spec.frequency),
        spec.strike,
        spec.is_cap,
    )
}

pub(crate) fn scheduled_cap_floor_implied_normal_vol(
    target: f64,
    discount_df: &(dyn Fn(f64) -> f64 + Sync),
    forward_df: &(dyn Fn(f64) -> f64 + Sync),
    schedule: &CapFloorSchedule,
    strike: f64,
    is_cap: bool,
) -> finstack_quant_core::Result<f64> {
    if !target.is_finite() || target < 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "Hull-White cap price must be finite and non-negative for quote inversion".into(),
        ));
    }
    let residual = |vol: f64| {
        scheduled_bachelier_cap_floor_price(discount_df, forward_df, schedule, strike, vol, is_cap)
            - target
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
#[cfg(test)]
pub(super) fn forward_rate_from_df(df: &(dyn Fn(f64) -> f64 + Sync), start: f64, end: f64) -> f64 {
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
