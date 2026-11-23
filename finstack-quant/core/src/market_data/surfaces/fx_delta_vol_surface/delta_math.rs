//! Checked FX wing volatility recovery and forward-delta-to-strike inversion.

use super::expansion::{exact_sum_scaled_to_f64, scaled_exp_product, two_prod};
use crate::{
    error::InputError,
    math::special_functions::{log_norm_cdf, standard_normal_inv_cdf},
    math::stats::{frexp, ldexp},
};

/// Recover 25-delta or 10-delta `(sigma_put, sigma_call)` from ATM DNS, RR, and smile BF quotes.
///
/// Uses power-of-2 scaling and exact compensated summation across `2 * (ATM + BF) ± RR`
/// followed by a single final rounding step so large cancellation or tiny valid wings
/// are preserved without subnormal half-RR distortion or intermediate rounding loss.
///
/// # Arguments
///
/// * `atm` - Positive finite ATM delta-neutral-straddle volatility (decimal).
/// * `rr` - Finite risk reversal quote (`sigma_call - sigma_put`, decimal).
/// * `bf` - Finite arithmetic smile butterfly quote (`0.5 * (sigma_call + sigma_put) - atm`, decimal).
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`] when `atm` or either recovered wing is `<= 0.0`
/// (including cancellation or exact subnormal underflow to zero), or [`crate::Error::Validation`]
/// when an input is non-finite or a positive recovered wing overflows binary64.
pub fn recover_fx_wing_vols(atm: f64, rr: f64, bf: f64) -> crate::Result<(f64, f64)> {
    if !atm.is_finite() || atm <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    if !rr.is_finite() || !bf.is_finite() {
        return Err(crate::Error::Validation(
            "FX risk reversal and butterfly quotes must be finite".to_string(),
        ));
    }

    let max_abs = atm.max(rr.abs()).max(bf.abs());
    let mut min_nz = atm;
    if rr != 0.0 {
        min_nz = min_nz.min(rr.abs());
    }
    if bf != 0.0 {
        min_nz = min_nz.min(bf.abs());
    }

    let in_shift = if max_abs > ldexp(1.0, 400) {
        -520
    } else if min_nz < ldexp(1.0, -900) {
        520
    } else {
        0
    };

    let atm_s = ldexp(atm, in_shift);
    let bf_s = ldexp(bf, in_shift);
    let rr_s = ldexp(rr, in_shift);

    // Multiplication by 2 is exact inside the scaled normal range.
    let two_atm = 2.0 * atm_s;
    let two_bf = 2.0 * bf_s;

    let sigma_put = exact_sum_scaled_to_f64(&[two_atm, two_bf, -rr_s], -in_shift - 1);
    let sigma_call = exact_sum_scaled_to_f64(&[two_atm, two_bf, rr_s], -in_shift - 1);

    if !sigma_put.is_finite() || !sigma_call.is_finite() {
        return Err(crate::Error::Validation(
            "recovered FX wing volatility overflows binary64".to_string(),
        ));
    }
    if sigma_put <= 0.0 || sigma_call <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    Ok((sigma_put, sigma_call))
}

/// Linearly interpolate `(atm, rr, bf)` quotes between two rows and reconstruct
/// `(atm, sigma_put, sigma_call)` with one final rounding per volatility.
///
/// Evaluates the interpolated affine combinations across all six unrounded
/// endpoint products so exact cancellation across `(ATM, BF, RR)` endpoints
/// does not lose valid positive wings through per-lane rounding.
///
/// # Arguments
///
/// * `w0` - Non-negative finite weight applied to `row0`.
/// * `row0` - First endpoint tuple `(atm, rr, bf)` in decimal volatility units.
/// * `w1` - Non-negative finite weight applied to `row1`.
/// * `row1` - Second endpoint tuple `(atm, rr, bf)` in decimal volatility units.
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`] when either row has `atm <= 0.0` or
/// any interpolated output is `<= 0.0`, or [`crate::Error::Validation`] when
/// weights or quotes are invalid or an output overflows binary64.
pub fn interpolate_fx_wing_vols(
    w0: f64,
    row0: (f64, f64, f64),
    w1: f64,
    row1: (f64, f64, f64),
) -> crate::Result<(f64, f64, f64)> {
    let (atm0, rr0, bf0) = row0;
    let (atm1, rr1, bf1) = row1;
    if !w0.is_finite() || !w1.is_finite() || w0 < 0.0 || w1 < 0.0 {
        return Err(crate::Error::Validation(
            "FX interpolation weights must be finite and non-negative".to_string(),
        ));
    }
    if !atm0.is_finite() || atm0 <= 0.0 || !atm1.is_finite() || atm1 <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    if !rr0.is_finite() || !bf0.is_finite() || !rr1.is_finite() || !bf1.is_finite() {
        return Err(crate::Error::Validation(
            "FX risk reversal and butterfly quotes must be finite".to_string(),
        ));
    }

    let mut min_nz_q = atm0.min(atm1);
    let mut max_q = atm0.max(atm1);
    for q in [rr0.abs(), bf0.abs(), rr1.abs(), bf1.abs()] {
        if q > max_q {
            max_q = q;
        }
        if q != 0.0 && q < min_nz_q {
            min_nz_q = q;
        }
    }
    let q_shift = if max_q > ldexp(1.0, 300) {
        -400
    } else if min_nz_q < ldexp(1.0, -800) && max_q < ldexp(1.0, 300) {
        400
    } else {
        0
    };

    let mut min_nz_w = f64::INFINITY;
    let max_w = w0.max(w1);
    for w in [w0, w1] {
        if w != 0.0 && w < min_nz_w {
            min_nz_w = w;
        }
    }
    let w_shift = if max_w > ldexp(1.0, 300) {
        -400
    } else if min_nz_w < ldexp(1.0, -800) && max_w < ldexp(1.0, 300) {
        400
    } else {
        0
    };

    let w0_s = ldexp(w0, w_shift);
    let w1_s = ldexp(w1, w_shift);
    let a0_s = ldexp(atm0, q_shift);
    let r0_s = ldexp(rr0, q_shift);
    let b0_s = ldexp(bf0, q_shift);
    let a1_s = ldexp(atm1, q_shift);
    let r1_s = ldexp(rr1, q_shift);
    let b1_s = ldexp(bf1, q_shift);

    let out_shift_atm = -q_shift - w_shift;
    let out_shift_wing = -q_shift - w_shift - 1;

    let (pa0, ea0) = two_prod(w0_s, a0_s);
    let (pa1, ea1) = two_prod(w1_s, a1_s);
    let atm = exact_sum_scaled_to_f64(&[pa0, ea0, pa1, ea1], out_shift_atm);

    let (p2a0, e2a0) = two_prod(w0_s, 2.0 * a0_s);
    let (p2b0, e2b0) = two_prod(w0_s, 2.0 * b0_s);
    let (pr0, er0) = two_prod(w0_s, r0_s);
    let (p2a1, e2a1) = two_prod(w1_s, 2.0 * a1_s);
    let (p2b1, e2b1) = two_prod(w1_s, 2.0 * b1_s);
    let (pr1, er1) = two_prod(w1_s, r1_s);

    let sigma_put = exact_sum_scaled_to_f64(
        &[
            p2a0, e2a0, p2b0, e2b0, -pr0, -er0, p2a1, e2a1, p2b1, e2b1, -pr1, -er1,
        ],
        out_shift_wing,
    );
    let sigma_call = exact_sum_scaled_to_f64(
        &[
            p2a0, e2a0, p2b0, e2b0, pr0, er0, p2a1, e2a1, p2b1, e2b1, pr1, er1,
        ],
        out_shift_wing,
    );

    if !atm.is_finite() || !sigma_put.is_finite() || !sigma_call.is_finite() {
        return Err(crate::Error::Validation(
            "interpolated FX volatility overflows binary64".to_string(),
        ));
    }
    if atm <= 0.0 || sigma_put <= 0.0 || sigma_call <= 0.0 {
        return Err(InputError::NonPositiveValue.into());
    }
    Ok((atm, sigma_put, sigma_call))
}

/// Compute `ln(num / den)` stably across extreme ratios without overflow or underflow.
///
/// # Arguments
///
/// * `num` - Finite strictly positive numerator.
/// * `den` - Finite strictly positive denominator.
#[must_use]
pub fn checked_ln_ratio(num: f64, den: f64) -> f64 {
    let ratio = num / den;
    if ratio.is_finite() && (0.5..=2.0).contains(&ratio) {
        return ((num - den) / den).ln_1p();
    }
    let (m_n, e_n) = frexp(num);
    let (m_d, e_d) = frexp(den);
    let exp_diff = f64::from(e_n - e_d);
    (m_n / m_d).ln() + exp_diff * std::f64::consts::LN_2
}

fn fx_atm_dns_strike_checked(forward: f64, vol: f64, expiry: f64) -> crate::Result<f64> {
    if !forward.is_finite()
        || forward <= 0.0
        || !vol.is_finite()
        || vol <= 0.0
        || !expiry.is_finite()
        || expiry <= 0.0
    {
        return Err(InputError::NonPositiveValue.into());
    }
    let std_dev = vol * expiry.sqrt();
    if !std_dev.is_finite() || std_dev <= 0.0 {
        return Err(crate::Error::Validation(
            "FX volatility standard deviation underflowed or overflowed".to_string(),
        ));
    }
    let half_var = 0.5 * std_dev * std_dev;
    if !half_var.is_finite() {
        return Err(crate::Error::Validation(
            "FX strike variance term overflowed binary64".to_string(),
        ));
    }
    scaled_exp_product(forward, half_var)
}

/// Convert an option side and absolute forward delta into a strike with relative tail verification.
///
/// Uses direct lower/upper tail inversion (`standard_normal_inv_cdf(p)` for `p <= 0.5` and
/// `-standard_normal_inv_cdf(1 - p)` for `p > 0.5`), scaled log/exp strike evaluation, and
/// checks the represented strike's Black forward delta in the smaller log-probability tail
/// against `delta_rel_tol`.
///
/// # Arguments
///
/// * `is_call` - `true` for a call option (`Delta_call = N(d1)`), `false` for a put (`|Delta_put| = N(-d1)`).
/// * `delta_abs` - Absolute forward delta strictly in the open unit interval `(0, 1)`.
/// * `forward` - Finite positive FX forward in quote currency per base unit.
/// * `vol` - Finite positive annualized Black volatility as a decimal.
/// * `expiry` - Finite positive time to expiry in years.
/// * `delta_rel_tol` - Finite strictly positive relative delta acceptance tolerance (for example `1e-3`).
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`] when `forward`, `vol`, or `expiry` is non-positive
/// or non-finite, or [`crate::Error::Validation`] when `delta_abs` is outside `(0, 1)`,
/// `delta_rel_tol` is invalid, the strike is non-representable, or floating-point rounding
/// causes the represented strike's delta to diverge beyond `delta_rel_tol`.
pub fn fx_option_delta_to_strike(
    is_call: bool,
    delta_abs: f64,
    forward: f64,
    vol: f64,
    expiry: f64,
    delta_rel_tol: f64,
) -> crate::Result<f64> {
    if !forward.is_finite()
        || forward <= 0.0
        || !vol.is_finite()
        || vol <= 0.0
        || !expiry.is_finite()
        || expiry <= 0.0
    {
        return Err(InputError::NonPositiveValue.into());
    }
    if !delta_abs.is_finite() || delta_abs <= 0.0 || delta_abs >= 1.0 {
        return Err(crate::Error::Validation(format!(
            "FX forward delta magnitude must lie strictly in (0, 1), got {delta_abs}"
        )));
    }
    if !delta_rel_tol.is_finite() || delta_rel_tol <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "FX delta relative tolerance must be finite and positive, got {delta_rel_tol}"
        )));
    }

    let std_dev = vol * expiry.sqrt();
    if !std_dev.is_finite() || std_dev <= 0.0 {
        return Err(crate::Error::Validation(
            "FX option standard deviation must be finite and positive".to_string(),
        ));
    }
    let half_var = 0.5 * std_dev * std_dev;
    if !half_var.is_finite() {
        return Err(crate::Error::Validation(
            "FX option variance term overflowed binary64".to_string(),
        ));
    }

    let z_q = if delta_abs <= 0.5 {
        standard_normal_inv_cdf(delta_abs)?
    } else {
        -standard_normal_inv_cdf(1.0 - delta_abs)?
    };
    if !z_q.is_finite() {
        return Err(crate::Error::Validation(
            "FX normal quantile is non-finite".to_string(),
        ));
    }
    let signed_z = if is_call { -z_q } else { z_q };
    let log_ratio = signed_z.mul_add(std_dev, half_var);
    let strike = scaled_exp_product(forward, log_ratio)?;

    let d1_actual = checked_ln_ratio(forward, strike) / std_dev + 0.5 * std_dev;
    if !d1_actual.is_finite() {
        return Err(crate::Error::Validation(
            "FX represented strike produces non-finite d1".to_string(),
        ));
    }
    let z_actual = if is_call { d1_actual } else { -d1_actual };
    let (log_p_target, log_p_actual) = if delta_abs <= 0.5 {
        (delta_abs.ln(), log_norm_cdf(z_actual)?)
    } else {
        ((1.0 - delta_abs).ln(), log_norm_cdf(-z_actual)?)
    };
    if !log_p_actual.is_finite() {
        return Err(crate::Error::Validation(
            "FX represented strike log-delta is non-finite".to_string(),
        ));
    }
    let rel_err = (log_p_actual - log_p_target).exp_m1().abs();
    if !rel_err.is_finite() || rel_err > delta_rel_tol {
        return Err(crate::Error::Validation(format!(
            "FX represented strike {strike} delta relative error {rel_err} exceeds tolerance {delta_rel_tol}"
        )));
    }
    Ok(strike)
}

/// Construct checked `(strikes, vols)` pillars for a single FX expiry smile and
/// verify strict ascending strike order `K_10P < K_25P < K_DNS < K_25C < K_10C`.
///
/// # Arguments
///
/// * `forward` - Finite positive FX forward in quote currency per base unit.
/// * `expiry` - Finite positive expiry time in years.
/// * `atm` - Positive finite ATM DNS volatility (decimal).
/// * `wings_25d` - Reconstructed 25-delta wing volatilities `(sigma_25p, sigma_25c)`.
/// * `wings_10d` - Optional reconstructed 10-delta wing volatilities `(sigma_10p, sigma_10c)`.
/// * `delta_rel_tol` - Relative delta acceptance tolerance for strike inversion.
///
/// # Errors
///
/// Returns [`InputError::NonPositiveValue`] or [`crate::Error::Validation`] if any
/// delta-to-strike conversion fails or if pillar strikes cross or collide.
pub fn fx_smile_pillars_checked(
    forward: f64,
    expiry: f64,
    atm: f64,
    wings_25d: (f64, f64),
    wings_10d: Option<(f64, f64)>,
    delta_rel_tol: f64,
) -> crate::Result<(Vec<f64>, Vec<f64>)> {
    let (sigma_put_25d, sigma_call_25d) = wings_25d;
    let k_atm = fx_atm_dns_strike_checked(forward, atm, expiry)?;
    let k_put_25d =
        fx_option_delta_to_strike(false, 0.25, forward, sigma_put_25d, expiry, delta_rel_tol)?;
    let k_call_25d =
        fx_option_delta_to_strike(true, 0.25, forward, sigma_call_25d, expiry, delta_rel_tol)?;

    let (strikes, vols) = if let Some((sigma_put_10d, sigma_call_10d)) = wings_10d {
        let k_put_10d =
            fx_option_delta_to_strike(false, 0.10, forward, sigma_put_10d, expiry, delta_rel_tol)?;
        let k_call_10d =
            fx_option_delta_to_strike(true, 0.10, forward, sigma_call_10d, expiry, delta_rel_tol)?;
        (
            vec![k_put_10d, k_put_25d, k_atm, k_call_25d, k_call_10d],
            vec![
                sigma_put_10d,
                sigma_put_25d,
                atm,
                sigma_call_25d,
                sigma_call_10d,
            ],
        )
    } else {
        (
            vec![k_put_25d, k_atm, k_call_25d],
            vec![sigma_put_25d, atm, sigma_call_25d],
        )
    };

    for pair in strikes.windows(2) {
        if pair[0].partial_cmp(&pair[1]) != Some(std::cmp::Ordering::Less) {
            return Err(crate::Error::Validation(format!(
                "FX smile pillar strikes must be strictly increasing across put, ATM, and call deltas (got {} followed by {})",
                pair[0], pair[1]
            )));
        }
    }
    Ok((strikes, vols))
}
