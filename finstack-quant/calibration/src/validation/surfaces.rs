//! Forward-aware volatility-surface acceptance checks.
//!
//! Input and evaluation errors propagate directly. Only collected acceptance
//! violations may be handled by a calibration report's Warn/Error policy.

use crate::validation::ValidationConfig;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::closed_form::black_call;

/// Check that the `forwards` slice matches the surface's expiry grid.
fn validate_forwards_input(surface: &VolSurface, forwards: &[f64]) -> Result<()> {
    let n = surface.expiries().len();
    if forwards.len() != n {
        return Err(Error::Validation(format!(
            "Surface validation requires one forward per expiry: got {} forwards for {} expiries in {}",
            forwards.len(),
            n,
            surface.id().as_str()
        )));
    }
    if let Some(f) = forwards.iter().find(|f| !f.is_finite() || **f <= 0.0) {
        return Err(Error::Validation(format!(
            "Surface validation requires positive finite forwards; got {} in {}",
            f,
            surface.id().as_str()
        )));
    }
    Ok(())
}

fn calendar_violations(
    surface: &VolSurface,
    config: &ValidationConfig,
    forwards: &[f64],
) -> Result<Vec<String>> {
    if !config.check_arbitrage {
        return Ok(Vec::new());
    }

    let strikes = surface.strikes();
    let expiries = surface.expiries();
    let (Some(&k_min), Some(&k_max)) = (strikes.first(), strikes.last()) else {
        return Ok(Vec::new());
    };

    let mut violations: Vec<(f64, f64, f64, f64, f64)> = Vec::new();

    for i in 1..expiries.len() {
        let (t1, f1) = (expiries[i - 1], forwards[i - 1]);
        let (t2, f2) = (expiries[i], forwards[i]);
        for &strike in strikes {
            let k = (strike / f2).ln();
            let strike_prev = f1 * k.exp();
            ensure_finite(&[k, strike_prev], "forward moneyness")?;
            if strike_prev < k_min || strike_prev > k_max {
                continue;
            }
            let v1 = finstack_quant_models::volatility::get_surface_vol(surface, t1, strike_prev)?;
            let v2 = finstack_quant_models::volatility::get_surface_vol(surface, t2, strike)?;
            ensure_finite(&[k, strike_prev, v1, v2], "calendar probe")?;
            let w1 = v1 * v1 * t1;
            let w2 = v2 * v2 * t2;
            ensure_finite(&[w1, w2], "total variance")?;
            if w2 < w1 - config.tolerance {
                violations.push((k, t1, t2, w1, w2));
            }
        }
    }

    Ok(describe_violations(
        surface,
        &violations,
        "Calendar spread arbitrage (fixed forward-moneyness)",
        "Total variance must be non-decreasing in expiry at fixed k = ln(K/F_T).",
        |(k, t1, t2, w1, w2)| format!("k={k:.4}: w({t1:.4}y)={w1:.6} > w({t2:.4}y)={w2:.6}"),
    ))
}

fn butterfly_violations(
    surface: &VolSurface,
    config: &ValidationConfig,
    forwards: &[f64],
) -> Result<Vec<String>> {
    if !config.check_arbitrage {
        return Ok(Vec::new());
    }

    let strikes = surface.strikes();
    let expiries = surface.expiries();
    if strikes.len() < 2 {
        return Ok(Vec::new());
    }

    let mut violations: Vec<String> = Vec::new();

    for (i, &expiry) in expiries.iter().enumerate() {
        let fwd = forwards[i];
        let price_tol = config.tolerance.max(1e-12) * fwd;
        let calls: Vec<f64> = strikes
            .iter()
            .map(|&k| {
                Ok(black_call(
                    fwd,
                    k,
                    finstack_quant_models::volatility::get_surface_vol(surface, expiry, k)?,
                    expiry,
                ))
            })
            .collect::<Result<_>>()?;

        ensure_finite(&calls, "Black call prices")?;
        ensure_finite(&[price_tol], "call-price tolerance")?;

        for (strike_pair, call_pair) in strikes.windows(2).zip(calls.windows(2)) {
            let strike_width = strike_pair[1] - strike_pair[0];
            let spread = call_pair[0] - call_pair[1];
            if spread < -price_tol || spread > strike_width + price_tol {
                let detail = format!(
                    "Vertical spread at T={expiry:.4}y, K=[{:.4}, {:.4}] costs {spread:.6e}; \
                     expected a value in [0, strike width {strike_width:.6e}]",
                    strike_pair[0], strike_pair[1]
                );
                violations.push(detail);
            }
        }

        for j in 1..strikes.len() - 1 {
            let (k1, k2, k3) = (strikes[j - 1], strikes[j], strikes[j + 1]);
            let (c1, c2, c3) = (calls[j - 1], calls[j], calls[j + 1]);

            // Butterfly: C(K2) must lie on or below the chord through
            // (K1, C1) and (K3, C3).
            let lambda = (k2 - k1) / (k3 - k1);
            let c_interp = c1 + lambda * (c3 - c1);
            if c2 > c_interp + price_tol {
                violations.push(format!(
                    "Butterfly at T={expiry:.4}y, K={k2:.4} \
                     (call={c2:.6e} > chord={c_interp:.6e})"
                ));
            }
        }
    }

    Ok(describe_violations(
        surface,
        &violations,
        "Call-spread arbitrage",
        "Undiscounted call prices must be convex in strike, and adjacent vertical spreads must cost between zero and their strike width.",
        String::clone,
    ))
}

/// Validate volatility bounds.
///
/// Ensures volatility is positive and within reasonable financial limits.
///
/// # Arguments
///
/// * `surface` - Volatility surface whose annualized values are inspected.
/// * `config` - Validation configuration supplying the maximum permissible
///   annualized volatility and other surface policy controls.
fn bounds_violations(surface: &VolSurface, config: &ValidationConfig) -> Result<Vec<String>> {
    let strikes = surface.strikes();
    let expiries = surface.expiries();
    let mut violations = Vec::new();

    for &expiry in expiries {
        for strike in strikes {
            let vol = finstack_quant_models::volatility::get_surface_vol(surface, expiry, *strike)?;

            ensure_finite(&[vol], "surface volatility")?;

            // Volatility should be positive
            if vol <= 0.0 {
                violations.push(format!(
                    "Non-positive volatility {:.2}% at T={}, K={} in {}",
                    vol * 100.0,
                    expiry,
                    strike,
                    surface.id().as_str()
                ));
            }

            // Cap at reasonable maximum (500% vol)
            if vol > config.max_volatility {
                violations.push(format!(
                    "Unreasonably high volatility {:.2}% at T={}, K={} in {} (limit: {:.2}%)",
                    vol * 100.0,
                    expiry,
                    strike,
                    surface.id().as_str(),
                    config.max_volatility * 100.0
                ));
            }
        }
    }

    Ok(violations)
}

/// Validate forward-moneyness calendars, call spreads, and volatility bounds.
///
/// # Arguments
///
/// * `surface` - Implied annualized decimal volatilities on expiry-year and absolute-strike grids.
/// * `config` - Positive comparison tolerance and volatility cap; `check_arbitrage=false`
///   skips calendar and call-spread checks but retains input checks and volatility bounds.
/// * `forwards` - One positive finite forward per expiry, in expiry order and strike units.
///
/// # Errors
///
/// Returns an error for malformed inputs, failed evaluation, or any acceptance violation.
pub fn validate_surface(
    surface: &VolSurface,
    config: &ValidationConfig,
    forwards: &[f64],
) -> Result<()> {
    violations_result(surface_violations(surface, config, forwards)?)
}

/// Validate nondecreasing total variance at fixed forward log-moneyness.
///
/// Probes outside the common quoted strike range are skipped.
///
/// # Arguments
///
/// * `surface` - Annualized decimal volatilities on expiry-year and absolute-strike grids.
/// * `config` - Comparison tolerance in total-variance units; `check_arbitrage=false` skips this check.
/// * `forwards` - One positive finite forward per expiry, in expiry order and strike units;
///   required even when arbitrage checks are disabled.
///
/// # Errors
///
/// Returns an error for malformed inputs, failed evaluation, or decreasing total variance.
pub fn validate_calendar_spread(
    surface: &VolSurface,
    config: &ValidationConfig,
    forwards: &[f64],
) -> Result<()> {
    validate_inputs(surface, config, forwards)?;
    violations_result(calendar_violations(surface, config, forwards)?)
}

/// Validate convex undiscounted Black calls and vertical-spread payoff bounds.
///
/// # Arguments
///
/// * `surface` - Annualized decimal volatilities on expiry-year and absolute-strike grids.
/// * `config` - Call-price tolerance `max(tolerance, 1e-12)` times the forward;
///   `check_arbitrage=false` skips this check.
/// * `forwards` - One positive finite forward per expiry, in expiry order and strike units;
///   required even when arbitrage checks are disabled.
///
/// # Errors
///
/// Returns an error for malformed inputs, failed evaluation, nonconvex calls, or a vertical
/// spread costing less than zero or more than its strike width, beyond tolerance.
pub fn validate_butterfly_spread(
    surface: &VolSurface,
    config: &ValidationConfig,
    forwards: &[f64],
) -> Result<()> {
    validate_inputs(surface, config, forwards)?;
    violations_result(butterfly_violations(surface, config, forwards)?)
}

/// Validate positive volatility bounded by the configured maximum.
///
/// # Arguments
///
/// * `surface` - Surface whose annualized decimal volatility is checked at every grid node.
/// * `config` - Maximum annualized decimal volatility; bounds apply regardless of `check_arbitrage`.
///
/// # Errors
///
/// Returns an error for malformed configuration, failed evaluation, or a volatility outside `(0, max_volatility]`.
pub fn validate_vol_bounds(surface: &VolSurface, config: &ValidationConfig) -> Result<()> {
    config.validate()?;
    violations_result(bounds_violations(surface, config)?)
}

pub(crate) fn surface_violations(
    surface: &VolSurface,
    config: &ValidationConfig,
    forwards: &[f64],
) -> Result<Vec<String>> {
    validate_inputs(surface, config, forwards)?;
    let mut violations = calendar_violations(surface, config, forwards)?;
    violations.extend(butterfly_violations(surface, config, forwards)?);
    violations.extend(bounds_violations(surface, config)?);
    Ok(violations)
}

pub(crate) fn violations_result(violations: Vec<String>) -> Result<()> {
    if violations.is_empty() {
        Ok(())
    } else {
        Err(Error::Validation(violations.join("; ")))
    }
}

fn validate_inputs(
    surface: &VolSurface,
    config: &ValidationConfig,
    forwards: &[f64],
) -> Result<()> {
    config.validate()?;
    validate_forwards_input(surface, forwards)?;
    if surface
        .strikes()
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.0)
    {
        return Err(Error::Validation(
            "Surface validation requires positive finite strikes".to_string(),
        ));
    }
    Ok(())
}

fn ensure_finite(values: &[f64], calculation: &str) -> Result<()> {
    if values.iter().any(|v| !v.is_finite()) {
        return Err(Error::Validation(format!(
            "Surface validation failed to evaluate finite {calculation}"
        )));
    }
    Ok(())
}

fn describe_violations<T>(
    surface: &VolSurface,
    violations: &[T],
    kind: &str,
    rule: &str,
    describe: impl Fn(&T) -> String,
) -> Vec<String> {
    if violations.is_empty() {
        return Vec::new();
    }
    let details: Vec<_> = violations.iter().take(5).map(describe).collect();
    vec![format!(
        "{kind} at {} point(s) in {}: [{}]. {rule}",
        violations.len(),
        surface.id().as_str(),
        details.join("; ")
    )]
}
