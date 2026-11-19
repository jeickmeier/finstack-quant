use crate::math::{solver::BrentSolver, summation::NeumaierAccumulator};
use crate::{Error, Result};
use std::f64::consts::{LN_2, TAU};

const MAX_TAIL_UPDATES: u32 = 100_000;
const TAIL_REL_TOL: f64 = 1e-15;

/// Return the central chi-squared quantile using a bounded logarithmic tail inversion.
///
/// # Arguments
///
/// * `p` - Finite cumulative probability in `[0, 1]`. The endpoints return
///   zero and positive infinity, respectively.
/// * `df` - Finite, strictly positive degrees of freedom. Fractional values
///   are supported. Interior probabilities are evaluated for `df <= 1e8`.
///
/// # Errors
///
/// Returns a validation error for an invalid input or unsupported interior
/// degrees of freedom. Returns a computation error if the tail evaluation
/// fails or the returned positive finite quantile cannot meet the logarithmic
/// tail residual tolerance of `1e-8`.
pub fn chi_squared_quantile(p: f64, df: f64) -> Result<f64> {
    if !(0.0..=1.0).contains(&p) || !df.is_finite() || df <= 0.0 {
        return Err(Error::Validation(format!(
            "Chi-squared requires p in [0, 1] and finite df > 0, got p={p}, df={df}"
        )));
    }
    if p == 0.0 {
        return Ok(0.0);
    }
    if p.to_bits() == 1.0_f64.to_bits() {
        return Ok(f64::INFINITY);
    }
    if df > 1e8 {
        return Err(Error::Validation(format!(
            "Chi-squared df={df} exceeds the interior evaluation limit of 1e8"
        )));
    }
    let shape = df / 2.0;
    if shape == 0.0 {
        return Err(tail_error("degrees of freedom halve to zero"));
    }
    let lower = p <= 0.5;
    let log_probability = if lower { p.ln() } else { (-p).ln_1p() };
    let residual = |log_x: f64| {
        let log_tail = gamma_log_tail(shape, log_x - LN_2, lower)?;
        Ok(if lower {
            log_tail - log_probability
        } else {
            log_probability - log_tail
        })
    };

    let smallest = f64::from_bits(1);
    let log_min = smallest.ln();
    let log_max = (df + 64.0 * df.sqrt() + 128.0).ln();
    let quantile = if residual(log_min)? >= 0.0 {
        smallest
    } else {
        BrentSolver::new(1e-13, 256)?
            .solve_in_bracket(residual, log_min, log_max)?
            .exp()
    };
    if quantile <= 0.0 || !quantile.is_finite() || residual(quantile.ln())?.abs() > 1e-8 {
        return Err(tail_error(
            "returned quantile cannot resolve the requested tail",
        ));
    }
    Ok(quantile)
}

fn tail_error(message: &str) -> Error {
    Error::Internal(format!("Chi-squared quantile: {message}"))
}

fn gamma_log_tail(shape: f64, log_x: f64, lower: bool) -> Result<f64> {
    let x = log_x.exp();
    let (log_mass, computed_lower) = if shape <= 1.0 && x <= 1.0 {
        let correction = small_gamma_correction(shape, x)?;
        (
            shape.mul_add(log_x, -log_gamma_one_plus(shape)?) + correction,
            true,
        )
    } else if x <= shape {
        (
            log_gamma_prefactor(shape, x, log_x)? + lower_gamma_series(shape, x)?.ln(),
            true,
        )
    } else {
        (
            log_gamma_prefactor(shape, x, log_x)?
                + shape.ln()
                + upper_gamma_fraction(shape, x)?.ln(),
            false,
        )
    };
    if !log_mass.is_finite() || log_mass > 0.0 {
        return Err(tail_error("invalid log-tail mass"));
    }
    if computed_lower == lower {
        return Ok(log_mass);
    }
    let complement = if log_mass > -LN_2 {
        (-log_mass.exp_m1()).ln()
    } else {
        (-log_mass.exp()).ln_1p()
    };
    if !complement.is_finite() {
        return Err(tail_error("complementary tail could not be resolved"));
    }
    Ok(complement)
}

fn small_gamma_correction(shape: f64, x: f64) -> Result<f64> {
    let mut sum = NeumaierAccumulator::new();
    let mut term = -1.0 / (shape + 1.0);
    sum.add(term)?;
    for k in 1..=MAX_TAIL_UPDATES {
        let k = f64::from(k);
        term *= -x * (shape + k) / ((k + 1.0) * (shape + k + 1.0));
        sum.add(term)?;
        let total = sum.get_total()?;
        if term.abs() <= TAIL_REL_TOL * total.abs() {
            return Ok((shape * (x * total)).ln_1p());
        }
    }
    Err(tail_error("small-shape correction did not converge"))
}

fn lower_gamma_series(shape: f64, x: f64) -> Result<f64> {
    let mut sum = NeumaierAccumulator::new();
    let mut term = 1.0;
    sum.add(term)?;
    for k in 1..=MAX_TAIL_UPDATES {
        term *= x / (shape + f64::from(k));
        sum.add(term)?;
        let total = sum.get_total()?;
        if term <= TAIL_REL_TOL * total {
            return Ok(total);
        }
    }
    Err(tail_error("lower Gamma series did not converge"))
}

// Continued-fraction recurrence adapted from statrs 0.18 (MIT license),
// function gamma::checked_gamma_ur. See ../special_functions_student_t.rs
// for the statrs copyright and permission notice. Keep both scales equal.
fn upper_gamma_fraction(shape: f64, x: f64) -> Result<f64> {
    const SCALE_DOWN: f64 = 1.0 / 4_503_599_627_370_496.0;
    let mut z = (x - shape) + 2.0;
    let mut num_previous = 1.0;
    let mut den_previous = x;
    let mut num = x + 1.0;
    let mut den = z * x;
    let mut ratio = num / den;
    for iteration in 1..=MAX_TAIL_UPDATES {
        let count = f64::from(iteration);
        z += 2.0;
        let coefficient = ((count + 1.0) - shape) * count;
        let next_num = z.mul_add(num, -coefficient * num_previous);
        let next_den = z.mul_add(den, -coefficient * den_previous);
        let next_ratio = next_num / next_den;
        if !next_ratio.is_finite() || next_ratio <= 0.0 {
            return Err(tail_error("invalid upper Gamma continued fraction ratio"));
        }
        if (ratio - next_ratio).abs() <= TAIL_REL_TOL * next_ratio {
            return Ok(next_ratio);
        }
        ratio = next_ratio;
        let scale = if next_num.abs().max(next_den.abs()) > 1.0 / SCALE_DOWN {
            SCALE_DOWN
        } else {
            1.0
        };
        num_previous = num * scale;
        den_previous = den * scale;
        num = next_num * scale;
        den = next_den * scale;
    }
    Err(tail_error(
        "upper Gamma continued fraction did not converge",
    ))
}

fn log_gamma_one_plus(shape: f64) -> Result<f64> {
    if shape > 0.125 {
        return crate::math::special_functions::ln_gamma(1.0 + shape);
    }
    // Coefficients (-1)^k zeta(k)/k of log Gamma(1 + shape), k = 2..16.
    const COEFFICIENTS: [f64; 15] = [
        0.822_467_033_424_113_2,
        -0.400_685_634_386_531_43,
        0.270_580_808_427_784_54,
        -0.207_385_551_028_673_98,
        0.169_557_176_997_408_2,
        -0.144_049_896_768_846_1,
        0.125_509_669_524_743_04,
        -0.111_334_265_869_564_69,
        0.100_099_457_512_781_8,
        -0.090_954_017_145_829_04,
        0.083_353_840_546_109,
        -0.076_932_516_411_352_2,
        0.071_432_946_295_361_33,
        -0.066_668_705_882_420_46,
        0.062_500_955_141_213_04,
    ];
    let polynomial = COEFFICIENTS
        .iter()
        .rev()
        .fold(0.0_f64, |acc, &value| acc.mul_add(shape, value));
    Ok(shape * polynomial.mul_add(shape, -0.577_215_664_901_532_9))
}

fn log_gamma_prefactor(shape: f64, x: f64, log_x: f64) -> Result<f64> {
    if shape < 16.0 {
        return Ok(shape.mul_add(log_x, -x) - log_gamma_one_plus(shape)?);
    }
    let inverse = shape.recip();
    let inverse_squared = inverse * inverse;
    let correction = inverse
        * (1.0_f64 / 1188.0)
            .mul_add(inverse_squared, -1.0 / 1680.0)
            .mul_add(inverse_squared, 1.0 / 1260.0)
            .mul_add(inverse_squared, -1.0 / 360.0)
            .mul_add(inverse_squared, 1.0 / 12.0);
    let r = (x - shape) / shape;
    let scaled_geometry = if r.abs() <= 0.5 {
        let mut sum = NeumaierAccumulator::new();
        let mut power = 1.0;
        for index in 0..64 {
            sum.add(power / f64::from(index + 2))?;
            power *= -r;
        }
        -(shape * r) * r * sum.get_total()?
    } else {
        shape.mul_add(log_x - shape.ln(), shape - x)
    };
    Ok(scaled_geometry - 0.5 * (TAU.ln() + shape.ln()) - correction)
}
