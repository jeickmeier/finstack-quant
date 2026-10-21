use super::*;

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

/// Simple forward rate over a caplet period from a discount-factor function.
///
/// Non-finite or non-positive discount factors propagate as `NaN` instead of
/// being clamped: the cap/floor residuals and the fixed-kappa sigma solve rely
/// on the non-finite-price check to detect broken curves.
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
        .with_tolerance(1e-14)
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

pub(super) fn normal_caplet_price(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    accrual: f64,
    df: f64,
    is_cap: bool,
) -> f64 {
    use finstack_quant_models::closed_form::{bachelier_call, bachelier_put};
    let unit_price = if is_cap {
        bachelier_call(forward, strike, vol, expiry)
    } else {
        bachelier_put(forward, strike, vol, expiry)
    };
    unit_price * (accrual * df)
}

fn normal_caplet_vega(forward: f64, strike: f64, vol: f64, expiry: f64) -> f64 {
    finstack_quant_models::closed_form::bachelier_vega(forward, strike, vol, expiry)
}
