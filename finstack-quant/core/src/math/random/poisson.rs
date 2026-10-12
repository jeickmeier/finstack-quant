//! Poisson distribution sampling for jump processes.
//!
//! Provides methods to sample from Poisson distribution for modeling
//! jump arrivals in jump-diffusion processes.

use statrs::distribution::{DiscreteCDF, Poisson};

/// Intensity below which the quantile is found by sequential inversion of the
/// probability-mass recurrence; larger intensities use a bracketed search on
/// the statrs CDF.
const SEQUENTIAL_INVERSION_MAX_INTENSITY: f64 = 30.0;

/// Count beyond which sequential inversion hands over to the CDF search; for
/// `lambda < 30` the mass above it is below `1e-100`, so only `u` within
/// rounding of one gets there.
const SEQUENTIAL_INVERSION_MAX_COUNT: usize = 200;

/// Return the Poisson quantile corresponding to `u`.
///
/// Finds the smallest count `k` with `P(N <= k) >= u`. Intensities below 30
/// accumulate `P(N = k + 1) = P(N = k) * lambda / (k + 1)` from `exp(-lambda)`
/// (one exponential per draw, as Monte Carlo jump simulation needs); larger
/// intensities, and `u` within rounding of one, search the statrs CDF, using
/// the survival function in the upper tail to avoid subtracting a probability
/// close to one.
///
/// # References
///
/// - Devroye, L. (1986). *Non-Uniform Random Variate Generation*. Springer.
///   Chapter X.3, sequential search for the Poisson distribution.
///
/// # Arguments
///
/// * `lambda` - Finite, nonnegative expected number of events. Zero returns zero.
/// * `u` - Finite uniform probability in `[0, 1)`; zero returns the zero-count quantile.
///
/// # Errors
///
/// Returns `Error::Validation` if either input is outside its domain, if the
/// quantile cannot be represented by `usize`, or if the distribution CDF is
/// numerically unavailable.
pub fn poisson_inverse_cdf(lambda: f64, u: f64) -> crate::Result<usize> {
    if !lambda.is_finite() || lambda < 0.0 {
        return Err(crate::Error::Validation(format!(
            "Poisson intensity must be finite and nonnegative, got {lambda}"
        )));
    }
    if !u.is_finite() || !(0.0..1.0).contains(&u) {
        return Err(crate::Error::Validation(format!(
            "Poisson quantile probability must be finite and in [0, 1), got {u}"
        )));
    }
    if lambda == 0.0 || u == 0.0 {
        return Ok(0);
    }
    if lambda < SEQUENTIAL_INVERSION_MAX_INTENSITY {
        let mut mass = (-lambda).exp();
        let mut cdf = mass;
        let mut count = 0usize;
        while cdf < u && count < SEQUENTIAL_INVERSION_MAX_COUNT {
            count += 1;
            mass *= lambda / count as f64;
            cdf += mass;
        }
        if cdf >= u {
            return Ok(count);
        }
    }

    let distribution = Poisson::new(lambda).map_err(|error| {
        crate::Error::Validation(format!("invalid Poisson intensity {lambda}: {error}"))
    })?;
    let upper_tail = u > 0.5;
    let threshold = if upper_tail { 1.0 - u } else { u };
    let satisfies = |count: usize| -> crate::Result<bool> {
        let count_u64 = u64::try_from(count).map_err(|_| {
            crate::Error::Validation(format!(
                "Poisson count {count} exceeds statrs' supported count range"
            ))
        })?;
        let probability = if upper_tail {
            distribution.sf(count_u64)
        } else {
            distribution.cdf(count_u64)
        };
        if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
            return Err(crate::Error::Validation(format!(
                "Poisson CDF is unavailable for intensity {lambda} at count {count}"
            )));
        }
        Ok(if upper_tail {
            probability <= threshold
        } else {
            probability >= threshold
        })
    };

    if satisfies(0)? {
        return Ok(0);
    }

    // `low` never satisfies the quantile condition; `high` does. Doubling
    // provides a bounded logarithmic search without a fixed count cap.
    let mut low = 0usize;
    let mut high = 1usize;
    while !satisfies(high)? {
        low = high;
        high = match high.checked_mul(2) {
            Some(next) => next,
            None => usize::MAX,
        };
        if high == low {
            return Err(crate::Error::Validation(format!(
                "Poisson quantile for intensity {lambda} and probability {u} exceeds usize"
            )));
        }
    }

    while high - low > 1 {
        let mid = low + (high - low) / 2;
        if satisfies(mid)? {
            high = mid;
        } else {
            low = mid;
        }
    }
    Ok(high)
}

#[cfg(test)]
mod tests {
    use super::*;
    use statrs::distribution::DiscreteCDF;

    #[test]
    fn probability_within_rounding_of_one_hands_over_to_the_cdf_search() {
        let u = 1.0 - f64::EPSILON / 2.0;
        let count = poisson_inverse_cdf(0.04, u).expect("near-one quantile");
        let distribution = Poisson::new(0.04).expect("valid intensity");
        let count_u64 = u64::try_from(count).expect("count fits");
        assert!(count < 40, "a 0.04 intensity cannot need {count} jumps");
        assert!(distribution.sf(count_u64) <= 1.0 - u);
    }

    #[test]
    fn zero_intensity_and_zero_probability_return_zero() {
        assert_eq!(poisson_inverse_cdf(0.0, 0.5).expect("valid inputs"), 0);
        assert_eq!(poisson_inverse_cdf(0.0, 0.9).expect("valid inputs"), 0);
        assert_eq!(poisson_inverse_cdf(1.0, 0.0).expect("valid inputs"), 0);
    }

    #[test]
    fn quantile_brackets_cdf_on_both_sides_of_normal_approximation_threshold() {
        for lambda in [29.9, 30.0, 30.1, 100.0] {
            let distribution = Poisson::new(lambda).expect("valid intensity");
            for probability in [0.01, 0.3, 0.5, 0.9, 0.999] {
                let count = poisson_inverse_cdf(lambda, probability).expect("valid quantile");
                let count = u64::try_from(count).expect("count fits statrs");
                assert!(distribution.cdf(count) >= probability);
                if count > 0 {
                    assert!(distribution.cdf(count - 1) < probability);
                }
            }
        }
    }

    #[test]
    fn quantile_is_monotone_and_uses_the_upper_tail() {
        let probabilities = [0.0, 0.01, 0.5, 0.9, 0.99, 0.9999];
        let counts = probabilities
            .iter()
            .map(|&u| poisson_inverse_cdf(30.0, u).expect("valid quantile"))
            .collect::<Vec<_>>();
        assert!(counts.windows(2).all(|pair| pair[0] <= pair[1]));
        let upper_tail = poisson_inverse_cdf(30.0, 0.999).expect("valid upper-tail quantile");
        assert!(upper_tail >= 47);
    }

    #[test]
    fn invalid_inputs_return_errors() {
        for (lambda, u) in [
            (-1.0, 0.5),
            (f64::NAN, 0.5),
            (f64::INFINITY, 0.5),
            (1.0, -0.1),
            (1.0, 1.0),
            (1.0, f64::NAN),
        ] {
            assert!(poisson_inverse_cdf(lambda, u).is_err());
        }
    }

    #[test]
    fn compensated_exponential_moment_is_accurate_across_threshold() {
        let n_samples = 100_000usize;
        let jump_mean = -0.2_f64;
        for lambda in [29.9, 30.0, 30.1] {
            let sampled_moment = (0..n_samples)
                .map(|i| {
                    let u = (i as f64 + 0.5) / n_samples as f64;
                    (jump_mean * poisson_inverse_cdf(lambda, u).expect("valid quantile") as f64)
                        .exp()
                })
                .sum::<f64>()
                / n_samples as f64;
            let compensated = (-lambda * jump_mean.exp_m1()).exp() * sampled_moment;
            assert!(
                (compensated - 1.0).abs() < 0.002,
                "lambda={lambda}, result={compensated}"
            );
        }
    }
}
