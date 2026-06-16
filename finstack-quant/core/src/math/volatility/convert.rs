//! ATM volatility conversion utilities across quoting conventions.
//!
//! The functions in this submodule convert volatility quotes by equating model
//! prices under normal, lognormal, and shifted-lognormal conventions, with
//! explicit validation and deterministic solver behavior.

use super::conventions::{validate_forward_for_convention, VolatilityConvention};
use super::pricing::{bachelier_call, black_call, black_shifted_call};
use crate::error::InputError;
use crate::math::{BrentSolver, Solver};
use crate::Result;

/// Convert ATM volatility between conventions.
///
/// Converts a volatility quote from one convention to another by equating option prices
/// at ATM (strike = forward). This is a deterministic conversion that returns explicit
/// errors rather than silently falling back to guesses.
///
/// # Arguments
/// * `vol` - Input volatility (must be positive and finite)
/// * `from_convention` - Source convention
/// * `to_convention` - Target convention
/// * `forward_rate` - Forward rate
/// * `time_to_expiry` - Time to expiry in years (must be non-negative)
///
/// # Errors
///
/// Returns an error if:
/// - `vol` is not positive and finite ([`InputError::InvalidVolatility`])
/// - `time_to_expiry` is negative ([`InputError::InvalidTimeToExpiry`])
/// - Forward rate is non-positive for lognormal conversion ([`InputError::NonPositiveForwardForLognormal`])
/// - Shifted forward is non-positive ([`InputError::NonPositiveShiftedForward`])
/// - Solver fails to converge ([`InputError::VolatilityConversionFailed`])
///
/// # Design Note
///
/// This function explicitly performs **ATM-only** conversion (strike = forward).
/// For surface-aware or strike-specific conversions, use a volatility surface
/// implementation that handles the full strike dimension.
///
/// # Example
///
/// ```rust
/// use finstack_quant_core::math::volatility::{convert_atm_volatility, VolatilityConvention};
/// # fn main() -> finstack_quant_core::Result<()> {
///
/// let forward = 0.05; // 5% forward rate
/// let normal_vol = 0.01; // 100bp normal vol
///
/// let lognormal_vol = convert_atm_volatility(
///     normal_vol,
///     VolatilityConvention::Normal,
///     VolatilityConvention::Lognormal,
///     forward,
///     1.0,
/// )?;
///
/// // Round-trip conversion
/// let recovered = convert_atm_volatility(
///     lognormal_vol,
///     VolatilityConvention::Lognormal,
///     VolatilityConvention::Normal,
///     forward,
///     1.0,
/// )?;
///
/// assert!((recovered - normal_vol).abs() < 1e-10);
/// # Ok(())
/// # }
/// ```
pub fn convert_atm_volatility(
    vol: f64,
    from_convention: VolatilityConvention,
    to_convention: VolatilityConvention,
    forward_rate: f64,
    time_to_expiry: f64,
) -> Result<f64> {
    // Validate inputs
    if !vol.is_finite() || vol <= 0.0 {
        return Err(InputError::InvalidVolatility { value: vol }.into());
    }

    if time_to_expiry < 0.0 {
        return Err(InputError::InvalidTimeToExpiry {
            value: time_to_expiry,
        }
        .into());
    }

    if !forward_rate.is_finite() {
        return Err(InputError::Invalid.into());
    }

    // Validate shifts for shifted-lognormal conventions
    if let VolatilityConvention::ShiftedLognormal { shift } = from_convention {
        if !shift.is_finite() {
            return Err(InputError::Invalid.into());
        }
    }
    if let VolatilityConvention::ShiftedLognormal { shift } = to_convention {
        if !shift.is_finite() {
            return Err(InputError::Invalid.into());
        }
    }

    // At expiry, option value is intrinsic and independent of volatility.
    // There is no unique price-matching volatility across conventions; preserve
    // the caller's quote deterministically (after basic validation above).
    if time_to_expiry == 0.0 {
        return Ok(vol);
    }

    // Validate forward rate for lognormal conventions
    validate_forward_for_convention(forward_rate, from_convention)?;
    validate_forward_for_convention(forward_rate, to_convention)?;

    // Early returns for identical convention (including same shift)
    match (from_convention, to_convention) {
        (VolatilityConvention::Lognormal, VolatilityConvention::Lognormal)
        | (VolatilityConvention::Normal, VolatilityConvention::Normal) => return Ok(vol),
        (
            VolatilityConvention::ShiftedLognormal { shift: s1 },
            VolatilityConvention::ShiftedLognormal { shift: s2 },
        ) if (s1 - s2).abs() < 1e-12 => return Ok(vol),
        _ => {}
    }

    // Price matching with numerical solver
    let f = forward_rate;
    let t = time_to_expiry.max(0.0);

    // Compute price under source convention (ATM: strike = forward)
    let price_from = match from_convention {
        VolatilityConvention::Normal => bachelier_call(f, f, vol, t),
        VolatilityConvention::Lognormal => black_call(f, f, vol, t),
        VolatilityConvention::ShiftedLognormal { shift } => black_shifted_call(f, f, vol, t, shift),
    };

    // Initial guess derived from ATM approximations
    let guess = compute_initial_guess(vol, from_convention, to_convention, f);

    // Objective: find sigma such that price(sigma, to_convention) = price_from.
    // Volatility is always positive; we clamp to a small floor to keep the
    // pricing functions in their valid domain and avoid a non-smooth landscape.
    const VOL_FLOOR: f64 = 1e-16;
    let objective = |sigma: f64| -> f64 {
        let sigma_safe = sigma.max(VOL_FLOOR);
        let p = match to_convention {
            VolatilityConvention::Normal => bachelier_call(f, f, sigma_safe, t),
            VolatilityConvention::Lognormal => black_call(f, f, sigma_safe, t),
            VolatilityConvention::ShiftedLognormal { shift } => {
                black_shifted_call(f, f, sigma_safe, t, shift)
            }
        };
        p - price_from
    };

    // Solve with Brent's method using default tolerance from BrentSolver
    let solver = BrentSolver::new();

    match solver.solve(objective, guess) {
        Ok(solved) => {
            let result = solved.max(VOL_FLOOR);
            if result.is_finite() && result > 0.0 {
                Ok(result)
            } else {
                Err(InputError::VolatilityConversionFailed {
                    tolerance: solver.tolerance,
                    residual: objective(solved).abs(),
                }
                .into())
            }
        }
        Err(_) => {
            // Solver failed - return explicit error with diagnostic info
            let residual = objective(guess).abs();
            Err(InputError::VolatilityConversionFailed {
                tolerance: solver.tolerance,
                residual,
            }
            .into())
        }
    }
}

/// Compute initial guess for volatility conversion solver.
fn compute_initial_guess(
    vol: f64,
    from_convention: VolatilityConvention,
    to_convention: VolatilityConvention,
    forward: f64,
) -> f64 {
    let guess = match (from_convention, to_convention) {
        (VolatilityConvention::Normal, VolatilityConvention::Lognormal) => {
            if forward.abs() > 1e-10 {
                vol / forward
            } else {
                vol
            }
        }
        (VolatilityConvention::Lognormal, VolatilityConvention::Normal) => vol * forward,
        (VolatilityConvention::Normal, VolatilityConvention::ShiftedLognormal { shift }) => {
            let shifted_f = forward + shift;
            if shifted_f.abs() > 1e-10 {
                vol / shifted_f
            } else {
                vol
            }
        }
        (VolatilityConvention::ShiftedLognormal { shift }, VolatilityConvention::Normal) => {
            vol * (forward + shift)
        }
        (VolatilityConvention::Lognormal, VolatilityConvention::ShiftedLognormal { shift }) => {
            let shifted_f = forward + shift;
            if shifted_f.abs() > 1e-10 {
                vol * forward / shifted_f
            } else {
                vol
            }
        }
        (VolatilityConvention::ShiftedLognormal { shift }, VolatilityConvention::Lognormal) => {
            if forward.abs() > 1e-10 {
                vol * (forward + shift) / forward
            } else {
                vol
            }
        }
        (
            VolatilityConvention::ShiftedLognormal { shift: s1 },
            VolatilityConvention::ShiftedLognormal { shift: s2 },
        ) => {
            // Converting between different shifts
            let f1 = forward + s1;
            let f2 = forward + s2;
            if f2.abs() > 1e-10 && f1.abs() > 1e-10 {
                vol * f1 / f2
            } else {
                vol
            }
        }
        _ => vol,
    };

    // Ensure guess is valid
    if !guess.is_finite() || guess <= 0.0 {
        (vol.abs() + 1e-6).max(1e-6)
    } else {
        guess
    }
}

#[cfg(test)]
mod tests {
    use super::super::pricing::{bachelier_call, black_call, black_shifted_call};
    use super::VolatilityConvention;
    use super::*;

    fn atm_price(convention: VolatilityConvention, forward: f64, vol: f64, t: f64) -> f64 {
        match convention {
            VolatilityConvention::Normal => bachelier_call(forward, forward, vol, t),
            VolatilityConvention::Lognormal => black_call(forward, forward, vol, t),
            VolatilityConvention::ShiftedLognormal { shift } => {
                black_shifted_call(forward, forward, vol, t, shift)
            }
        }
    }

    fn assert_price_matched(
        from_conv: VolatilityConvention,
        to_conv: VolatilityConvention,
        forward: f64,
        t: f64,
        from_vol: f64,
        to_vol: f64,
    ) {
        let p_from = atm_price(from_conv, forward, from_vol, t);
        let p_to = atm_price(to_conv, forward, to_vol, t);
        let err = (p_to - p_from).abs();
        assert!(
            err <= 1e-12,
            "ATM price mismatch: from={p_from}, to={p_to}, err={err}, forward={forward}, t={t}, from_vol={from_vol}, to_vol={to_vol}"
        );
    }

    #[test]
    fn test_normal_vs_lognormal_atm_conversion() {
        let forward = 0.05; // 5% forward rate
        let normal_vol = 0.01; // 100bp normal vol

        // Convert to lognormal
        let lognormal_vol = convert_atm_volatility(
            normal_vol,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            1.0,
        )
        .expect("conversion should succeed");

        assert_price_matched(
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            1.0,
            normal_vol,
            lognormal_vol,
        );

        // Convert back
        let recovered_normal = convert_atm_volatility(
            lognormal_vol,
            VolatilityConvention::Lognormal,
            VolatilityConvention::Normal,
            forward,
            1.0,
        )
        .expect("conversion should succeed");

        assert!((recovered_normal - normal_vol).abs() < 1e-10);
    }

    #[test]
    fn test_round_trip_normal_to_shifted_lognormal() {
        let forward = 0.03;
        let normal_vol = 0.008;
        let shift = 0.02;

        let shifted_ln = convert_atm_volatility(
            normal_vol,
            VolatilityConvention::Normal,
            VolatilityConvention::ShiftedLognormal { shift },
            forward,
            1.0,
        )
        .expect("conversion should succeed");

        assert_price_matched(
            VolatilityConvention::Normal,
            VolatilityConvention::ShiftedLognormal { shift },
            forward,
            1.0,
            normal_vol,
            shifted_ln,
        );

        let recovered = convert_atm_volatility(
            shifted_ln,
            VolatilityConvention::ShiftedLognormal { shift },
            VolatilityConvention::Normal,
            forward,
            1.0,
        )
        .expect("conversion should succeed");

        assert!(
            (recovered - normal_vol).abs() < 1e-8,
            "Round trip failed: got {recovered}, expected {normal_vol}"
        );
    }

    #[test]
    fn test_round_trip_lognormal_to_shifted_lognormal() {
        let forward = 0.04;
        let ln_vol = 0.25;
        let shift = 0.01;

        let shifted_ln = convert_atm_volatility(
            ln_vol,
            VolatilityConvention::Lognormal,
            VolatilityConvention::ShiftedLognormal { shift },
            forward,
            1.0,
        )
        .expect("conversion should succeed");

        assert_price_matched(
            VolatilityConvention::Lognormal,
            VolatilityConvention::ShiftedLognormal { shift },
            forward,
            1.0,
            ln_vol,
            shifted_ln,
        );

        let recovered = convert_atm_volatility(
            shifted_ln,
            VolatilityConvention::ShiftedLognormal { shift },
            VolatilityConvention::Lognormal,
            forward,
            1.0,
        )
        .expect("conversion should succeed");

        assert!(
            (recovered - ln_vol).abs() < 1e-8,
            "Round trip failed: got {recovered}, expected {ln_vol}"
        );
    }

    #[test]
    fn test_negative_forward_lognormal_errors() {
        let forward = -0.01; // -1% forward rate
        let normal_vol = 0.01;

        let result = convert_atm_volatility(
            normal_vol,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            1.0,
        );

        let err = result.expect_err("expected error for negative forward");
        assert!(
            err.to_string()
                .contains("Lognormal volatility requires positive forward"),
            "Expected NonPositiveForwardForLognormal error, got: {err}"
        );
    }

    #[test]
    fn test_negative_forward_with_shifted_lognormal() {
        let forward = -0.01; // -1% forward rate
        let normal_vol = 0.01;
        let shift = 0.02; // Shift makes (F + shift) = 1% > 0

        // This should succeed because shifted forward is positive
        let shifted_ln = convert_atm_volatility(
            normal_vol,
            VolatilityConvention::Normal,
            VolatilityConvention::ShiftedLognormal { shift },
            forward,
            1.0,
        )
        .expect("conversion should succeed with sufficient shift");

        assert!(shifted_ln > 0.0);
    }

    #[test]
    fn test_insufficient_shift_errors() {
        let forward = -0.03; // -3% forward rate
        let normal_vol = 0.01;
        let shift = 0.02; // Insufficient: (F + shift) = -1% < 0

        let result = convert_atm_volatility(
            normal_vol,
            VolatilityConvention::Normal,
            VolatilityConvention::ShiftedLognormal { shift },
            forward,
            1.0,
        );

        let err = result.expect_err("expected error for insufficient shift");
        assert!(
            err.to_string().contains("Shifted forward must be positive"),
            "Expected NonPositiveShiftedForward error, got: {err}"
        );
    }

    #[test]
    fn test_invalid_volatility_errors() {
        let forward = 0.05;

        // Zero volatility
        let result = convert_atm_volatility(
            0.0,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            1.0,
        );
        assert!(result.is_err());

        // Negative volatility
        let result = convert_atm_volatility(
            -0.1,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            1.0,
        );
        assert!(result.is_err());

        // NaN volatility
        let result = convert_atm_volatility(
            f64::NAN,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            1.0,
        );
        assert!(result.is_err());

        // Infinite volatility
        let result = convert_atm_volatility(
            f64::INFINITY,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            1.0,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_time_to_expiry_errors() {
        let result = convert_atm_volatility(
            0.01,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            0.05,
            -1.0,
        );

        let err = result.expect_err("expected error for negative time to expiry");
        assert!(
            err.to_string().contains("Invalid time to expiry"),
            "Expected InvalidTimeToExpiry error, got: {err}"
        );
    }

    #[test]
    fn test_same_convention_returns_unchanged() {
        let vol = 0.15;
        let forward = 0.05;

        // Normal -> Normal
        let result = convert_atm_volatility(
            vol,
            VolatilityConvention::Normal,
            VolatilityConvention::Normal,
            forward,
            1.0,
        )
        .expect("should succeed");
        assert_eq!(result, vol);

        // Lognormal -> Lognormal
        let result = convert_atm_volatility(
            vol,
            VolatilityConvention::Lognormal,
            VolatilityConvention::Lognormal,
            forward,
            1.0,
        )
        .expect("should succeed");
        assert_eq!(result, vol);

        // ShiftedLognormal -> ShiftedLognormal (same shift)
        let result = convert_atm_volatility(
            vol,
            VolatilityConvention::ShiftedLognormal { shift: 0.02 },
            VolatilityConvention::ShiftedLognormal { shift: 0.02 },
            forward,
            1.0,
        )
        .expect("should succeed");
        assert_eq!(result, vol);
    }

    #[test]
    fn test_different_shifts_conversion() {
        let forward = 0.03;
        let vol = 0.20;
        let shift1 = 0.01;
        let shift2 = 0.03;

        let converted = convert_atm_volatility(
            vol,
            VolatilityConvention::ShiftedLognormal { shift: shift1 },
            VolatilityConvention::ShiftedLognormal { shift: shift2 },
            forward,
            1.0,
        )
        .expect("conversion should succeed");

        assert_price_matched(
            VolatilityConvention::ShiftedLognormal { shift: shift1 },
            VolatilityConvention::ShiftedLognormal { shift: shift2 },
            forward,
            1.0,
            vol,
            converted,
        );

        // Round trip
        let recovered = convert_atm_volatility(
            converted,
            VolatilityConvention::ShiftedLognormal { shift: shift2 },
            VolatilityConvention::ShiftedLognormal { shift: shift1 },
            forward,
            1.0,
        )
        .expect("conversion should succeed");

        assert!(
            (recovered - vol).abs() < 1e-8,
            "Round trip failed: got {recovered}, expected {vol}"
        );
    }

    #[test]
    fn test_zero_time_to_expiry() {
        // At T=0, all volatilities should convert to themselves
        // (intrinsic value is the same regardless of vol convention)
        let vol = 0.15;
        let forward = 0.05;

        let result = convert_atm_volatility(
            vol,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            0.0,
        )
        .expect("should succeed");

        // At expiry, there is no unique price-matching volatility, so we preserve
        // the caller's quote deterministically.
        assert_eq!(result, vol);
    }

    #[test]
    fn test_representative_atm_cases() {
        // Test cases representing typical market scenarios

        // Case 1: Low rate environment
        let normal_vol = 0.005; // 50bp normal vol
        let forward = 0.01; // 1% forward
        let t = 1.0;

        let result = convert_atm_volatility(
            normal_vol,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            t,
        )
        .expect("should succeed");

        assert_price_matched(
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            t,
            normal_vol,
            result,
        );

        // Case 2: Higher rate environment
        let normal_vol = 0.012; // 120bp normal vol
        let forward = 0.04; // 4% forward
        let t = 1.0;
        let result = convert_atm_volatility(
            normal_vol,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            t,
        )
        .expect("should succeed");
        assert_price_matched(
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            t,
            normal_vol,
            result,
        );

        // Case 3: Longer expiry
        let normal_vol = 0.008;
        let forward = 0.03;

        let ln_1y = convert_atm_volatility(
            normal_vol,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            1.0,
        )
        .expect("should succeed");

        let ln_5y = convert_atm_volatility(
            normal_vol,
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            5.0,
        )
        .expect("should succeed");

        // Both should price-match for their respective expiries.
        assert_price_matched(
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            1.0,
            normal_vol,
            ln_1y,
        );
        assert_price_matched(
            VolatilityConvention::Normal,
            VolatilityConvention::Lognormal,
            forward,
            5.0,
            normal_vol,
            ln_5y,
        );
    }
}
