//! Signed empirical and additive model tail estimates on investment returns.
//!
//! Historical estimates use R-7 VaR and exact fractional empirical ES. Gaussian
//! and classical plug-in Cornish–Fisher quantiles approximate additive sums;
//! their finite outputs may lie below the investment-return lower bound -1.

use finstack_quant_core::math::stats::{empirical_tail_mean, quantile_checked, quantile_ratio};
use finstack_quant_core::{Error, Result};
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};

const MAX_HORIZON: u64 = (1_u64 << 53) - 1;

fn calculation(message: &str) -> Error {
    Error::Internal(format!("Tail calculation failed: {message}"))
}

fn validate_confidence(confidence: f64) -> Result<()> {
    if confidence.is_finite() && confidence > 0.5 && confidence < 1.0 {
        Ok(())
    } else {
        Err(Error::Validation(
            "confidence must be finite and strictly between 0.5 and 1".into(),
        ))
    }
}

fn validate_horizon(horizon_periods: Option<u64>) -> Result<f64> {
    let count = horizon_periods.unwrap_or(1);
    if count == 0 || count > MAX_HORIZON {
        return Err(Error::Validation(
            "horizon_periods must be an integer in 1..=2^53-1".into(),
        ));
    }
    Ok(count as f64)
}

fn historical_sample(returns: &[f64], confidence: f64) -> Result<Vec<f64>> {
    validate_confidence(confidence)?;
    crate::returns::validate_history(returns)?;
    let mut data = returns.to_vec();
    data.sort_unstable_by(f64::total_cmp);
    Ok(data)
}

/// Exact power-of-two normalization, including subnormal observations.
fn normalized(value: f64) -> (f64, i32) {
    if value == 0.0 {
        return (0.0, 0);
    }
    let magnitude = value.abs();
    let raw_exponent = ((magnitude.to_bits() >> 52) & 0x7ff) as i32;
    if raw_exponent == 0 {
        let (mantissa, exponent) = normalized(value * 2_f64.powi(1022));
        return (mantissa, exponent - 1022);
    }
    let exponent = raw_exponent - 1023;
    (value / 2_f64.powi(exponent), exponent)
}

fn expected_shortfall_from_sorted(data: &[f64], confidence: f64) -> Result<f64> {
    if data.is_empty() {
        return Ok(f64::NAN);
    }
    empirical_tail_mean(data, 1.0 - confidence)
}

/// Signed native-period R-7 historical lower quantile.
///
/// # Arguments
/// * `returns` - Chronological investment simple decimal returns, with -1 only terminal.
/// * `confidence` - Explicit finite tail confidence strictly between 0.5 and 1.
///
/// # Returns
/// Undefined for empty history; otherwise the signed interpolated observation.
/// Invalid inputs or unrepresentable requested estimates return an error.
pub(crate) fn value_at_risk(returns: &[f64], confidence: f64) -> Result<f64> {
    let data = historical_sample(returns, confidence)?;
    if data.is_empty() {
        return Ok(f64::NAN);
    }
    quantile_checked(&data, 1.0 - confidence)
}

/// Signed mean of exactly the worst empirical tail mass, including its boundary fraction.
///
/// # Arguments
/// * `returns` - Chronological investment simple decimal returns, with -1 only terminal.
/// * `confidence` - Explicit finite tail confidence strictly between 0.5 and 1.
///
/// # Returns
/// Undefined for empty history; invalid requests or numerical failures return an error.
pub(crate) fn expected_shortfall(returns: &[f64], confidence: f64) -> Result<f64> {
    let data = historical_sample(returns, confidence)?;
    expected_shortfall_from_sorted(&data, confidence)
}

/// Evaluate both selected historical estimators on one sorted active sample.
///
/// # Arguments
/// * `returns` - Chronological investment simple decimal returns, with -1 only terminal.
/// * `confidence` - Explicit finite tail confidence strictly between 0.5 and 1.
///
/// # Returns
/// Signed R-7 VaR and fractional empirical ES; empty histories yield two NaNs.
pub(crate) fn value_at_risk_and_es(returns: &[f64], confidence: f64) -> Result<(f64, f64)> {
    let data = historical_sample(returns, confidence)?;
    if data.is_empty() {
        return Ok((f64::NAN, f64::NAN));
    }
    let var = quantile_checked(&data, 1.0 - confidence)?;
    let es = expected_shortfall_from_sorted(&data, confidence)?;
    if es > var {
        return Err(calculation("empirical ES exceeds the paired R-7 quantile"));
    }
    Ok((var, es))
}

/// Unannualized magnitude ratio of upper and lower R-7 quantiles.
///
/// # Arguments
/// * `returns` - Chronological investment simple decimal returns, with -1 only terminal.
/// * `confidence` - Explicit finite tail confidence strictly between 0.5 and 1.
///
/// # Returns
/// Undefined for empty history or two zero quantiles; positive infinity for a
/// genuinely zero lower quantile and nonzero upper quantile. Numerical failures error.
pub(crate) fn tail_ratio(returns: &[f64], confidence: f64) -> Result<f64> {
    let data = historical_sample(returns, confidence)?;
    if data.is_empty() {
        return Ok(f64::NAN);
    }
    quantile_ratio(&data, confidence, 1.0 - confidence)
}

struct Moments {
    exponent: i32,
    mean: BigRational,
    sd: f64,
    skew: Option<BigRational>,
    kurt: Option<BigRational>,
}

/// Dimensional quantities remain normalized until a requested final estimate is formed.
fn moments(returns: &[f64], shape_order: u8) -> Result<Moments> {
    let magnitude = returns.iter().map(|x| x.abs()).fold(0.0, f64::max);
    let (_, exponent) = normalized(magnitude);
    let exponent = exponent.max(-1022);
    let scale = BigRational::from_integer(2.into()).pow(exponent);
    let data: Vec<BigRational> = returns
        .iter()
        .map(|&x| rational(x).map(|x| x / &scale))
        .collect::<Result<_>>()?;
    let count = BigRational::from_integer(returns.len().into());
    let one = BigRational::from_integer(1.into());
    let two = BigRational::from_integer(2.into());
    let three = BigRational::from_integer(3.into());
    let sum: BigRational = data.iter().cloned().sum();
    let mean = sum / &count;
    let deviations: Vec<BigRational> = data.iter().map(|x| x - &mean).collect();
    let max_deviation = deviations
        .iter()
        .map(Signed::abs)
        .max()
        .unwrap_or_else(BigRational::zero);
    if max_deviation.is_zero() {
        return Ok(Moments {
            exponent,
            mean,
            sd: 0.0,
            skew: None,
            kurt: None,
        });
    }
    // Normalize before reconstructing the bounded dimensional moment. Exact
    // source differences and centered powers avoid both false point masses at
    // high offsets and lost cancellation signs in requested shape estimates.
    let mut second = BigRational::zero();
    let mut third = BigRational::zero();
    let mut fourth = BigRational::zero();
    for deviation in deviations {
        let delta = deviation / &max_deviation;
        let square = &delta * &delta;
        second += &square;
        if shape_order >= 3 {
            third += &square * &delta;
        }
        if shape_order >= 4 {
            fourth += &square * &square;
        }
    }
    let max_deviation = rational_output(&max_deviation)?;
    let variance = rational_output(&(&second / (&count - &one)))?;
    let sd = max_deviation * variance.sqrt();
    if !sd.is_finite() || sd <= 0.0 {
        return Err(calculation("sample dispersion could not be resolved"));
    }
    let skew = if shape_order >= 3 {
        let second_sqrt = rational_output(&second)?.sqrt();
        let n = returns.len() as f64;
        let adjustment = n * (n - 1.0).sqrt() / (n - 2.0);
        Some(third * rational(adjustment)? / (&second * rational(second_sqrt)?))
    } else {
        None
    };
    let kurt = if shape_order >= 4 {
        let ratio = &count * fourth / (&second * &second);
        Some(
            (&count - &one)
                * ((&count + &one) * (ratio - &three) + BigRational::from_integer(6.into()))
                / ((&count - &two) * (&count - &three)),
        )
    } else {
        None
    };
    Ok(Moments {
        exponent,
        mean,
        sd,
        skew,
        kurt,
    })
}

/// Fisher-adjusted Type 2 sample skewness.
///
/// # Arguments
/// * `returns` - Chronological investment simple decimal returns, with -1 only terminal.
///
/// # Returns
/// Dimensionless shape, undefined below three observations or for constant history.
/// Invalid inputs and numerical failures return an error.
pub(crate) fn skewness(returns: &[f64]) -> Result<f64> {
    crate::returns::validate_history(returns)?;
    if returns.len() < 3 {
        return Ok(f64::NAN);
    }
    moments(returns, 3)?
        .skew
        .as_ref()
        .map_or(Ok(f64::NAN), rational_output)
}

/// Fisher-adjusted Type 2 excess kurtosis, with zero as the normal reference.
///
/// # Arguments
/// * `returns` - Chronological investment simple decimal returns, with -1 only terminal.
///
/// # Returns
/// Dimensionless excess shape, undefined below four observations or for constant history.
/// Invalid inputs and numerical failures return an error.
pub(crate) fn kurtosis(returns: &[f64]) -> Result<f64> {
    crate::returns::validate_history(returns)?;
    if returns.len() < 4 {
        return Ok(f64::NAN);
    }
    moments(returns, 4)?
        .kurt
        .as_ref()
        .map_or(Ok(f64::NAN), rational_output)
}

fn rational(value: f64) -> Result<BigRational> {
    BigRational::from_float(value).ok_or_else(|| calculation("nonfinite rational operand"))
}

fn rational_output(value: &BigRational) -> Result<f64> {
    let rounded = value
        .to_f64()
        .ok_or_else(|| calculation("requested output cannot be reconstructed"))?;
    if !rounded.is_finite() || rounded == 0.0 && !value.is_zero() {
        return Err(calculation("requested nonzero output is not representable"));
    }
    Ok(rounded)
}

fn horizon_quantile(moment: &Moments, z: &BigRational, horizon: f64) -> Result<f64> {
    let mean = &moment.mean * rational(horizon)?;
    let volatility = rational(moment.sd)? * rational(horizon.sqrt())? * z;
    let scale = BigRational::from_integer(2.into()).pow(moment.exponent);
    rational_output(&((mean + volatility) * scale))
}

/// Gaussian signed quantile for an additive whole-observation-period horizon.
///
/// # Arguments
/// * `returns` - Chronological investment simple decimal returns, with -1 only terminal.
/// * `confidence` - Explicit finite tail confidence strictly between 0.5 and 1.
/// * `horizon_periods` - Whole periods in 1..=2^53-1; omitted means one, independent of dates/frequency.
///
/// # Returns
/// Undefined below two observations; constant history produces its additive point mass.
/// Finite estimates may be below -1. Invalid requests and unrepresentable outputs error.
pub(crate) fn parametric_var(
    returns: &[f64],
    confidence: f64,
    horizon_periods: Option<u64>,
) -> Result<f64> {
    validate_confidence(confidence)?;
    let horizon = validate_horizon(horizon_periods)?;
    crate::returns::validate_history(returns)?;
    if returns.len() < 2 {
        return Ok(f64::NAN);
    }
    let moment = moments(returns, 2)?;
    let z = finstack_quant_core::math::special_functions::standard_normal_inv_cdf(1.0 - confidence);
    horizon_quantile(&moment, &rational(z)?, horizon)
}

fn cf_polynomial(
    z: f64,
    skew: &BigRational,
    kurt: &BigRational,
    horizon: f64,
) -> Result<BigRational> {
    // Use the exact supplied binary coefficients and exact whole-period h.
    // Keeping the sqrt(h) operand rational also preserves consequential
    // scaling residues; no coefficient is rounded away before the gate.
    let skew = skew / rational(horizon.sqrt())?;
    let kurt = kurt / rational(horizon)?;
    let square = &skew * &skew;
    // Integer-scaled derivative: 72*g'(z) = A*z² + B*z + D.
    let a = &kurt * BigRational::from_integer(9.into())
        - &square * BigRational::from_integer(12.into());
    let b = &skew * BigRational::from_integer(24.into());
    let d = BigRational::from_integer(72.into()) - &kurt * BigRational::from_integer(9.into())
        + square * BigRational::from_integer(10.into());
    let discriminant = &a * &d * BigRational::from_integer(4.into()) - &b * &b;
    let zero = BigRational::zero();
    let identity = skew.is_zero() && kurt.is_zero();
    if !identity && (a <= zero || discriminant < zero) {
        return Err(Error::Validation(
            "Cornish–Fisher quantile curve is not globally nondecreasing".into(),
        ));
    }
    let z = rational(z)?;
    // Exact Horner evaluation retains isolated flat points, including z³/3
    // for S=0,K=8, and leaves final representability to the requested Q.
    let cubic = a / BigRational::from_integer(3.into());
    let quadratic = &skew * BigRational::from_integer(12.into());
    let constant = -skew * BigRational::from_integer(12.into());
    Ok((((cubic * &z + quadratic) * &z + d) * z + constant) / BigRational::from_integer(72.into()))
}

/// Classical plug-in CF quantile with an analytic global ordering gate.
///
/// # Arguments
/// * `returns` - Chronological investment simple decimal returns, with -1 only terminal.
/// * `confidence` - Explicit finite tail confidence strictly between 0.5 and 1.
/// * `horizon_periods` - Whole periods in 1..=2^53-1; omitted means one.
///
/// # Returns
/// Undefined below four observations or for constant history. Invalid CF curves
/// and numerical failures error explicitly without estimator substitution. Valid
/// finite outputs use additive return units and may be below -1.
pub(crate) fn cornish_fisher_var(
    returns: &[f64],
    confidence: f64,
    horizon_periods: Option<u64>,
) -> Result<f64> {
    validate_confidence(confidence)?;
    let horizon = validate_horizon(horizon_periods)?;
    crate::returns::validate_history(returns)?;
    if returns.len() < 4 {
        return Ok(f64::NAN);
    }
    let moment = moments(returns, 4)?;
    if moment.sd == 0.0 {
        return Ok(f64::NAN);
    }
    let (Some(skew), Some(kurt)) = (&moment.skew, &moment.kurt) else {
        return Err(calculation(
            "required CF moments are unavailable after admission",
        ));
    };
    let z = finstack_quant_core::math::special_functions::standard_normal_inv_cdf(1.0 - confidence);
    horizon_quantile(&moment, &cf_polynomial(z, skew, kurt, horizon)?, horizon)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cf_value(z: f64, skew: f64, kurt: f64) -> Result<f64> {
        rational_output(&cf_polynomial(z, &rational(skew)?, &rational(kurt)?, 1.0)?)
    }

    #[test]
    fn historical_pair_uses_distinct_accepted_estimators() {
        let mut sample = vec![0.0; 30];
        sample[0] = -0.2;
        sample[1] = -0.1;
        let (var, es) = value_at_risk_and_es(&sample, 0.95).expect("valid sample");
        assert!((var + 0.055).abs() < 1e-14);
        assert!((es + 1.0 / 6.0).abs() < 1e-14);
        assert!(es <= var);
        assert_eq!(value_at_risk(&[0.01], 0.95).expect("singleton"), 0.01);
    }

    #[test]
    fn requests_are_validated_before_availability() {
        for confidence in [f64::NAN, 0.5, 0.1, 1.0] {
            assert!(value_at_risk(&[], confidence).is_err());
            assert!(parametric_var(&[], confidence, None).is_err());
            assert!(cornish_fisher_var(&[], confidence, None).is_err());
        }
        for horizon in [0, 1_u64 << 53, u64::MAX] {
            assert!(parametric_var(&[], 0.95, Some(horizon)).is_err());
            assert!(cornish_fisher_var(&[], 0.95, Some(horizon)).is_err());
        }
    }

    #[test]
    fn unavailable_shape_is_distinct_from_genuine_zero_shape() {
        assert!(skewness(&[1.0, 2.0]).expect("short").is_nan());
        assert!(kurtosis(&[1.0; 4]).expect("constant").is_nan());
        assert!(
            cornish_fisher_var(&[1.0; 4], 0.95, None)
                .expect("constant")
                .is_nan()
        );
        for exponent in [-600, 0, 600] {
            let scale = 2_f64.powi(exponent);
            let sample = [0.0, scale, 2.0 * scale, 3.0 * scale];
            assert_eq!(skewness(&sample).expect("symmetric"), 0.0);
            assert!((kurtosis(&sample).expect("scaled") + 1.2).abs() < 1e-13);
        }
    }

    #[test]
    fn gaussian_constant_and_model_support_are_preserved() {
        assert_eq!(
            parametric_var(&[-0.75, -0.75], 0.95, Some(2)).expect("point mass"),
            -1.5
        );
        assert!(parametric_var(&[0.01], 0.95, None).expect("short").is_nan());
    }

    #[test]
    fn cf_gate_is_global_and_allows_isolated_flat_points() {
        assert!(cf_value(-1.644853626951, 0.0, 10.0).is_err());
        let z = -1e-10;
        let expected = z * z * z / 3.0;
        let actual = cf_value(z, 0.0, 8.0).expect("valid flat point");
        assert!((actual / expected - 1.0).abs() < 1e-15);
        assert_eq!(cf_value(-2.0, 0.0, 0.0).expect("Gaussian identity"), -2.0);
        let mut sample = vec![-0.01; 80];
        sample.extend(vec![0.05; 20]);
        assert!(cornish_fisher_var(&sample, 0.95, None).is_err());
    }

    #[test]
    fn subnormal_joint_ratio_avoids_component_rounding() {
        let u = f64::from_bits(1);
        assert!(value_at_risk(&[0.0, u], 0.75).is_err());
        assert_eq!(expected_shortfall(&[0.0, u], 0.75).expect("zero ES"), 0.0);
        assert_eq!(tail_ratio(&[0.0, u], 0.75).expect("joint ratio"), 3.0);
        assert_eq!(
            expected_shortfall(&[u; 5], 0.6).expect("constant subnormal"),
            u
        );
    }

    #[test]
    fn joint_ratio_preserves_a_consequential_subnormal_midpoint_residual() {
        let tiny = f64::from_bits(1);
        let sample = [-1.0 / 3.0, -tiny, 0.0, -1.0];
        assert_eq!(tail_ratio(&sample, 0.875).expect("exact joint ratio"), tiny);
    }

    #[test]
    fn skewness_preserves_small_centering_residuals() {
        let tiny = 1e-20;
        let actual = skewness(&[1.0, tiny, -1.0]).expect("terminal valid sample");
        assert!((actual / (-3.0 * tiny) - 1.0).abs() < 1e-14);
    }

    #[test]
    fn moments_resolve_half_ulp_centers_at_a_large_offset() {
        let epsilon = f64::EPSILON;
        let sample = [1.0, 1.0 + epsilon, 1.0 + 2.0 * epsilon, 1.0 + 3.0 * epsilon];
        assert_eq!(skewness(&sample).expect("symmetric adjacent values"), 0.0);
        assert!((kurtosis(&sample).expect("adjacent values") + 1.2).abs() < 1e-13);
    }

    #[test]
    fn valid_cf_matches_the_selected_polynomial() {
        let sample = [-0.2, 0.2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let z = finstack_quant_core::math::special_functions::standard_normal_inv_cdf(0.05);
        let kurt = 4.5;
        let sd = (0.08_f64 / 9.0).sqrt();
        let expected = sd * (z + (z * z * z - 3.0 * z) * kurt / 24.0);
        let actual = cornish_fisher_var(&sample, 0.95, None).expect("valid global CF curve");
        assert!((actual - expected).abs() < 1e-14);
    }

    #[test]
    fn terminal_history_is_checked_before_tail_sorting() {
        assert!(value_at_risk(&[-1.0, 0.01], 0.95).is_err());
        assert!(value_at_risk(&[0.01, -1.0], 0.95).is_ok());
        assert!(skewness(&[f64::INFINITY; 4]).is_err());
    }
}
