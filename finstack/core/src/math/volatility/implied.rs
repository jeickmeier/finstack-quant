//! Implied volatility solvers for Black-76 and Bachelier (normal) models.
//!
//! Implements implied volatility extraction for Black-76 and Bachelier prices:
//!
//! 1. **Normalize** the problem using put-call parity to work with the
//!    out-of-the-money option (better numerical conditioning — avoids
//!    catastrophic cancellation when computing time value for deep ITM)
//! 2. **Bracket & bisect** to obtain a reliable initial guess across all
//!    moneyness regimes (deep OTM through deep ITM)
//! 3. **Refine** with Householder's third-order method (Halley's method)
//!    using analytical vega and volga for cubic convergence
//!
//! # Accuracy
//!
//! Both solvers achieve machine-precision accuracy (< 10⁻¹² relative error
//! on volatility) within 2–4 Householder iterations from the initial guess.
//!
//! The method is a robust bracketed bisection followed by Halley polishing —
//! simpler (and slower in the worst case) than rational-approximation
//! initial-guess schemes such as Jäckel's "Let's Be Rational", which is not
//! implemented here.
//!
//! # References
//!
//! - Brenner, M., & Subrahmanyam, M. G. (1988). "A Simple Formula to Compute
//!   the Implied Standard Deviation." *Financial Analysts Journal*, 44(5), 80–83.
//! - Manaster, S., & Koehler, G. (1982). "The Calculation of Implied Variances
//!   from the Black-Scholes Model." *Journal of Finance*, 37(1), 227–230.
//! - Li, B. (2006). "A New Formula for Computing Implied Normal Volatility."
//!   Working paper.

use crate::error::InputError;

use super::pricing::{
    bachelier_call, bachelier_put, bachelier_vega, black_call, black_put, black_vega,
};

// ─── Constants ───────────────────────────────────────────────────────────────

/// Maximum Householder/Newton iterations after the bisection phase.
/// The bisection delivers a starting point within ~0.01% of the root,
/// so 4 cubic iterations are more than enough for machine precision.
const MAX_ITER: usize = 4;

/// Convergence tolerance on the Newton/Householder step |Δσ|.
const STEP_TOL: f64 = 1e-14;

/// Floor for implied volatility; values below this are treated as zero.
const VOL_FLOOR: f64 = 1e-16;

/// Ceiling for Black-76 implied vol (1000%).
const VOL_CEIL_BLACK: f64 = 10.0;

/// Ceiling multiplier for Bachelier implied vol.
///
/// Normal vol is in *price units*, so a fixed absolute ceiling would reject
/// price-quoted underlyings (e.g. equities near 100). The effective ceiling is
/// `max(|F|, |K|, 1) × VOL_CEIL_BACH`, i.e. 1000% of the price scale.
const VOL_CEIL_BACH: f64 = 10.0;

/// Number of bisection steps used to narrow the bracket before Householder.
/// 20 bisection steps refine the bracket by a factor of 2²⁰ ≈ 10⁶.
const BISECTION_STEPS: usize = 20;

// ─── Public API ──────────────────────────────────────────────────────────────

/// Extract Black-76 (lognormal) implied volatility from an option price.
///
/// Given a market option price under the Black-76 model (per unit annuity),
/// finds the unique lognormal volatility σ that reproduces the price:
///
/// ```text
/// black_call(F, K, σ, T) = price   (for calls)
/// black_put(F, K, σ, T)  = price   (for puts)
/// ```
///
/// # Algorithm
///
/// 1. **Put-call parity**: Convert to the out-of-the-money option for
///    better numerical conditioning (avoids catastrophic cancellation in
///    the time-value computation for deep ITM options).
/// 2. **Arbitrage bounds**: Verify `intrinsic ≤ price ≤ upper_bound`.
/// 3. **Bracket & bisect**: Find a tight bracket around the root using
///    monotonicity of the Black price in vol, then narrow with bisection.
/// 4. **Householder refinement**: Third-order (Halley) iterations using
///    analytical Black vega and volga for cubic convergence — each step
///    approximately triples the number of correct digits.
///
/// # Arguments
///
/// * `price` - Market option price per unit annuity (non-negative)
/// * `forward` - Forward rate or price (must be positive and finite)
/// * `strike` - Strike rate or price (must be positive and finite)
/// * `t` - Time to expiry in years (must be positive and finite)
/// * `is_call` - `true` for a call option, `false` for a put option
///
/// # Returns
///
/// The implied lognormal volatility σ ≥ 0.
///
/// # Errors
///
/// | Condition | Error |
/// |-----------|-------|
/// | `forward ≤ 0` or non-finite | [`InputError::NonPositiveValue`] |
/// | `strike ≤ 0` or non-finite | [`InputError::NonPositiveValue`] |
/// | `t ≤ 0` or non-finite | [`InputError::InvalidTimeToExpiry`] |
/// | `price < 0` or non-finite | [`InputError::NegativeValue`] |
/// | `price < intrinsic` (arbitrage) | [`InputError::InvalidVolatility`] |
/// | `price ≥ F` (call) or `≥ K` (put) | [`InputError::InvalidVolatility`] |
/// | Solver non-convergence | [`InputError::VolatilityConversionFailed`] |
///
/// # Example
///
/// ```rust
/// use finstack_core::math::volatility::{implied_vol_black, black_call};
/// # fn main() -> finstack_core::Result<()> {
///
/// let forward = 0.05;
/// let strike = 0.05;  // ATM
/// let sigma = 0.25;   // 25% lognormal vol
/// let t = 1.0;
///
/// // Round-trip: price → implied vol → verify
/// let price = black_call(forward, strike, sigma, t);
/// let implied = implied_vol_black(price, forward, strike, t, true)?;
/// assert!((implied - sigma).abs() < 1e-12);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Brenner, M., & Subrahmanyam, M. G. (1988). "A Simple Formula to Compute
///   the Implied Standard Deviation." *Financial Analysts Journal*, 44(5), 80–83.
/// - Manaster, S., & Koehler, G. (1982). "The Calculation of Implied Variances
///   from the Black-Scholes Model." *Journal of Finance*, 37(1), 227–230.
pub fn implied_vol_black(
    price: f64,
    forward: f64,
    strike: f64,
    t: f64,
    is_call: bool,
) -> crate::Result<f64> {
    // ── 1. Input validation ──────────────────────────────────────────────
    validate_positive(forward)?;
    validate_positive(strike)?;
    validate_time(t)?;
    validate_price(price)?;

    // ── 2. Intrinsic value and arbitrage bounds ──────────────────────────
    let scale = forward.max(strike);
    let intrinsic = if is_call {
        (forward - strike).max(0.0)
    } else {
        (strike - forward).max(0.0)
    };

    // Price below intrinsic → negative time value → arbitrage violation
    if price < intrinsic - f64::EPSILON * scale {
        return Err(InputError::InvalidVolatility { value: -1.0 }.into());
    }
    // Price at intrinsic → vol is zero
    if price <= intrinsic + f64::EPSILON * scale {
        return Ok(0.0);
    }

    // Upper bound: Call ≤ F, Put ≤ K (Black-76 property as σ → ∞)
    let upper = if is_call { forward } else { strike };
    if price >= upper - f64::EPSILON * scale {
        return Err(InputError::InvalidVolatility {
            value: f64::INFINITY,
        }
        .into());
    }

    // ── 3. Convert to OTM option via put-call parity ─────────────────────
    let (otm_price, otm_is_call) = to_otm(price, forward, strike, is_call);
    if otm_price <= f64::EPSILON * scale {
        return Ok(0.0);
    }

    // ── 4. Bracket & bisect for initial guess ────────────────────────────
    let price_fn = |sigma: f64| -> f64 {
        if otm_is_call {
            black_call(forward, strike, sigma, t)
        } else {
            black_put(forward, strike, sigma, t)
        }
    };
    let analytical = analytical_guess_black(otm_price, forward, strike, t);
    let mut sigma = bracket_and_bisect(otm_price, &price_fn, analytical, VOL_CEIL_BLACK);

    // ── 5. Householder (Halley) refinement ───────────────────────────────
    //
    // Third-order convergence using:
    //   f(σ)   = black_price(σ) − target
    //   f′(σ)  = vega  = F √T φ(d₁)
    //   f″(σ) = volga = vega · d₁ · d₂ / σ
    //
    // Halley update: σ ← σ − 2f·f′ / (2f′² − f·f″)
    let sqrt_t = t.sqrt();
    let ln_fk = (forward / strike).ln();

    for _ in 0..MAX_ITER {
        let diff = price_fn(sigma) - otm_price;

        let vega = black_vega(forward, strike, sigma, t);
        if vega < f64::MIN_POSITIVE {
            break;
        }

        let newton_step = diff / vega;
        if newton_step.abs() < STEP_TOL {
            break;
        }

        // Compute d₁, d₂ for volga
        let st = sigma * sqrt_t;
        // d1/d2 intentionally inline: ln_fk pre-hoisted for Newton loop performance
        let d1 = (ln_fk + 0.5 * st * st) / st;
        let d2 = d1 - st;
        let volga = vega * d1 * d2 / sigma;

        // Halley step with Newton fallback for ill-conditioned denominator
        let denom = 2.0 * vega * vega - diff * volga;
        sigma = if denom.abs() > f64::EPSILON * vega * vega {
            sigma - 2.0 * diff * vega / denom
        } else {
            sigma - newton_step
        };
        sigma = sigma.clamp(VOL_FLOOR, VOL_CEIL_BLACK);
    }

    // ── 6. Final convergence verification ────────────────────────────────
    verify_convergence_black(sigma, otm_price, forward, strike, t, otm_is_call)
}

/// Extract Bachelier (normal) implied volatility from an option price.
///
/// Given a market option price under the Bachelier model (per unit annuity),
/// finds the unique normal volatility σ_n that reproduces the price:
///
/// ```text
/// bachelier_call(F, K, σ_n, T) = price   (for calls)
/// bachelier_put(F, K, σ_n, T)  = price   (for puts)
/// ```
///
/// # Algorithm
///
/// 1. **Put-call parity**: Convert to OTM option.
/// 2. **Bracket & bisect**: Reliable starting point via monotone bisection.
/// 3. **Householder refinement**: Third-order iterations using analytical
///    Bachelier vega (√T × φ(d)) and volga (vega × d²/σ).
///
/// # Arguments
///
/// * `price` - Market option price per unit annuity (non-negative)
/// * `forward` - Forward rate (any finite value; negative rates supported)
/// * `strike` - Strike rate (any finite value)
/// * `t` - Time to expiry in years (must be positive and finite)
/// * `is_call` - `true` for a call option, `false` for a put option
///
/// # Returns
///
/// The implied normal volatility σ_n ≥ 0.
///
/// # Errors
///
/// | Condition | Error |
/// |-----------|-------|
/// | Forward or strike non-finite | [`InputError::Invalid`] |
/// | `t ≤ 0` or non-finite | [`InputError::InvalidTimeToExpiry`] |
/// | `price < 0` or non-finite | [`InputError::NegativeValue`] |
/// | `price < intrinsic` (arbitrage) | [`InputError::InvalidVolatility`] |
/// | Solver non-convergence | [`InputError::VolatilityConversionFailed`] |
///
/// # Example
///
/// ```rust
/// use finstack_core::math::volatility::{implied_vol_bachelier, bachelier_call};
/// # fn main() -> finstack_core::Result<()> {
///
/// let forward = 0.03;     // 3% forward rate
/// let strike = 0.025;     // 2.5% strike
/// let sigma_n = 0.005;    // 50bp normal vol
/// let t = 1.0;
///
/// let price = bachelier_call(forward, strike, sigma_n, t);
/// let implied = implied_vol_bachelier(price, forward, strike, t, true)?;
/// assert!((implied - sigma_n).abs() < 1e-12);
/// # Ok(())
/// # }
/// ```
///
/// # References
///
/// - Li, B. (2006). "A New Formula for Computing Implied Normal Volatility."
/// - Bachelier, L. (1900). "Théorie de la spéculation."
pub fn implied_vol_bachelier(
    price: f64,
    forward: f64,
    strike: f64,
    t: f64,
    is_call: bool,
) -> crate::Result<f64> {
    // ── 1. Input validation ──────────────────────────────────────────────
    if !forward.is_finite() || !strike.is_finite() {
        return Err(InputError::Invalid.into());
    }
    validate_time(t)?;
    validate_price(price)?;

    // ── 2. Intrinsic value and arbitrage bounds ──────────────────────────
    let scale = forward.abs().max(strike.abs()).max(1e-10);
    let intrinsic = if is_call {
        (forward - strike).max(0.0)
    } else {
        (strike - forward).max(0.0)
    };

    if price < intrinsic - f64::EPSILON * scale {
        return Err(InputError::InvalidVolatility { value: -1.0 }.into());
    }
    if price <= intrinsic + f64::EPSILON * scale {
        return Ok(0.0);
    }

    // ── 3. Convert to OTM option via put-call parity ─────────────────────
    let (otm_price, otm_is_call) = to_otm(price, forward, strike, is_call);
    if otm_price <= f64::EPSILON * scale {
        return Ok(0.0);
    }

    // ── 4. Bracket & bisect for initial guess ────────────────────────────
    let price_fn = |sigma: f64| -> f64 {
        if otm_is_call {
            bachelier_call(forward, strike, sigma, t)
        } else {
            bachelier_put(forward, strike, sigma, t)
        }
    };
    let two_pi = 2.0 * std::f64::consts::PI;
    let analytical = (otm_price * (two_pi / t).sqrt()).max(VOL_FLOOR);
    // Normal vol scales with the price level: bound it relative to the
    // forward/strike magnitude so price-quoted underlyings invert.
    let vol_ceil = forward.abs().max(strike.abs()).max(1.0) * VOL_CEIL_BACH;
    let mut sigma = bracket_and_bisect(otm_price, &price_fn, analytical, vol_ceil);

    // ── 5. Householder (Halley) refinement ───────────────────────────────
    //
    // Bachelier derivatives:
    //   f′(σ)  = vega  = √T × φ(d)
    //   f″(σ) = volga = vega × d² / σ
    // where d = (F − K) / (σ √T)
    let sqrt_t = t.sqrt();

    for _ in 0..MAX_ITER {
        let diff = price_fn(sigma) - otm_price;

        let vega = bachelier_vega(forward, strike, sigma, t);
        if vega < f64::MIN_POSITIVE {
            break;
        }

        let newton_step = diff / vega;
        if newton_step.abs() < STEP_TOL {
            break;
        }

        // Volga for Householder correction
        let st = sigma * sqrt_t;
        let d = if st > VOL_FLOOR {
            (forward - strike) / st
        } else {
            0.0
        };
        let volga = if sigma > VOL_FLOOR {
            vega * d * d / sigma
        } else {
            0.0
        };

        let denom = 2.0 * vega * vega - diff * volga;
        sigma = if denom.abs() > f64::EPSILON * vega * vega {
            sigma - 2.0 * diff * vega / denom
        } else {
            sigma - newton_step
        };
        sigma = sigma.clamp(VOL_FLOOR, vol_ceil);
    }

    // ── 6. Final convergence verification ────────────────────────────────
    verify_convergence_bachelier(sigma, otm_price, forward, strike, t, otm_is_call)
}

// ─── Internal Helpers ────────────────────────────────────────────────────────

/// Validate that a value is positive and finite.
#[inline]
fn validate_positive(value: f64) -> crate::Result<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    Ok(())
}

/// Validate time-to-expiry is positive and finite.
#[inline]
fn validate_time(t: f64) -> crate::Result<()> {
    if !t.is_finite() || t <= 0.0 {
        return Err(InputError::InvalidTimeToExpiry { value: t }.into());
    }
    Ok(())
}

/// Validate price is non-negative and finite.
#[inline]
fn validate_price(price: f64) -> crate::Result<()> {
    if !price.is_finite() || price < 0.0 {
        return Err(InputError::NegativeValue.into());
    }
    Ok(())
}

/// Convert an option price to its out-of-the-money equivalent using
/// put-call parity: C − P = F − K.
///
/// Returns `(otm_price, otm_is_call)`.
#[inline]
fn to_otm(price: f64, forward: f64, strike: f64, is_call: bool) -> (f64, bool) {
    if is_call && forward > strike {
        // ITM call → OTM put: P = C − (F − K)
        (price - (forward - strike), false)
    } else if !is_call && strike > forward {
        // ITM put → OTM call: C = P − (K − F)
        (price - (strike - forward), true)
    } else {
        // Already OTM or ATM — no conversion needed
        (price, is_call)
    }
}

/// Analytical initial guess for Black-76 implied volatility.
///
/// Uses Brenner-Subrahmanyam near ATM and Manaster-Koehler away from money.
fn analytical_guess_black(otm_price: f64, forward: f64, strike: f64, t: f64) -> f64 {
    let two_pi = 2.0 * std::f64::consts::PI;
    let abs_moneyness = (forward / strike).ln().abs();

    // Brenner-Subrahmanyam: σ ≈ √(2π/T) × price / F
    let bs = (two_pi / t).sqrt() * otm_price / forward;

    // Manaster-Koehler: σ ≈ √(2|ln(F/K)|/T)
    let mk = if abs_moneyness > 1e-12 {
        (2.0 * abs_moneyness / t).sqrt()
    } else {
        bs
    };

    // Blend: near ATM → 100% BS; far OTM → 100% MK
    let blend = (abs_moneyness * 5.0).min(1.0);
    let guess = (1.0 - blend) * bs + blend * mk;

    guess.clamp(VOL_FLOOR, VOL_CEIL_BLACK)
}

/// Bracket the root and bisect to obtain a tight initial guess.
///
/// Exploits the monotonicity of option prices in vol: for any model,
/// ∂price/∂σ = vega > 0, so the price function is strictly increasing.
///
/// 1. Start from the analytical guess and expand the bracket (doubling)
///    until `price_fn(lo) ≤ target ≤ price_fn(hi)`.
/// 2. Bisect `BISECTION_STEPS` times to narrow the bracket by ~10⁶×.
fn bracket_and_bisect(
    target: f64,
    price_fn: &dyn Fn(f64) -> f64,
    analytical_guess: f64,
    vol_ceil: f64,
) -> f64 {
    let mut lo = VOL_FLOOR;
    let mut hi = analytical_guess.max(VOL_FLOOR * 2.0);

    // Expand bracket upward until price_fn(hi) ≥ target
    for _ in 0..64 {
        if price_fn(hi) >= target || hi >= vol_ceil {
            break;
        }
        lo = hi;
        hi = (hi * 2.0).min(vol_ceil);
    }

    // If lo already overshoots, reset it
    if price_fn(lo) > target {
        lo = VOL_FLOOR;
    }

    // Bisect to narrow the bracket
    for _ in 0..BISECTION_STEPS {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if price_fn(mid) > target {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    0.5 * (lo + hi)
}

/// Verify that the Black-76 solver converged.
fn verify_convergence_black(
    sigma: f64,
    target: f64,
    forward: f64,
    strike: f64,
    t: f64,
    is_call: bool,
) -> crate::Result<f64> {
    let final_model = if is_call {
        black_call(forward, strike, sigma, t)
    } else {
        black_put(forward, strike, sigma, t)
    };
    let scale = forward.max(strike);
    check_residual(sigma, final_model, target, scale)
}

/// Verify that the Bachelier solver converged.
fn verify_convergence_bachelier(
    sigma: f64,
    target: f64,
    forward: f64,
    strike: f64,
    t: f64,
    is_call: bool,
) -> crate::Result<f64> {
    let final_model = if is_call {
        bachelier_call(forward, strike, sigma, t)
    } else {
        bachelier_put(forward, strike, sigma, t)
    };
    let scale = forward.abs().max(strike.abs()).max(1e-10);
    check_residual(sigma, final_model, target, scale)
}

/// Common convergence check: compare model vs target price.
///
/// Uses a tolerance that accounts for both relative error on the target
/// and the absolute floating-point accuracy floor.
#[inline]
fn check_residual(sigma: f64, model: f64, target: f64, scale: f64) -> crate::Result<f64> {
    let residual = (model - target).abs();
    // Generous relative tolerance (1e-8) with an absolute floor for tiny prices.
    // The Householder refinement achieves much tighter convergence; this is
    // just a safety net to catch genuine non-convergence.
    let tol = (target * 1e-8).max(scale * 1e-14);

    if residual > tol {
        return Err(InputError::VolatilityConversionFailed {
            tolerance: tol,
            residual,
        }
        .into());
    }

    Ok(sigma)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Base round-trip tolerance on implied volatility (σ_implied vs σ_true).
    const VOL_TOL: f64 = 1e-12;

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Compute a dynamic tolerance that accounts for the fundamental
    /// floating-point precision limit of the OTM conversion.
    ///
    /// For deep ITM options, `otm_price = price - intrinsic` loses
    /// significant digits. The resulting vol error is bounded by
    /// `ε × intrinsic / vega`, which exceeds 1e-12 for extreme cases.
    fn rt_tolerance_black(forward: f64, strike: f64, sigma: f64, t: f64, is_call: bool) -> f64 {
        let intrinsic = if is_call {
            (forward - strike).max(0.0)
        } else {
            (strike - forward).max(0.0)
        };
        let vega = black_vega(forward, strike, sigma, t);
        let precision_limit = if vega > f64::MIN_POSITIVE {
            f64::EPSILON * intrinsic / vega
        } else {
            0.0
        };
        // Use base tolerance or the cancellation-limited precision (with 4× safety factor)
        VOL_TOL.max(precision_limit * 4.0)
    }

    /// Compute price from vol, then recover vol, assert round-trip accuracy.
    fn assert_rt_black(forward: f64, strike: f64, sigma: f64, t: f64, is_call: bool) {
        let price = if is_call {
            black_call(forward, strike, sigma, t)
        } else {
            black_put(forward, strike, sigma, t)
        };

        let implied = implied_vol_black(price, forward, strike, t, is_call).unwrap_or_else(|_| {
            panic!(
                "implied_vol_black failed: F={forward}, K={strike}, σ={sigma}, T={t}, \
                 call={is_call}, price={price:.6e}"
            )
        });

        let err = (implied - sigma).abs();
        let tol = rt_tolerance_black(forward, strike, sigma, t, is_call);
        assert!(
            err < tol,
            "Black round-trip |{implied:.15e} - {sigma:.15e}| = {err:.2e} > {tol:.2e} \
             (F={forward}, K={strike}, T={t}, call={is_call})"
        );
    }

    fn assert_rt_bach(forward: f64, strike: f64, sigma: f64, t: f64, is_call: bool) {
        let price = if is_call {
            bachelier_call(forward, strike, sigma, t)
        } else {
            bachelier_put(forward, strike, sigma, t)
        };

        let implied =
            implied_vol_bachelier(price, forward, strike, t, is_call).unwrap_or_else(|_| {
                panic!(
                    "implied_vol_bachelier failed: F={forward}, K={strike}, σ_n={sigma}, T={t}, \
                     call={is_call}, price={price:.6e}"
                )
            });

        let err = (implied - sigma).abs();
        assert!(
            err < VOL_TOL,
            "Bachelier round-trip |{implied:.15e} - {sigma:.15e}| = {err:.2e} > {VOL_TOL:.2e} \
             (F={forward}, K={strike}, T={t}, call={is_call})"
        );
    }

    // ── Black-76: Round-Trip Tests ───────────────────────────────────────

    #[test]
    fn black_rt_atm_various_expiries() {
        let f = 0.05;
        let k = 0.05;
        let sigma = 0.20;
        for &t in &[0.25, 0.5, 1.0, 2.0, 5.0, 10.0] {
            assert_rt_black(f, k, sigma, t, true);
            assert_rt_black(f, k, sigma, t, false);
        }
    }

    #[test]
    fn black_rt_various_vols() {
        let f = 0.05;
        let k = 0.05;
        let t = 1.0;
        for &sigma in &[0.01, 0.05, 0.10, 0.20, 0.50, 1.0, 2.0] {
            assert_rt_black(f, k, sigma, t, true);
            assert_rt_black(f, k, sigma, t, false);
        }
    }

    #[test]
    fn black_rt_otm_calls() {
        let f = 0.05;
        let sigma = 0.20;
        let t = 1.0;
        for &k in &[0.055, 0.06, 0.07, 0.08, 0.10] {
            assert_rt_black(f, k, sigma, t, true);
        }
    }

    #[test]
    fn black_rt_otm_puts() {
        let f = 0.05;
        let sigma = 0.20;
        let t = 1.0;
        for &k in &[0.045, 0.04, 0.03, 0.02] {
            assert_rt_black(f, k, sigma, t, false);
        }
    }

    #[test]
    fn black_rt_itm_calls() {
        let f = 0.05;
        let sigma = 0.20;
        let t = 1.0;
        for &k in &[0.045, 0.04, 0.03, 0.02] {
            assert_rt_black(f, k, sigma, t, true);
        }
    }

    #[test]
    fn black_rt_itm_puts() {
        let f = 0.05;
        let sigma = 0.20;
        let t = 1.0;
        for &k in &[0.055, 0.06, 0.07, 0.08] {
            assert_rt_black(f, k, sigma, t, false);
        }
    }

    #[test]
    fn black_rt_deep_otm() {
        let f = 0.05;
        let sigma = 0.20;
        let t = 1.0;
        // Call K = 2F, Put K = F/2
        assert_rt_black(f, 0.10, sigma, t, true);
        assert_rt_black(f, 0.025, sigma, t, false);
    }

    #[test]
    fn black_rt_near_zero_vol() {
        let f = 0.05;
        let k = 0.05;
        let t = 1.0;
        assert_rt_black(f, k, 0.001, t, true);
        assert_rt_black(f, k, 0.001, t, false);
    }

    #[test]
    fn black_rt_high_vol() {
        let f = 0.05;
        let k = 0.05;
        let t = 1.0;
        assert_rt_black(f, k, 3.0, t, true);
        assert_rt_black(f, k, 3.0, t, false);
    }

    #[test]
    fn black_rt_short_expiry() {
        let f = 0.05;
        let sigma = 0.20;
        for &k in &[0.05, 0.055, 0.045] {
            assert_rt_black(f, k, sigma, 0.01, true);
            assert_rt_black(f, k, sigma, 0.01, false);
        }
    }

    #[test]
    fn black_rt_long_expiry() {
        let f = 0.05;
        let k = 0.05;
        let sigma = 0.20;
        assert_rt_black(f, k, sigma, 30.0, true);
        assert_rt_black(f, k, sigma, 30.0, false);
    }

    #[test]
    fn black_rt_equity_like() {
        // Typical equity parameters: S=100, K=100, σ=30%, T=0.5
        let f = 100.0;
        let sigma = 0.30;
        let t = 0.5;
        for &k in &[80.0, 90.0, 100.0, 110.0, 120.0] {
            assert_rt_black(f, k, sigma, t, true);
            assert_rt_black(f, k, sigma, t, false);
        }
    }

    // ── Black-76: Put-Call Parity ────────────────────────────────────────

    #[test]
    fn black_put_call_parity_consistency() {
        let f = 0.05;
        let k = 0.06;
        let sigma = 0.25;
        let t = 1.0;

        let call_price = black_call(f, k, sigma, t);
        let put_price = black_put(f, k, sigma, t);

        let vol_from_call =
            implied_vol_black(call_price, f, k, t, true).expect("call should succeed");
        let vol_from_put =
            implied_vol_black(put_price, f, k, t, false).expect("put should succeed");

        assert!(
            (vol_from_call - vol_from_put).abs() < VOL_TOL,
            "Put-call parity: vol_call={vol_from_call:.15e} != vol_put={vol_from_put:.15e}"
        );
    }

    #[test]
    fn black_put_call_parity_itm() {
        let f = 0.05;
        let k = 0.04; // call is ITM
        let sigma = 0.25;
        let t = 1.0;

        let call_price = black_call(f, k, sigma, t);
        let put_price = black_put(f, k, sigma, t);

        let vol_from_call =
            implied_vol_black(call_price, f, k, t, true).expect("ITM call should succeed");
        let vol_from_put =
            implied_vol_black(put_price, f, k, t, false).expect("OTM put should succeed");

        assert!(
            (vol_from_call - vol_from_put).abs() < VOL_TOL,
            "ITM put-call: vol_call={vol_from_call:.15e} != vol_put={vol_from_put:.15e}"
        );
    }

    // ── Black-76: Known Values ───────────────────────────────────────────

    #[test]
    fn black_known_atm_100() {
        // F = K = 100, σ = 20%, T = 1 → well-known ATM price
        let f = 100.0;
        let k = 100.0;
        let t = 1.0;
        let price = black_call(f, k, 0.20, t);
        let implied = implied_vol_black(price, f, k, t, true).expect("known ATM should succeed");
        assert!(
            (implied - 0.20).abs() < 1e-13,
            "Known ATM: expected 0.20, got {implied}"
        );
    }

    #[test]
    fn black_known_otm_100() {
        // F = 100, K = 110, σ = 25%, T = 0.5
        let f = 100.0;
        let k = 110.0;
        let sigma = 0.25;
        let t = 0.5;
        let price = black_call(f, k, sigma, t);
        let implied = implied_vol_black(price, f, k, t, true).expect("known OTM should succeed");
        assert!(
            (implied - sigma).abs() < 1e-13,
            "Known OTM: expected {sigma}, got {implied}"
        );
    }

    // ── Black-76: Boundary / Edge Cases ──────────────────────────────────

    #[test]
    fn black_intrinsic_only() {
        let f = 0.05;
        let k = 0.04;
        let t = 1.0;
        let intrinsic = f - k;
        let vol = implied_vol_black(intrinsic, f, k, t, true).expect("intrinsic should succeed");
        assert_eq!(vol, 0.0, "Intrinsic price should give vol = 0");
    }

    #[test]
    fn black_zero_price_otm() {
        let f = 0.05;
        let k = 0.06;
        let t = 1.0;
        let vol = implied_vol_black(0.0, f, k, t, true).expect("zero OTM should succeed");
        assert_eq!(vol, 0.0);
    }

    // ── Black-76: Error Cases ────────────────────────────────────────────

    #[test]
    fn black_err_negative_price() {
        assert!(implied_vol_black(-0.001, 0.05, 0.05, 1.0, true).is_err());
    }

    #[test]
    fn black_err_nan_price() {
        assert!(implied_vol_black(f64::NAN, 0.05, 0.05, 1.0, true).is_err());
    }

    #[test]
    fn black_err_non_positive_forward() {
        assert!(implied_vol_black(0.001, 0.0, 0.05, 1.0, true).is_err());
        assert!(implied_vol_black(0.001, -0.01, 0.05, 1.0, true).is_err());
    }

    #[test]
    fn black_err_non_positive_strike() {
        assert!(implied_vol_black(0.001, 0.05, 0.0, 1.0, true).is_err());
    }

    #[test]
    fn black_err_non_positive_time() {
        assert!(implied_vol_black(0.001, 0.05, 0.05, 0.0, true).is_err());
        assert!(implied_vol_black(0.001, 0.05, 0.05, -1.0, true).is_err());
    }

    #[test]
    fn black_err_arbitrage_below_intrinsic() {
        let f = 0.05;
        let k = 0.04;
        let intrinsic = f - k;
        assert!(implied_vol_black(intrinsic - 0.001, f, k, 1.0, true).is_err());
    }

    #[test]
    fn black_err_price_at_upper_bound() {
        // Call price = forward → vol = ∞ (not representable)
        assert!(implied_vol_black(0.05, 0.05, 0.05, 1.0, true).is_err());
        // Put price = strike → vol = ∞
        assert!(implied_vol_black(0.05, 0.05, 0.05, 1.0, false).is_err());
    }

    #[test]
    fn black_err_infinite_inputs() {
        assert!(implied_vol_black(0.001, f64::INFINITY, 0.05, 1.0, true).is_err());
        assert!(implied_vol_black(0.001, 0.05, f64::INFINITY, 1.0, true).is_err());
        assert!(implied_vol_black(0.001, 0.05, 0.05, f64::INFINITY, true).is_err());
    }

    // ── Bachelier: Round-Trip Tests ──────────────────────────────────────

    #[test]
    fn bach_rt_atm_various_expiries() {
        let f = 0.03;
        let k = 0.03;
        let sigma = 0.005;
        for &t in &[0.25, 0.5, 1.0, 2.0, 5.0, 10.0] {
            assert_rt_bach(f, k, sigma, t, true);
            assert_rt_bach(f, k, sigma, t, false);
        }
    }

    #[test]
    fn bach_rt_various_vols() {
        let f = 0.03;
        let k = 0.03;
        let t = 1.0;
        for &sigma in &[0.001, 0.002, 0.005, 0.010, 0.020] {
            assert_rt_bach(f, k, sigma, t, true);
            assert_rt_bach(f, k, sigma, t, false);
        }
    }

    #[test]
    fn bach_rt_otm() {
        let f = 0.03;
        let sigma = 0.005;
        let t = 1.0;
        for &k in &[0.035, 0.04, 0.05] {
            assert_rt_bach(f, k, sigma, t, true);
        }
        for &k in &[0.025, 0.02, 0.01] {
            assert_rt_bach(f, k, sigma, t, false);
        }
    }

    #[test]
    fn bach_rt_itm() {
        let f = 0.03;
        let sigma = 0.005;
        let t = 1.0;
        for &k in &[0.025, 0.02] {
            assert_rt_bach(f, k, sigma, t, true);
        }
        for &k in &[0.035, 0.04] {
            assert_rt_bach(f, k, sigma, t, false);
        }
    }

    #[test]
    fn bach_rt_price_quoted_underlying() {
        // Normal vol in price units: F = K = 100 with σ_n = 20 (20% of spot).
        // The pre-fix absolute ceiling VOL_CEIL_BACH = 10.0 made this fail.
        assert_rt_bach(100.0, 100.0, 20.0, 1.0, true);
        assert_rt_bach(100.0, 100.0, 20.0, 1.0, false);
        // OTM/ITM variants at price scale
        assert_rt_bach(100.0, 110.0, 15.0, 0.5, true);
        assert_rt_bach(100.0, 90.0, 15.0, 0.5, false);
    }

    #[test]
    fn bach_rt_negative_rates() {
        // Bachelier is designed for negative rate environments
        let f = -0.005; // −50bp
        let k = 0.0;
        let sigma = 0.006; // 60bp normal vol
        let t = 1.0;
        // Call is OTM (F < K)
        assert_rt_bach(f, k, sigma, t, true);
        // Put is ITM → converted to OTM call internally
        assert_rt_bach(f, k, sigma, t, false);
    }

    #[test]
    fn bach_rt_deep_negative_rates() {
        let f = -0.02;
        let k = -0.01;
        let sigma = 0.008;
        let t = 2.0;
        assert_rt_bach(f, k, sigma, t, true);
        assert_rt_bach(f, k, sigma, t, false);
    }

    // ── Bachelier: Put-Call Parity ───────────────────────────────────────

    #[test]
    fn bach_put_call_parity() {
        let f = 0.03;
        let k = 0.04;
        let sigma = 0.005;
        let t = 1.0;

        let call_price = bachelier_call(f, k, sigma, t);
        let put_price = bachelier_put(f, k, sigma, t);

        let vol_call =
            implied_vol_bachelier(call_price, f, k, t, true).expect("call should succeed");
        let vol_put = implied_vol_bachelier(put_price, f, k, t, false).expect("put should succeed");

        assert!(
            (vol_call - vol_put).abs() < VOL_TOL,
            "Bachelier parity: vol_call={vol_call:.15e} != vol_put={vol_put:.15e}"
        );
    }

    // ── Bachelier: Edge Cases ────────────────────────────────────────────

    #[test]
    fn bach_intrinsic_only() {
        let f = 0.03;
        let k = 0.02;
        let intrinsic = f - k;
        let vol =
            implied_vol_bachelier(intrinsic, f, k, 1.0, true).expect("intrinsic should succeed");
        assert_eq!(vol, 0.0);
    }

    #[test]
    fn bach_zero_price_otm() {
        let vol =
            implied_vol_bachelier(0.0, 0.03, 0.04, 1.0, true).expect("zero OTM should succeed");
        assert_eq!(vol, 0.0);
    }

    // ── Bachelier: Error Cases ───────────────────────────────────────────

    #[test]
    fn bach_err_negative_price() {
        assert!(implied_vol_bachelier(-0.001, 0.03, 0.03, 1.0, true).is_err());
    }

    #[test]
    fn bach_err_non_positive_time() {
        assert!(implied_vol_bachelier(0.001, 0.03, 0.03, 0.0, true).is_err());
        assert!(implied_vol_bachelier(0.001, 0.03, 0.03, -1.0, true).is_err());
    }

    #[test]
    fn bach_err_nan_inputs() {
        assert!(implied_vol_bachelier(f64::NAN, 0.03, 0.03, 1.0, true).is_err());
        assert!(implied_vol_bachelier(0.001, f64::NAN, 0.03, 1.0, true).is_err());
        assert!(implied_vol_bachelier(0.001, 0.03, f64::NAN, 1.0, true).is_err());
        assert!(implied_vol_bachelier(0.001, 0.03, 0.03, f64::NAN, true).is_err());
    }

    #[test]
    fn bach_err_infinite_inputs() {
        assert!(implied_vol_bachelier(0.001, f64::INFINITY, 0.03, 1.0, true).is_err());
        assert!(implied_vol_bachelier(0.001, 0.03, f64::INFINITY, 1.0, true).is_err());
    }

    #[test]
    fn bach_err_arbitrage_below_intrinsic() {
        let f = 0.03;
        let k = 0.02;
        let intrinsic = f - k;
        assert!(implied_vol_bachelier(intrinsic - 0.001, f, k, 1.0, true).is_err());
    }

    // ── Cross-Model Sanity ───────────────────────────────────────────────

    #[test]
    fn black_vs_bachelier_atm_approx() {
        // For ATM options: σ_normal ≈ σ_lognormal × F (first-order)
        let f = 0.05;
        let k = 0.05;
        let sigma_ln = 0.20;
        let t = 1.0;

        let price = black_call(f, k, sigma_ln, t);
        let sigma_n =
            implied_vol_bachelier(price, f, k, t, true).expect("bachelier should succeed");

        // First-order ATM relationship: σ_n ≈ σ_ln × F
        let expected_sigma_n = sigma_ln * f;
        let rel_err = (sigma_n - expected_sigma_n).abs() / expected_sigma_n;
        assert!(
            rel_err < 0.05,
            "ATM normal ≈ lognormal × F: got σ_n={sigma_n:.6}, expected ≈{expected_sigma_n:.6}, \
             rel_err={rel_err:.4}"
        );
    }
}
