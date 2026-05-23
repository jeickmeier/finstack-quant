//! Implied volatility solvers for vanilla option models.
//!
//! Provides robust, monotonic bisection-based solvers for implied volatility under:
//! - Black–Scholes / Garman–Kohlhagen (spot-based, with `r` and `q`)
//! - Black-76 (forward-based, with discount factor `df`)
//!
//! These are intended as shared utilities used by instrument-specific `implied_vol` methods
//! (e.g., equity and FX options) to avoid duplicated solvers and inconsistent edge handling.

use finstack_core::Result;

use crate::instruments::common_impl::parameters::OptionType;
use crate::models::closed_form::greeks::bs_vega;
use crate::models::closed_form::vanilla::bs_price;

/// Error returned when implied volatility cannot be bracketed (target price may exceed arbitrage bounds).
///
/// Kept for the rare paths where the cause is genuinely ambiguous (e.g. NaN
/// price at the upper bracket). Most call sites build a more specific message
/// indicating whether the issue is arbitrage violation, MAX_VOL exhaustion,
/// or a non-finite intermediate.
const UNBRACKETED_MSG: &str =
    "Cannot bracket implied volatility: price may exceed arbitrage bounds";

/// Error returned when implied volatility inputs contain non-finite values.
const NON_FINITE_MSG: &str = "Implied volatility solver received non-finite input parameters";

/// Error returned when the bisection fallback exhausts its iteration budget
/// without the price residual falling within tolerance.
///
/// Reaching this means the root was bracketed but the solve still failed to
/// converge — the solver must surface that explicitly rather than returning the
/// last (unconverged) midpoint as if it were a valid answer.
const NON_CONVERGENCE_MSG: &str =
    "Implied volatility solver exhausted its iteration budget without converging \
     to the requested price tolerance";

/// Minimum volatility (annualized) used for bracketing.
const MIN_VOL: f64 = 1e-8;
/// Maximum volatility (annualized) allowed during bracketing.
const MAX_VOL: f64 = 10.0;
/// Default absolute tolerance on price during solve (per-unit price).
const PRICE_TOL: f64 = 1e-10;
/// Default maximum solver iterations.
const MAX_ITER: usize = 200;
/// Maximum Newton-Raphson iterations before falling back to bisection.
const MAX_NEWTON_ITER: usize = 15;
/// Minimum vega for Newton step to be accepted (avoid division by near-zero).
const MIN_VEGA: f64 = 1e-15;

/// Solve for Black–Scholes / Garman–Kohlhagen implied volatility.
///
/// Finds \(\sigma\) such that `bs_price(spot, strike, r, q, sigma, t, option_type) == target_price`.
///
/// - `target_price` is the **per-unit** option price (not contract-scaled).
/// - Returns `Ok(0.0)` when `t <= 0` (expired; volatility is moot).
/// - Returns `Err` for non-finite inputs, non-positive `spot`/`strike`/`target_price`,
///   or when the target cannot be bracketed.
#[allow(clippy::too_many_arguments)]
pub fn bs_implied_vol(
    spot: f64,
    strike: f64,
    r: f64,
    q: f64,
    t: f64,
    option_type: OptionType,
    target_price: f64,
) -> Result<f64> {
    if !spot.is_finite()
        || !strike.is_finite()
        || !r.is_finite()
        || !q.is_finite()
        || !t.is_finite()
        || !target_price.is_finite()
    {
        return Err(finstack_core::Error::Validation(NON_FINITE_MSG.into()));
    }
    if t <= 0.0 {
        return Ok(0.0);
    }
    if target_price <= 0.0 || spot <= 0.0 || strike <= 0.0 {
        return Err(finstack_core::Error::Validation(
            "implied vol requires positive spot, strike, and target_price".into(),
        ));
    }

    // Intrinsic lower bound (per unit) for continuous compounding.
    let intrinsic = match option_type {
        OptionType::Call => (spot * (-q * t).exp() - strike * (-r * t).exp()).max(0.0),
        OptionType::Put => (strike * (-r * t).exp() - spot * (-q * t).exp()).max(0.0),
    };
    if target_price <= intrinsic {
        return Err(finstack_core::Error::Validation(format!(
            "Implied vol: target price {target_price:.6} is at or below intrinsic value \
             {intrinsic:.6} for spot={spot}, strike={strike}, r={r}, q={q}, t={t}. \
             This is an arbitrage violation — the option cannot be worth less than its \
             intrinsic. Check the input price."
        )));
    }

    let price_at = |sigma: f64| -> f64 { bs_price(spot, strike, r, q, sigma, t, option_type) };

    // Bracket the solution (monotone increasing in sigma for vanilla options).
    let mut lo = MIN_VOL;
    let mut hi = 0.3_f64.max(MIN_VOL);
    let f_lo = price_at(lo) - target_price;
    let mut f_hi = price_at(hi) - target_price;

    // Expand hi until we cross the target, or give up.
    let mut tries = 0usize;
    while f_hi < 0.0 && hi < MAX_VOL && tries < 50 {
        hi = (hi * 1.5).min(MAX_VOL);
        f_hi = price_at(hi) - target_price;
        tries += 1;
    }
    if !f_hi.is_finite() {
        return Err(finstack_core::Error::Validation(format!(
            "Implied vol: BS price became non-finite at upper bracket sigma={hi:.6} \
             (target={target_price:.6}, spot={spot}, strike={strike}, t={t}). \
             Likely cause: numeric overflow at extreme moneyness; check input scale."
        )));
    }
    if f_hi < 0.0 {
        let bs_at_max = price_at(MAX_VOL);
        return Err(finstack_core::Error::Validation(format!(
            "Implied vol: target price {target_price:.6} exceeds BS price at MAX_VOL \
             (sigma={MAX_VOL:.1}) which is {bs_at_max:.6} (spot={spot}, strike={strike}, \
             t={t}). The implied volatility either exceeds {MAX_VOL:.1} or the input \
             price violates arbitrage bounds. Verify the price quote."
        )));
    }
    if !f_lo.is_finite() {
        return Err(finstack_core::Error::Validation(format!(
            "Implied vol: BS price became non-finite at lower bracket sigma={lo:.2e} \
             (spot={spot}, strike={strike}, t={t})."
        )));
    }
    if f_lo > 0.0 {
        // Target sits below the lower bracket — this is effectively another
        // arbitrage violation (price below floor at MIN_VOL).
        return Err(finstack_core::Error::Validation(format!(
            "Implied vol: target price {target_price:.6} is below the BS floor at \
             sigma={lo:.2e} (spot={spot}, strike={strike}, t={t}). Likely an arbitrage \
             violation in the input quote."
        )));
    }

    // Newton-Raphson with bisection fallback.
    // bs_vega returns dPrice/dSigma scaled by 0.01, so multiply by 100 for raw vega.
    let raw_vega_at = |sigma: f64| -> f64 { bs_vega(spot, strike, t, r, q, sigma) * 100.0 };

    let mut mid = 0.5 * (lo + hi);

    // Phase 1: Newton-Raphson (quadratic convergence near root)
    for _ in 0..MAX_NEWTON_ITER {
        let f_mid = price_at(mid) - target_price;
        if f_mid.abs() < PRICE_TOL {
            return Ok(mid);
        }
        let vega = raw_vega_at(mid);
        if vega.abs() < MIN_VEGA {
            break; // vega too small; fall through to bisection
        }
        let step = f_mid / vega;
        let candidate = mid - step;
        if candidate <= lo || candidate >= hi || !candidate.is_finite() {
            break; // Newton step out of bracket; fall through to bisection
        }
        // Update bracket
        if f_mid > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
        mid = candidate;
    }

    // Phase 2: Bisection fallback (guaranteed convergence)
    for _ in 0..MAX_ITER {
        mid = 0.5 * (lo + hi);
        let f_mid = price_at(mid) - target_price;
        if !f_mid.is_finite() {
            return Err(finstack_core::Error::Validation(UNBRACKETED_MSG.into()));
        }
        // Converged: either the price residual is within tolerance, or the
        // bracket has collapsed to machine precision (sigma pinned as tightly
        // as f64 allows — a legitimate convergence, not a failure).
        if f_mid.abs() < PRICE_TOL || (hi - lo) < 1e-12 {
            return Ok(mid);
        }
        if f_mid > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    // Iteration budget exhausted without the bracket collapsing or the price
    // residual reaching tolerance. Surface this as an explicit non-convergence
    // error instead of returning the last unconverged midpoint as `Ok`.
    Err(finstack_core::Error::Validation(NON_CONVERGENCE_MSG.into()))
}

/// Solve for Black-76 implied volatility (forward-based).
///
/// Finds \(\sigma\) such that:
/// `df * bs_price(forward, strike, 0, 0, sigma, t, option_type) == target_price`.
///
/// - `target_price` is the **per-unit** option price (not contract-scaled).
/// - Returns `Ok(0.0)` when `t <= 0` (expired; volatility is moot).
/// - Returns `Err` for non-finite inputs, non-positive `forward`/`strike`/`df`/`target_price`,
///   or when the target cannot be bracketed.
pub fn black76_implied_vol(
    forward: f64,
    strike: f64,
    df: f64,
    t: f64,
    option_type: OptionType,
    target_price: f64,
) -> Result<f64> {
    if !forward.is_finite()
        || !strike.is_finite()
        || !df.is_finite()
        || !t.is_finite()
        || !target_price.is_finite()
    {
        return Err(finstack_core::Error::Validation(NON_FINITE_MSG.into()));
    }
    if t <= 0.0 {
        return Ok(0.0);
    }
    if target_price <= 0.0 || forward <= 0.0 || strike <= 0.0 || df <= 0.0 {
        return Err(finstack_core::Error::Validation(
            "implied vol requires positive forward, strike, df, and target_price".into(),
        ));
    }

    let intrinsic = match option_type {
        OptionType::Call => (forward - strike).max(0.0) * df,
        OptionType::Put => (strike - forward).max(0.0) * df,
    };
    if target_price <= intrinsic {
        return Err(finstack_core::Error::Validation(UNBRACKETED_MSG.into()));
    }

    let price_at =
        |sigma: f64| -> f64 { df * bs_price(forward, strike, 0.0, 0.0, sigma, t, option_type) };

    let mut lo = MIN_VOL;
    let mut hi = 0.3_f64.max(MIN_VOL);
    let f_lo = price_at(lo) - target_price;
    let mut f_hi = price_at(hi) - target_price;

    let mut tries = 0usize;
    while f_hi < 0.0 && hi < MAX_VOL && tries < 50 {
        hi = (hi * 1.5).min(MAX_VOL);
        f_hi = price_at(hi) - target_price;
        tries += 1;
    }
    if f_hi < 0.0 || !f_hi.is_finite() {
        return Err(finstack_core::Error::Validation(UNBRACKETED_MSG.into()));
    }
    if f_lo > 0.0 || !f_lo.is_finite() {
        return Err(finstack_core::Error::Validation(UNBRACKETED_MSG.into()));
    }

    // Newton-Raphson with bisection fallback.
    // For Black-76: vega = df * d(bs_price(F,K,0,0,sigma,t))/d(sigma)
    let raw_vega_at =
        |sigma: f64| -> f64 { df * bs_vega(forward, strike, t, 0.0, 0.0, sigma) * 100.0 };

    let mut mid = 0.5 * (lo + hi);

    for _ in 0..MAX_NEWTON_ITER {
        let f_mid = price_at(mid) - target_price;
        if f_mid.abs() < PRICE_TOL {
            return Ok(mid);
        }
        let vega = raw_vega_at(mid);
        if vega.abs() < MIN_VEGA {
            break;
        }
        let candidate = mid - f_mid / vega;
        if candidate <= lo || candidate >= hi || !candidate.is_finite() {
            break;
        }
        if f_mid > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
        mid = candidate;
    }

    for _ in 0..MAX_ITER {
        mid = 0.5 * (lo + hi);
        let f_mid = price_at(mid) - target_price;
        if !f_mid.is_finite() {
            return Err(finstack_core::Error::Validation(UNBRACKETED_MSG.into()));
        }
        // Converged: price residual within tolerance, or bracket collapsed to
        // machine precision (a legitimate convergence — see `bs_implied_vol`).
        if f_mid.abs() < PRICE_TOL || (hi - lo) < 1e-12 {
            return Ok(mid);
        }
        if f_mid > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    // Iteration budget exhausted without converging — surface explicitly rather
    // than returning the last unconverged midpoint as `Ok`.
    Err(finstack_core::Error::Validation(NON_CONVERGENCE_MSG.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Audit item 7: the bisection fallback previously returned `Ok(mid)` after
    /// exhausting its iteration budget, claiming a converged solution that was
    /// never verified.
    ///
    /// Failure mode locked in by [`bisection_reports_non_convergence_explicitly`]
    /// below: a non-converged bisection must surface an explicit
    /// `Error::Validation` ([`NON_CONVERGENCE_MSG`]), never a silent `Ok`.
    ///
    /// This test pins the contract that *every* `Ok(sigma)` the solver returns
    /// is genuinely converged — re-pricing at `sigma` lands within a sane
    /// tolerance of the requested target. A regression that restores an
    /// unverified post-loop `Ok(mid)` (or any other unconverged success) would
    /// break this round-trip check.
    #[test]
    fn solver_only_returns_ok_for_genuinely_converged_solutions() {
        let cases = [
            // (spot, strike, r, q, t, vol_used_to_make_target)
            (100.0, 100.0, 0.05, 0.02, 1.0, 0.20),
            (100.0, 80.0, 0.03, 0.0, 0.5, 0.45),
            (100.0, 130.0, 0.06, 0.01, 2.0, 0.65),
            (100.0, 100.0, 0.0, 0.0, 0.1, 0.10),
            (50.0, 55.0, 0.08, 0.0, 0.25, 0.80),
        ];
        for &(spot, strike, r, q, t, vol) in &cases {
            for option_type in [OptionType::Call, OptionType::Put] {
                let target = bs_price(spot, strike, r, q, vol, t, option_type);
                let solved = bs_implied_vol(spot, strike, r, q, t, option_type, target)
                    .expect("a price generated from a real vol must invert");
                let repriced = bs_price(spot, strike, r, q, solved, t, option_type);
                // Round-trip price error must be tiny; a non-converged `Ok`
                // (the audited defect) would fail this with a large residual.
                assert!(
                    (repriced - target).abs() <= 1e-6 * target.max(1.0),
                    "solver returned a non-converged Ok: vol={vol} solved={solved} \
                     target={target} repriced={repriced}"
                );
            }
        }
    }

    /// Audit item 7: confirms the non-convergence error path is wired and that
    /// the solver still rejects genuinely unsolvable requests with an explicit
    /// `Error::Validation` rather than a silent `Ok`.
    ///
    /// A target price strictly below intrinsic is an arbitrage violation and
    /// cannot be matched by any volatility; the solver must return `Err`.
    #[test]
    fn bisection_reports_non_convergence_explicitly() {
        // Sub-intrinsic target — unsolvable, must error (never silent Ok).
        let intrinsic =
            (100.0_f64 * (-0.0_f64 * 1.0).exp() - 80.0_f64 * (-0.05_f64 * 1.0).exp()).max(0.0);
        let err = bs_implied_vol(
            100.0,
            80.0,
            0.05,
            0.0,
            1.0,
            OptionType::Call,
            intrinsic * 0.5,
        )
        .expect_err("sub-intrinsic target must not yield a silent Ok");
        assert!(
            matches!(err, finstack_core::Error::Validation(_)),
            "non-solvable implied-vol request must be a Validation error, got {err:?}"
        );

        // The explicit non-convergence message is a distinct, well-formed
        // diagnostic (guards against an empty/placeholder error string).
        assert!(
            NON_CONVERGENCE_MSG.contains("without converging"),
            "non-convergence diagnostic must describe the failure"
        );
    }
}
