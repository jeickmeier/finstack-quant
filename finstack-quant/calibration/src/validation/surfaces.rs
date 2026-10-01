//! Volatility-surface validators.
//!
//! Prefer the forward-aware validators when per-expiry forwards are available.
//! The forward-free variants remain as heuristics for callers without them.

use crate::validation::ValidationConfig;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::closed_form::black_call;

/// Validate all volatility-surface constraints.
///
/// Forward-free variant. Prefer
/// [`validate_surface_with_forwards`] when per-expiry forwards are available.
///
/// # Arguments
///
/// * `surface` - Volatility surface whose expiry/strike grid and quoted
///   annualized volatilities are checked.
/// * `config` - Validation tolerances, volatility cap, and strict-versus-
///   lenient arbitrage policy.
pub fn validate_surface(surface: &VolSurface, config: &ValidationConfig) -> Result<()> {
    validate_calendar_spread(surface, config)?;
    validate_butterfly_spread(surface, config)?;
    validate_vol_bounds(surface, config)?;
    Ok(())
}

/// Validate all volatility-surface constraints using per-expiry forwards.
///
/// Checks calendar monotonicity at fixed forward log-moneyness, call-price
/// convexity in strike, and volatility bounds.
///
/// # Arguments
///
/// * `surface` - Volatility surface whose expiry/strike grid is checked.
/// * `config` - Validation tolerances and strict-versus-lenient policy.
/// * `forwards` - Forward level `F(T_i)` for each expiry on the surface's
///   grid, in the same order as `surface.expiries()`. All must be positive.
///
/// # Errors
///
/// Returns an error on the first violated constraint (in strict mode), or if
/// `forwards` is malformed.
pub fn validate_surface_with_forwards(
    surface: &VolSurface,
    config: &ValidationConfig,
    forwards: &[f64],
) -> Result<()> {
    validate_forwards_input(surface, forwards)?;
    validate_calendar_spread_with_forwards(surface, config, forwards)?;
    validate_butterfly_call_convexity(surface, config, forwards)?;
    validate_vol_bounds(surface, config)?;
    Ok(())
}

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

/// Validate calendar monotonicity of total variance at fixed forward
/// log-moneyness, skipping probes outside the quoted strike range.
///
/// # Arguments
///
/// * `surface` - Volatility surface under test.
/// * `config` - Arbitrage-check toggle, tolerance, and strict/lenient policy.
/// * `forwards` - `F(T_i)` per expiry, aligned with `surface.expiries()`.
///
/// # Errors
///
/// In strict mode (`lenient_arbitrage = false`), returns an error listing the
/// violating `(k, T)` points. Lenient mode logs warnings and returns `Ok`.
pub fn validate_calendar_spread_with_forwards(
    surface: &VolSurface,
    config: &ValidationConfig,
    forwards: &[f64],
) -> Result<()> {
    if !config.check_arbitrage {
        return Ok(());
    }
    validate_forwards_input(surface, forwards)?;

    let strikes = surface.strikes();
    let expiries = surface.expiries();
    let (Some(&k_min), Some(&k_max)) = (strikes.first(), strikes.last()) else {
        return Ok(());
    };

    let mut violations: Vec<(f64, f64, f64, f64, f64)> = Vec::new();

    for i in 1..expiries.len() {
        let (t1, f1) = (expiries[i - 1], forwards[i - 1]);
        let (t2, f2) = (expiries[i], forwards[i]);
        for &strike in strikes {
            let k = (strike / f2).ln();
            let strike_prev = f1 * k.exp();
            if strike_prev < k_min || strike_prev > k_max {
                continue;
            }
            let v1 = finstack_quant_models::volatility::get_surface_vol(surface, t1, strike_prev)?;
            let v2 = finstack_quant_models::volatility::get_surface_vol(surface, t2, strike)?;
            let w1 = v1 * v1 * t1;
            let w2 = v2 * v2 * t2;
            if w2 < w1 - config.tolerance {
                violations.push((k, t1, t2, w1, w2));
                if config.lenient_arbitrage {
                    tracing::warn!(
                        "Calendar spread arbitrage at fixed forward-moneyness k={:.4}: \
                         w({:.4}y)={:.6} > w({:.4}y)={:.6} in {}",
                        k,
                        t1,
                        w1,
                        t2,
                        w2,
                        surface.id().as_str()
                    );
                }
            }
        }
    }

    if !violations.is_empty() && !config.lenient_arbitrage {
        let details: Vec<String> = violations
            .iter()
            .take(5)
            .map(|(k, t1, t2, w1, w2)| {
                format!("k={k:.4}: w({t1:.4}y)={w1:.6} > w({t2:.4}y)={w2:.6}")
            })
            .collect();
        let suffix = if violations.len() > 5 {
            format!(" (and {} more)", violations.len() - 5)
        } else {
            String::new()
        };
        return Err(Error::Validation(format!(
            "Calendar spread arbitrage (fixed forward-moneyness) at {} point(s) in {}: [{}]{}. \
             Total variance must be non-decreasing in expiry at fixed k = ln(K/F_T).",
            violations.len(),
            surface.id().as_str(),
            details.join("; "),
            suffix
        )));
    }

    Ok(())
}

/// Validate that undiscounted call prices are convex in strike and that every
/// adjacent vertical spread costs between zero and its strike width.
///
/// # Arguments
///
/// * `surface` - Volatility surface under test.
/// * `config` - Arbitrage-check toggle, tolerance, and strict/lenient policy.
///   `config.tolerance` is applied *relative to the forward* (prices scale
///   with `F`).
/// * `forwards` - `F(T_i)` per expiry, aligned with `surface.expiries()`.
///
/// # Errors
///
/// In strict mode, returns an error listing the violating `(K, T)` points.
pub fn validate_butterfly_call_convexity(
    surface: &VolSurface,
    config: &ValidationConfig,
    forwards: &[f64],
) -> Result<()> {
    if !config.check_arbitrage {
        return Ok(());
    }
    validate_forwards_input(surface, forwards)?;

    let strikes = surface.strikes();
    let expiries = surface.expiries();
    if strikes.len() < 2 {
        return Ok(());
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

        for (strike_pair, call_pair) in strikes.windows(2).zip(calls.windows(2)) {
            let strike_width = strike_pair[1] - strike_pair[0];
            let spread = call_pair[0] - call_pair[1];
            if spread < -price_tol || spread > strike_width + price_tol {
                let detail = format!(
                    "Vertical spread at T={expiry:.4}y, K=[{:.4}, {:.4}] costs {spread:.6e}; \
                     expected a value in [0, strike width {strike_width:.6e}]",
                    strike_pair[0], strike_pair[1]
                );
                if config.lenient_arbitrage {
                    tracing::warn!("{} in {}", detail, surface.id().as_str());
                }
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
                if config.lenient_arbitrage {
                    tracing::warn!(
                        "Butterfly arbitrage (negative density) at T={:.4}, K={:.4} in {}: \
                         call={:.6e} above chord={:.6e}",
                        expiry,
                        k2,
                        surface.id().as_str(),
                        c2,
                        c_interp
                    );
                }
            }
        }
    }

    if !violations.is_empty() && !config.lenient_arbitrage {
        let details: Vec<&str> = violations.iter().take(5).map(String::as_str).collect();
        let suffix = if violations.len() > 5 {
            format!(" (and {} more)", violations.len() - 5)
        } else {
            String::new()
        };
        return Err(Error::Validation(format!(
            "Call-spread arbitrage at {} point(s) in {}: [{}]{}. \
             Undiscounted call prices must be convex in strike, and adjacent vertical \
             spreads must cost between zero and their strike width.",
            violations.len(),
            surface.id().as_str(),
            details.join("; "),
            suffix
        )));
    }

    Ok(())
}

/// Validate calendar monotonicity at fixed absolute strike.
///
/// Prefer [`validate_calendar_spread_with_forwards`] when forwards vary by
/// expiry.
///
/// # Arguments
///
/// * `surface` - Volatility surface tested along every strike across increasing
///   expiry nodes.
/// * `config` - Arbitrage-check toggle, numerical tolerance, and policy that
///   turns violations into errors or warnings.
pub fn validate_calendar_spread(surface: &VolSurface, config: &ValidationConfig) -> Result<()> {
    if !config.check_arbitrage {
        return Ok(());
    }

    // Total variance (σ²T) must be monotonically increasing with time to prevent calendar arbitrage.
    // This is a fundamental no-arbitrage condition: longer-dated options must have at least
    // as much total variance as shorter-dated options at the same strike.
    let strikes = surface.strikes();
    let expiries = surface.expiries();
    let mut violations: Vec<(f64, f64, f64, f64)> = Vec::new(); // (strike, expiry, actual, expected)

    for strike in strikes {
        let mut prev_total_var = 0.0;
        let mut prev_expiry = 0.0_f64;

        for &expiry in expiries {
            let vol = finstack_quant_models::volatility::get_surface_vol(surface, expiry, *strike)?;
            let total_var = vol * vol * expiry; // σ²T

            // Check monotonicity of total variance
            if total_var < prev_total_var - config.tolerance {
                violations.push((*strike, expiry, total_var, prev_total_var));

                if config.lenient_arbitrage {
                    tracing::warn!(
                            "Calendar spread arbitrage detected: total variance {:.6} < {:.6} at K={}, T={:.4} (prev T={:.4}) in {}. \
                            Consider using SVI or monotone convex fitting for arbitrage-free surfaces.",
                            total_var,
                            prev_total_var,
                            strike,
                            expiry,
                            prev_expiry,
                            surface.id().as_str()
                        );
                }
            }

            prev_total_var = total_var;
            prev_expiry = expiry;
        }
    }

    // In strict mode (default), fail on any calendar arbitrage violations
    if !violations.is_empty() && !config.lenient_arbitrage {
        let details: Vec<String> = violations
            .iter()
            .take(5)
            .map(|(k, t, actual, expected)| {
                format!(
                    "K={:.2}, T={:.4}y (var={:.6} < {:.6})",
                    k, t, actual, expected
                )
            })
            .collect();
        let suffix = if violations.len() > 5 {
            format!(" (and {} more)", violations.len() - 5)
        } else {
            String::new()
        };
        return Err(Error::Validation(format!(
            "Calendar spread arbitrage detected at {} point(s) in {}: [{}]{}. \
                Total variance must be monotonically increasing in expiry. \
                Consider using SVI or monotone convex fitting for arbitrage-free surfaces.",
            violations.len(),
            surface.id().as_str(),
            details.join("; "),
            suffix
        )));
    }

    Ok(())
}

/// Apply a forward-free convexity-band heuristic to total variance.
///
/// Prefer [`validate_butterfly_call_convexity`] when per-expiry forwards are
/// available.
///
/// # Arguments
///
/// * `surface` - Volatility surface tested along every expiry across ordered
///   strike nodes.
/// * `config` - Arbitrage-check toggle and allowed convexity-ratio band used
///   to classify butterfly violations.
pub fn validate_butterfly_spread(surface: &VolSurface, config: &ValidationConfig) -> Result<()> {
    if !config.check_arbitrage {
        return Ok(());
    }

    // Check convexity of total variance in strike dimension.
    // Proper butterfly arbitrage check requires that total variance (σ²T) is convex in strike,
    // which prevents risk-free arbitrage via butterfly spreads.
    let strikes = surface.strikes();
    let expiries = surface.expiries();

    if strikes.len() < 3 {
        return Ok(()); // Need at least 3 strikes to check
    }

    let mut violations: Vec<(f64, f64, f64, f64, f64)> = Vec::new(); // (expiry, strike, actual, interp, ratio)

    for &expiry in expiries {
        for i in 1..strikes.len() - 1 {
            let k1 = strikes[i - 1];
            let k2 = strikes[i];
            let k3 = strikes[i + 1];

            let v1 = finstack_quant_models::volatility::get_surface_vol(surface, expiry, k1)?;
            let v2 = finstack_quant_models::volatility::get_surface_vol(surface, expiry, k2)?;
            let v3 = finstack_quant_models::volatility::get_surface_vol(surface, expiry, k3)?;

            // Convert to total variance for proper arbitrage check
            let w1 = v1 * v1 * expiry;
            let w2 = v2 * v2 * expiry;
            let w3 = v3 * v3 * expiry;

            // Check convexity of total variance: w2 should be ≤ linear interpolation
            let weight = (k2 - k1) / (k3 - k1);
            let w2_interpolated = w1 + weight * (w3 - w1);

            let ratio = if w2_interpolated.abs() > 1e-12 {
                w2 / w2_interpolated
            } else {
                1.0
            };

            if w2 > w2_interpolated * config.butterfly_upper_ratio
                || w2 < w2_interpolated * config.butterfly_lower_ratio
            {
                violations.push((expiry, k2, w2, w2_interpolated, ratio));

                if config.lenient_arbitrage {
                    tracing::warn!(
                        "Potential butterfly arbitrage at T={:.2}, K={:.2} in {}: \
                            total_var={:.6} vs interpolated={:.6} (ratio {:.2}). \
                            Consider SVI or monotone convex fitting.",
                        expiry,
                        k2,
                        surface.id().as_str(),
                        w2,
                        w2_interpolated,
                        ratio
                    );
                }
            }
        }
    }

    // In strict mode (default), fail on any butterfly arbitrage violations
    if !violations.is_empty() && !config.lenient_arbitrage {
        let details: Vec<String> = violations
            .iter()
            .take(5)
            .map(|(t, k, actual, interp, ratio)| {
                format!(
                    "T={:.2}y, K={:.2} (var={:.6} vs interp={:.6}, ratio={:.2})",
                    t, k, actual, interp, ratio
                )
            })
            .collect();
        let suffix = if violations.len() > 5 {
            format!(" (and {} more)", violations.len() - 5)
        } else {
            String::new()
        };
        return Err(Error::Validation(format!(
            "Butterfly spread arbitrage detected at {} point(s) in {}: [{}]{}. \
                Total variance must be convex in strike. \
                Consider using SVI or monotone convex fitting for arbitrage-free surfaces.",
            violations.len(),
            surface.id().as_str(),
            details.join("; "),
            suffix
        )));
    }

    Ok(())
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
pub fn validate_vol_bounds(surface: &VolSurface, config: &ValidationConfig) -> Result<()> {
    let strikes = surface.strikes();
    let expiries = surface.expiries();

    for &expiry in expiries {
        for strike in strikes {
            let vol = finstack_quant_models::volatility::get_surface_vol(surface, expiry, *strike)?;

            // Volatility should be positive
            if vol <= 0.0 {
                return Err(Error::Validation(format!(
                    "Non-positive volatility {:.2}% at T={}, K={} in {}",
                    vol * 100.0,
                    expiry,
                    strike,
                    surface.id().as_str()
                )));
            }

            // Cap at reasonable maximum (500% vol)
            if vol > config.max_volatility {
                return Err(Error::Validation(format!(
                    "Unreasonably high volatility {:.2}% at T={}, K={} in {} (limit: {:.2}%)",
                    vol * 100.0,
                    expiry,
                    strike,
                    surface.id().as_str(),
                    config.max_volatility * 100.0
                )));
            }
        }
    }

    Ok(())
}
