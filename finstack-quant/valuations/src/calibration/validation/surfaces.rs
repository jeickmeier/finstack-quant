//! Surface validators (volatility surfaces).

use crate::calibration::validation::ValidationConfig;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::{Error, Result};

/// Validate all volatility-surface constraints.
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

/// Validate no calendar spread arbitrage.
///
/// Checks that total variance (sigma^2 T) is monotonically increasing with expiry,
/// ensuring that longer-dated options are not cheaper than shorter-dated ones.
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
            let vol = surface.value_checked(expiry, *strike)?;
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

/// Validate no butterfly arbitrage.
///
/// Checks that total variance (sigma^2 T) is convex in strike, ensuring that
/// butterfly spreads have non-negative value.
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

            let v1 = surface.value_checked(expiry, k1)?;
            let v2 = surface.value_checked(expiry, k2)?;
            let v3 = surface.value_checked(expiry, k3)?;

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
            let vol = surface.value_checked(expiry, *strike)?;

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
