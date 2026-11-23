// The continued fraction and inverse-beta refinement are adapted from statrs
// 0.18.0, whose inverse algorithm credits special 0.8.1.
//
// Copyright (c) 2016 Michael Ma
// Copyright 2014–2019 The special Developers
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is furnished
// to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
// WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use crate::{Error, Result};
use std::f64::consts::LN_2;

const HALF_LOG_PI: f64 = 0.572_364_942_924_700_1;
const FRACTION_TOLERANCE: f64 = f64::EPSILON / 2.0;
const INVERSE_LOG_RESIDUAL_TOLERANCE: f64 = 1e-8;
const MIN_DENOMINATOR: f64 = f64::MIN_POSITIVE / FRACTION_TOLERANCE;

/// Student-t cumulative distribution function.
///
/// Computes the CDF of the Student-t distribution with the specified degrees of freedom.
///
/// # Definition
///
/// ```text
/// F(x; ν) = P(T ≤ x) where T ~ t(ν)
/// ```
///
/// # Arguments
///
/// * `x` - Input value (any real number)
/// * `df` - Degrees of freedom (must be > 0)
///
/// # Returns
///
/// Cumulative probability F(x; df) ∈ (0, 1)
///
/// # Use Cases
///
/// - **Copula models**: Student-t copula tail dependence calculations
/// - **Credit modeling**: Heavy-tailed default correlation
/// - **Risk metrics**: VaR with fat tails
/// - **Statistical tests**: t-tests, confidence intervals
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::special_functions::student_t_cdf;
///
/// // CDF at zero should be 0.5 (symmetric distribution)
/// assert!((student_t_cdf(0.0, 5.0).expect("valid checked inputs") - 0.5).abs() < 1e-6);
///
/// // High df approaches normal distribution
/// let t_cdf = student_t_cdf(1.96, 1000.0).expect("valid checked inputs");
/// assert!((t_cdf - 0.975).abs() < 0.01);
/// ```
///
/// # Implementation
///
/// Uses a bounded regularized incomplete beta fraction.
///
/// # Errors
/// Returns a validation error for NaN `x` or non-finite or nonpositive `df`.
/// Returns a computation error if the tail cannot be evaluated.
pub fn student_t_cdf(x: f64, df: f64) -> Result<f64> {
    if x.is_nan() || !df.is_finite() || df <= 0.0 {
        return Err(Error::Validation(
            "Student-t CDF requires a non-NaN variate and finite positive degrees of freedom df"
                .into(),
        ));
    }
    if x == 0.0 {
        return Ok(0.5);
    }
    if x.is_infinite() {
        return Ok(f64::from(x.is_sign_positive()));
    }
    let shape = checked_shape(df)?;
    let log_odds = 2.0 * x.abs().ln() - df.ln();
    let log_beta_x = -softplus(log_odds);
    let log_beta_complement = -softplus(-log_odds);
    // Preserve the scaled logarithm even when the complementary argument
    // underflows before multiplication by a large shape.
    let scaled_log_beta_x = if log_odds < -700.0 {
        -(shape.ln() + log_odds).exp()
    } else {
        shape * log_beta_x
    };
    let log_front = scaled_log_beta_x + 0.5 * log_beta_complement - ln_beta_half(shape)?;
    let log_mass = log_regularized_beta(shape, 0.5, log_beta_x, log_beta_complement, log_front)?;
    let tail = (log_mass - LN_2).exp();
    Ok(if x < 0.0 { tail } else { 1.0 - tail })
}

/// Inverse Student-t cumulative distribution function.
///
/// Computes the inverse CDF (quantile function) of the Student-t distribution.
///
/// # Arguments
///
/// * `p` - Probability in `[0, 1]`
/// * `df` - Degrees of freedom (must be finite and > 0)
///
/// # Returns
///
/// x such that F(x; df) = p (`-∞` at `p = 0.0`, `+∞` at `p = 1.0`).
///
/// # Domain-edge convention
///
/// Exact boundary probabilities `p == 0.0` and `p == 1.0` return
/// `Ok(f64::NEG_INFINITY)` and `Ok(f64::INFINITY)`. Strictly out-of-domain or
/// non-finite probabilities (`p < 0.0`, `p > 1.0`, `NaN`) return a validation
/// error instead of silently saturating.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::special_functions::{student_t_inv_cdf, student_t_cdf};
///
/// // Inverse of median should be zero
/// assert!((student_t_inv_cdf(0.5, 5.0).expect("valid checked inputs") - 0.0).abs() < 1e-6);
///
/// // Round-trip test
/// let x = student_t_inv_cdf(0.95, 10.0).expect("valid checked inputs");
/// let p_back = student_t_cdf(x, 10.0).expect("valid checked inputs");
/// assert!((p_back - 0.95).abs() < 1e-6);
/// ```
///
/// # Implementation
///
/// Uses bounded inverse regularized incomplete beta refinement.
///
/// # Errors
/// Returns a validation error for non-finite or nonpositive `df` or for `p`
/// outside `[0, 1]`. Returns a computation error if the interior quantile
/// cannot be resolved or represented.
pub fn student_t_inv_cdf(p: f64, df: f64) -> Result<f64> {
    if !(0.0..=1.0).contains(&p) || !df.is_finite() || df <= 0.0 {
        return Err(Error::Validation(
            "Student-t quantile requires probability p in [0, 1] and finite positive degrees of freedom df".into(),
        ));
    }
    if p == 0.0 {
        return Ok(f64::NEG_INFINITY);
    }
    if p.to_bits() == 1.0_f64.to_bits() {
        return Ok(f64::INFINITY);
    }
    if p.to_bits() == 0.5_f64.to_bits() {
        return Ok(0.0);
    }
    let shape = checked_shape(df)?;
    let target = 2.0 * p.min(1.0 - p);
    let (a, b, probability) = if target > 0.5 {
        (0.5, shape, 1.0 - target)
    } else {
        (shape, 0.5, target)
    };
    let normalization = ln_beta_half(shape)?;
    let coordinate = inverse_beta(a, b, probability, normalization)?;
    if !(0.0 < coordinate && coordinate < 1.0) {
        return Err(computation_error("unresolved inverse beta coordinate"));
    }
    let log_x = coordinate.ln();
    let log_complement = (-coordinate).ln_1p();
    let log_odds = log_complement - log_x;
    let log_odds = if target > 0.5 { -log_odds } else { log_odds };
    let quantile = (0.5 * (df.ln() + log_odds)).exp();
    if !quantile.is_finite() || quantile == 0.0 {
        return Err(computation_error(
            "quantile is outside the representable range",
        ));
    }
    Ok(if p < 0.5 { -quantile } else { quantile })
}

fn checked_shape(df: f64) -> Result<f64> {
    let shape = df / 2.0;
    if shape <= 0.0 {
        return Err(computation_error("degrees of freedom halve to zero"));
    }
    Ok(shape)
}

fn computation_error(message: &str) -> Error {
    Error::Internal(format!("Student-t distribution: {message}"))
}

fn softplus(x: f64) -> f64 {
    x.max(0.0) + (-x.abs()).exp().ln_1p()
}

fn ln_beta_half(a: f64) -> Result<f64> {
    use super::ln_gamma;

    let result = if a >= 1e4 {
        // Stirling expansion of ln Γ(a + 1/2) − ln Γ(a). The next
        // term after a^-3 is smaller than 1e-22 in this region.
        let inverse = a.recip();
        HALF_LOG_PI - 0.5 * a.ln() + inverse * (0.125 - inverse * inverse / 192.0)
    } else if a < 0.125 {
        ln_gamma(1.0 + a)? - a.ln() + HALF_LOG_PI - ln_gamma(a + 0.5)?
    } else {
        ln_gamma(a)? + HALF_LOG_PI - ln_gamma(a + 0.5)?
    };
    if !result.is_finite() {
        return Err(computation_error(
            "beta normalization could not be resolved",
        ));
    }
    Ok(result)
}

fn log_regularized_beta(
    a: f64,
    b: f64,
    log_x: f64,
    log_complement: f64,
    log_front: f64,
) -> Result<f64> {
    // Compare equivalent odds to preserve the threshold near either endpoint.
    let reflected = log_x - log_complement >= (a + 1.0).ln() - (b + 1.0).ln();
    let (a, b, log_coordinate, log_remaining) = if reflected {
        (b, a, log_complement, log_x)
    } else {
        (a, b, log_x, log_complement)
    };
    let x = log_coordinate.exp();
    if x > 0.5 {
        // The continued fraction forms coefficients near unity. Even when
        // its recurrence terminates, rounding the complement in x can
        // distort those coefficients and the resulting tail.
        let complement = log_remaining.exp();
        let complement_error = ((1.0 - x) - complement).abs();
        if complement_error > 1e-9 * complement
            || f64::EPSILON > 1e-9 * (complement + (a + 1.0).recip())
        {
            return Err(computation_error(
                "incomplete beta complement could not be resolved",
            ));
        }
    }
    let log_mass = log_front + beta_fraction(a, b, x)?.ln() - a.ln();
    if !log_mass.is_finite() || log_mass > 0.0 {
        return Err(computation_error("invalid incomplete beta mass"));
    }
    if !reflected {
        return Ok(log_mass);
    }
    let complement = if log_mass > -LN_2 {
        (-log_mass.exp_m1()).ln()
    } else {
        (-log_mass.exp()).ln_1p()
    };
    if !complement.is_finite() {
        return Err(computation_error(
            "incomplete beta complement could not be resolved",
        ));
    }
    Ok(complement)
}

// Modified Lentz recurrence adapted from statrs 0.18 (MIT license).
fn beta_fraction(a: f64, b: f64, x: f64) -> Result<f64> {
    let mut c = 1.0;
    let mut d = protect_denominator(1.0 - (a + b) / (a + 1.0) * x).recip();
    let mut fraction = d;
    for index in 1..=140 {
        let m = f64::from(index);
        let double_m = 2.0 * m;
        let even = m / ((a - 1.0) + double_m) * ((b - m) * x / (a + double_m));
        fraction *= lentz_step(even, &mut c, &mut d);
        let odd = -(a + m) / (a + double_m) * (((a + b + m) * x) / (a + 1.0 + double_m));
        let update = lentz_step(odd, &mut c, &mut d);
        fraction *= update;
        if !fraction.is_finite() || fraction <= 0.0 {
            return Err(computation_error("invalid incomplete beta fraction"));
        }
        if (update - 1.0).abs() <= FRACTION_TOLERANCE {
            return Ok(fraction);
        }
    }
    Err(computation_error(
        "incomplete beta fraction did not converge",
    ))
}

fn protect_denominator(value: f64) -> f64 {
    if value.abs() < MIN_DENOMINATOR {
        MIN_DENOMINATOR.copysign(value)
    } else {
        value
    }
}

fn lentz_step(coefficient: f64, c: &mut f64, d: &mut f64) -> f64 {
    // Keep the pinned Lentz rounding profile. Fusing this update perturbs
    // the last-bit oscillation of the ratio even after effective convergence.
    *d = protect_denominator(1.0 + coefficient * *d).recip();
    *c = protect_denominator(1.0 + coefficient / *c);
    *d * *c
}

// Algorithms AS 64 and AS 109, adapted from statrs 0.18 (MIT) and
// special 0.8.1 (MIT, Copyright 2014–2019 The special Developers).
// The AS 64 branch alone applies here because one shape is always 1/2.
fn inverse_beta(a: f64, b: f64, target: f64, log_beta: f64) -> Result<f64> {
    let log_target = target.ln();
    let normal_scale = (-2.0 * log_target).sqrt();
    let normal = normal_scale
        - (2.30753 + 0.27061 * normal_scale)
            / (1.0 + (0.99229 + 0.04481 * normal_scale) * normal_scale);
    let scale = (b.recip()) / 9.0;
    let estimate = 2.0 * b * (1.0 - scale + normal * scale.sqrt()).powi(3);
    let mut coordinate = if estimate <= 0.0 {
        -(((-target).ln_1p() + b.ln() + log_beta) / b).exp_m1()
    } else {
        let ratio = (2.0 * a + (b - 1.0)) / (0.5 * estimate);
        if ratio <= 1.0 {
            ((log_target + a.ln() + log_beta) / a).exp()
        } else {
            1.0 - 2.0 / (ratio + 1.0)
        }
    }
    .clamp(0.0001, 0.9999);
    if !coordinate.is_finite() {
        return Err(computation_error(
            "invalid inverse beta starting coordinate",
        ));
    }

    // Work in adjustment units (rather than their squares).
    let accuracy_exponent = (-5.0 / a / a - target.powf(-0.2) - 13.0).trunc();
    let accuracy = 10.0_f64.powf(0.5 * accuracy_exponent.max(-30.0));
    let mut previous_correction = 0.0_f64;
    let mut previous_adjustment = 1.0_f64;
    let mut adjustment_bound = 1.0_f64;
    let mut work = 1024_u16;
    let mut passes_step_test = false;
    loop {
        consume_work(&mut work)?;
        let log_x = coordinate.ln();
        let log_complement = (-coordinate).ln_1p();
        let log_front = a.mul_add(log_x, b * log_complement) - log_beta;
        let log_mass = log_regularized_beta(a, b, log_x, log_complement, log_front)?;
        let delta = log_mass - log_target;
        // An absolute step can be small compared with the heuristic and still
        // be large relative to a representable near-endpoint solution.
        if passes_step_test && delta.abs() <= INVERSE_LOG_RESIDUAL_TOLERANCE {
            return Ok(coordinate);
        }
        let sign = if delta < 0.0 { -1.0 } else { 1.0 };
        let log_difference = log_mass.max(log_target) + (-(-delta.abs()).exp_m1()).ln();
        let correction = sign * (log_difference - log_front + log_x + log_complement).exp();
        if !correction.is_finite() {
            return Err(computation_error("invalid inverse beta Newton correction"));
        }
        if correction == 0.0
            || previous_correction == 0.0
            || correction.is_sign_negative() != previous_correction.is_sign_negative()
        {
            adjustment_bound = previous_adjustment.abs().max(1e-15);
        }
        let mut adjustment = correction;
        loop {
            consume_work(&mut work)?;
            let next = coordinate - adjustment;
            if adjustment.abs() < adjustment_bound && 0.0 < next && next < 1.0 {
                passes_step_test = adjustment_bound <= accuracy || correction.abs() <= accuracy;
                if next.to_bits() == coordinate.to_bits() {
                    if passes_step_test && delta.abs() <= INVERSE_LOG_RESIDUAL_TOLERANCE {
                        return Ok(coordinate);
                    }
                    return Err(computation_error("inverse beta stopped progressing"));
                }
                coordinate = next;
                previous_correction = correction;
                previous_adjustment = adjustment;
                break;
            }
            adjustment /= 3.0;
        }
    }
}

fn consume_work(work: &mut u16) -> Result<()> {
    *work = work
        .checked_sub(1)
        .ok_or_else(|| computation_error("inverse beta work limit exhausted"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn cauchy_tail_stays_positive_past_square_overflow() {
        let magnitude = 1e200_f64;
        let reference = magnitude.recip().atan() / PI;
        let actual = student_t_cdf(-magnitude, 1.0).expect("representable Cauchy tail");
        assert!((actual / reference - 1.0).abs() < 8e-14);
    }

    #[test]
    fn inverse_rejects_unresolved_tail() {
        assert!(student_t_inv_cdf(1e-12, 0.01).is_err());
    }

    #[test]
    fn inverse_refines_small_beta_coordinates_after_absolute_step_test() {
        let p = 0.499_999_95;
        let reference = (PI * (p - 0.5)).tan();
        let actual = student_t_inv_cdf(p, 1.0).expect("Cauchy quantile near the median");
        assert!((actual / reference - 1.0).abs() < 2e-8);
    }

    #[test]
    fn inverse_converges_for_monte_carlo_sample_sizes() {
        // Independent reference values from R's Student-t quantile.
        for (df, reference) in [
            (7_999.0, -1.960_260_599_632_775_8),
            (49_999.0, -1.960_011_432_042_652_5),
            (999_999.0, -1.959_966_356_816_479_1),
        ] {
            for (p, reference) in [(0.025, reference), (0.975, -reference)] {
                let quantile = student_t_inv_cdf(p, df).expect("finite Student-t CI quantile");
                let probability = student_t_cdf(quantile, df).expect("finite Student-t tail");
                assert!((quantile - reference).abs() < 1e-8);
                assert!((probability - p).abs() < 5e-10);
            }
        }
    }

    #[test]
    fn direct_beta_rejects_tail_geometry_lost_when_rounding_near_one() {
        assert!(student_t_cdf(-8.0, 1e18).is_err());
    }
}
