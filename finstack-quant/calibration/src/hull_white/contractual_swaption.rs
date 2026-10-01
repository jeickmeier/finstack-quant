//! European HW payoff integration for complete contractual swap schedules.
//!
//! Under the exercise-date forward measure, the centered HW state is normal.
//! Every fixed payment and every deferred floating coupon is a signed
//! exponential of that same state. Integrating their positive sum preserves
//! floating payment delays without the zero-lag telescoping assumption.

use super::{HullWhiteCalibrationParams, SwaptionSchedule};
use finstack_quant_core::math::integration::gauss_legendre_integrate_adaptive;
use finstack_quant_core::math::special_functions::norm_cdf;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::rates::hull_white::{hw1f_delayed_coupon_adjustment, hw_b};

struct GaussianTerm {
    sign: f64,
    log_abs: f64,
    loading: f64,
}

/// Present value of a payer's positive contractual swap value at expiry.
pub(crate) fn price(
    params: HullWhiteCalibrationParams,
    df: &(dyn Fn(f64) -> f64 + Sync),
    expiry: f64,
    strike: f64,
    schedule: &SwaptionSchedule,
) -> Result<f64> {
    let factor = |time: f64| -> Result<f64> {
        let value = df(time);
        if value.is_finite() && value > 0.0 {
            Ok(value)
        } else {
            Err(Error::Validation(format!(
                "contractual HW swaption requires a positive finite discount factor at {time}"
            )))
        }
    };
    let variance = params.sigma * params.sigma * hw_b(2.0 * params.kappa, 0.0, expiry);
    if !variance.is_finite() || variance < 0.0 {
        return Err(Error::Validation(
            "contractual HW swaption has invalid state variance".into(),
        ));
    }
    let state_std = variance.sqrt();
    let mut terms =
        Vec::with_capacity(2 * schedule.floating_periods.len() + schedule.accruals.len());
    for period in &schedule.floating_periods {
        let start_b = hw_b(params.kappa, expiry, period.start_time);
        let end_b = hw_b(params.kappa, expiry, period.end_time);
        let pay_b = hw_b(params.kappa, expiry, period.payment_time);
        let adjustment = hw1f_delayed_coupon_adjustment(
            params,
            expiry,
            period.start_time,
            period.end_time,
            period.payment_time,
            period.fixing_time,
            schedule.floating_is_compounded,
        );
        // The exercise-date discount factor cancels from these PV-weighted
        // terms. Only the normal expectation remains to be integrated.
        terms.push(GaussianTerm {
            sign: 1.0,
            log_abs: factor(period.start_time)?.ln() + factor(period.payment_time)?.ln()
                - factor(period.end_time)?.ln()
                - 0.5 * variance * (start_b * start_b + pay_b * pay_b - end_b * end_b)
                + adjustment.ln(),
            loading: -(start_b + pay_b - end_b) * state_std,
        });
        terms.push(GaussianTerm {
            sign: -1.0,
            log_abs: factor(period.payment_time)?.ln() - 0.5 * variance * pay_b * pay_b,
            loading: -pay_b * state_std,
        });
    }
    if strike != 0.0 {
        for (&payment, &accrual) in schedule.payment_times.iter().zip(&schedule.accruals) {
            let pay_b = hw_b(params.kappa, expiry, payment);
            terms.push(GaussianTerm {
                sign: -strike.signum(),
                log_abs: (strike.abs() * accrual * factor(payment)?).ln()
                    - 0.5 * variance * pay_b * pay_b,
                loading: -pay_b * state_std,
            });
        }
    }
    if terms
        .iter()
        .any(|term| !term.log_abs.is_finite() || !term.loading.is_finite())
    {
        return Err(Error::Validation(
            "contractual HW swaption produced non-finite Gaussian cashflows".into(),
        ));
    }
    if state_std == 0.0 {
        return Ok(terms
            .iter()
            .map(|term| term.sign * term.log_abs.exp())
            .sum::<f64>()
            .max(0.0));
    }

    const TOLERANCE: f64 = 1e-12;
    // Exponential tilting shifts each weighted normal term by its loading.
    // Enclose every such center and verify the remaining absolute tail bound,
    // rather than truncating the unweighted state at a fixed number of sigmas.
    let bound = terms
        .iter()
        .map(|term| term.loading.abs())
        .fold(0.0_f64, f64::max)
        + 12.0;
    let tail_bound: f64 = terms
        .iter()
        .map(|term| {
            (term.log_abs + 0.5 * term.loading * term.loading).exp()
                * (norm_cdf(-bound - term.loading) + norm_cdf(term.loading - bound))
        })
        .sum();
    if !tail_bound.is_finite() || tail_bound > TOLERANCE * 0.1 || bound > 1024.0 {
        return Err(Error::Validation(
            "contractual HW swaption cannot bound its Gaussian integration error".into(),
        ));
    }
    let integrand = |state: f64| {
        let weighted_swap: f64 = terms
            .iter()
            .map(|term| {
                term.sign * (term.log_abs + term.loading * state - 0.5 * state * state).exp()
            })
            .sum();
        if !weighted_swap.is_finite() {
            f64::NAN
        } else {
            weighted_swap.max(0.0) / (2.0 * std::f64::consts::PI).sqrt()
        }
    };
    // Unit-width panels prevent a narrow exercise region from being missed by
    // a coarse whole-domain sample. Adaptive refinement resolves payoff kinks.
    let panels = (2.0 * bound).ceil() as usize;
    let width = 2.0 * bound / panels as f64;
    let mut value = 0.0;
    for panel in 0..panels {
        let left = -bound + panel as f64 * width;
        value += gauss_legendre_integrate_adaptive(
            integrand,
            left,
            left + width,
            16,
            TOLERANCE * 0.9 / panels as f64,
            20,
        )?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hull_white::SwaptionFloatingPeriod;

    #[test]
    fn one_coupon_swaption_matches_independent_log_ratio_option() {
        let params = HullWhiteCalibrationParams {
            kappa: 0.07,
            sigma: 0.025,
        };
        let df = |time: f64| (-0.03 * time).exp();
        let (expiry, fixing, start, end, payment) = (1.0, 1.98, 2.0, 3.0, 3.2);
        let accrual = 365.0 / 360.0;
        let strike = 0.03;
        for compounded in [false, true] {
            let schedule = SwaptionSchedule {
                swap_start_time: start,
                maturity_time: end,
                payment_times: vec![payment],
                accruals: vec![accrual],
                floating_periods: vec![SwaptionFloatingPeriod {
                    fixing_time: fixing,
                    start_time: start,
                    end_time: end,
                    payment_time: payment,
                    accrual,
                }],
                floating_is_compounded: compounded,
            };
            // The whole-life payment covariance is computed from Brownian
            // loadings, independently of the conditional coupon adjustment.
            let last = if compounded { end } else { fixing };
            let n = 30_000;
            let du = last / f64::from(n);
            let covariance: f64 = (0..n)
                .map(|i| {
                    let u = (f64::from(i) + 0.5) * du;
                    let observed_start = if compounded { start.max(u) } else { start };
                    let coupon_loading = params.sigma
                        * ((-params.kappa * (observed_start - u)).exp()
                            - (-params.kappa * (end - u)).exp())
                        / params.kappa;
                    let payment_loading = params.sigma
                        * ((-params.kappa * (end - u)).exp()
                            - (-params.kappa * (payment - u)).exp())
                        / params.kappa;
                    coupon_loading * payment_loading * du
                })
                .sum();
            let positive_mean = df(start) * df(payment) / df(end) * (-covariance).exp();
            let negative_mean = (1.0 + strike * accrual) * df(payment);
            let state_std = params.sigma * hw_b(2.0 * params.kappa, 0.0, expiry).sqrt();
            let ratio_vol =
                (hw_b(params.kappa, expiry, end) - hw_b(params.kappa, expiry, start)) * state_std;
            let d1 = (positive_mean / negative_mean).ln() / ratio_vol + 0.5 * ratio_vol;
            let expected = positive_mean * norm_cdf(d1) - negative_mean * norm_cdf(d1 - ratio_vol);
            let actual = price(params, &df, expiry, strike, &schedule).expect("contractual price");
            assert!(
                (actual - expected).abs() < 2e-12,
                "compounded={compounded}, {actual} vs {expected}"
            );
        }
    }

    #[test]
    fn zero_variance_retains_delayed_contractual_intrinsic() {
        let params = HullWhiteCalibrationParams {
            kappa: 0.05,
            sigma: 0.0,
        };
        let df = |time: f64| (-0.03 * time).exp();
        let schedule = SwaptionSchedule {
            swap_start_time: 1.02,
            maturity_time: 2.02,
            payment_times: vec![2.04],
            accruals: vec![1.0],
            floating_periods: vec![SwaptionFloatingPeriod {
                fixing_time: 1.0,
                start_time: 1.02,
                end_time: 2.02,
                payment_time: 2.06,
                accrual: 365.0 / 360.0,
            }],
            floating_is_compounded: false,
        };
        for strike in [0.02, 0.04] {
            let expected = ((df(1.02) / df(2.02) - 1.0) * df(2.06) - strike * df(2.04)).max(0.0);
            let actual = price(params, &df, 1.0, strike, &schedule).expect("intrinsic value");
            assert!((actual - expected).abs() < 1e-15);
        }
    }
}
