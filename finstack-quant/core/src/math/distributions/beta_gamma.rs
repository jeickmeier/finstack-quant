//! Beta sampling from independent unit-scale Gamma draws.

use crate::math::random::RandomNumberGenerator;
use crate::math::summation::neumaier_sum;
use crate::{Error, Result};

const GAMMA_MAX_ATTEMPTS: usize = 100_000;

/// Draw a dimensionless Beta variate from independent unit-scale Gamma proposals.
///
/// # Arguments
///
/// * `rng` - Source of independent uniforms in `[0, 1)` and standard normal draws.
/// * `alpha` - Finite, strictly positive first shape parameter.
/// * `beta` - Finite, strictly positive second shape parameter.
///
/// # Returns
///
/// A finite sample in `[0, 1]`. For both shapes exactly one, returns one uniform.
/// Other shapes can produce rounded endpoints.
///
/// # Errors
///
/// Invalid shapes fail before consuming randomness. Malformed draws, an
/// unresolved Gamma ratio, or exhaustion of 100,000 proposals for either
/// shape return a computation error. Draws consumed before an error stay consumed.
///
/// # References
///
/// Marsaglia and Tsang (2000), *A Simple Method for Generating Gamma Variables*.
/// Devroye (1986), *Non-Uniform Random Variate Generation*, chapter IX.
pub fn sample_beta(rng: &mut dyn RandomNumberGenerator, alpha: f64, beta: f64) -> Result<f64> {
    for shape in [alpha, beta] {
        validate_shape(shape)?;
    }
    if alpha.to_bits() == 1.0_f64.to_bits() && beta.to_bits() == 1.0_f64.to_bits() {
        return uniform(rng, "Beta(1,1)");
    }
    let x = draw_gamma(rng, alpha, GAMMA_MAX_ATTEMPTS)?;
    let y = draw_gamma(rng, beta, GAMMA_MAX_ATTEMPTS)?;
    gamma_ratio(x, y)
}

fn validate_shape(shape: f64) -> Result<()> {
    if shape.is_finite() && shape > 0.0 {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "Beta shape must be finite and positive, got {shape}"
        )))
    }
}

fn uniform(rng: &mut dyn RandomNumberGenerator, stage: &str) -> Result<f64> {
    let u = rng.uniform();
    if (0.0..1.0).contains(&u) {
        Ok(u)
    } else {
        Err(Error::Internal(format!(
            "{stage}: expected a uniform in [0, 1), got {u}"
        )))
    }
}

/// Store the proposal components so common scales cancel before restoring the ratio.
#[derive(Debug, Clone, Copy)]
struct GammaDraw {
    d: f64,
    t: f64,
    base: f64,
    boost: Option<Boost>,
}

#[derive(Debug, Clone, Copy)]
struct Boost {
    uniform: f64,
    shape: f64,
}

fn draw_gamma(
    rng: &mut dyn RandomNumberGenerator,
    shape: f64,
    attempts: usize,
) -> Result<GammaDraw> {
    let boost = if shape < 1.0 {
        Some(Boost {
            uniform: uniform(rng, "Gamma augmentation")?,
            shape,
        })
    } else {
        None
    };
    let base_shape = if boost.is_some() { shape + 1.0 } else { shape };
    let d = base_shape - 1.0 / 3.0;
    let c = (1.0 / d.sqrt()) / 3.0;
    for attempt in 1..=attempts {
        let x = rng.normal(0.0, 1.0).map_err(|err| {
            Error::Internal(format!(
                "Gamma normal draw for shape {shape}, proposal {attempt}: {err}"
            ))
        })?;
        if !x.is_finite() {
            return Err(Error::Internal(format!(
                "Gamma normal draw for shape {shape}, proposal {attempt} must be finite, got {x}"
            )));
        }
        let t = c * x;
        let base = c.mul_add(x, 1.0);
        if base <= 0.0 {
            continue;
        }
        let u = uniform(rng, "Gamma rejection test")?;
        // Outside |x| < 3 the squeeze cannot accept. The restricted power
        // stays bounded even for a generator returning extreme finite normals.
        if u == 0.0
            || (x.abs() < 3.0 && 1.0 - u > 0.0331 * x.powi(4))
            || u.ln() < log_acceptance(d, t, base)?
        {
            return Ok(GammaDraw { d, t, base, boost });
        }
    }
    Err(Error::Internal(format!(
        "Gamma({shape}) rejection sampler exceeded {attempts} attempts"
    )))
}

/// MT's exact log threshold d * (3 log(1+t) - 3t + 1.5t² - t³).
fn log_acceptance(d: f64, t: f64, base: f64) -> Result<f64> {
    let result = if t.abs() <= 0.5 {
        // After all terms through cubic cancel, the remaining series starts
        // at -3t^4/4. Scaling by d first preserves tiny t at large shapes.
        let mut power = 1.0;
        let terms = (4..68).map(|k| {
            let term = power / f64::from(k);
            power *= -t;
            term
        });
        let h = -3.0 * neumaier_sum(terms)?;
        (((d * t) * t) * t) * t * h
    } else {
        let polynomial = t * t.mul_add(1.5 - t, -3.0);
        d * 3.0_f64.mul_add(base.ln(), polynomial)
    };
    if result.is_finite() && result <= 0.0 {
        Ok(result)
    } else {
        Err(Error::Internal(format!(
            "unresolved Gamma acceptance for scale {d}, displacement {t}"
        )))
    }
}

/// Log of a positive ratio, with a relative difference for neighbouring operands.
fn log_ratio(x: f64, y: f64) -> f64 {
    if x * 0.5 <= y && y * 0.5 <= x {
        ((x - y) / y).ln_1p()
    } else {
        let ratio = x / y;
        if ratio.is_normal() {
            ratio.ln()
        } else {
            x.ln() - y.ln()
        }
    }
}

fn gamma_ratio(x: GammaDraw, y: GammaDraw) -> Result<f64> {
    let zero = |draw: GammaDraw| draw.boost.is_some_and(|b| b.uniform == 0.0);
    match (zero(x), zero(y)) {
        (true, true) => {
            return Err(Error::Internal(
                "Beta ratio has two exact-zero Gamma draws".into(),
            ));
        }
        (true, false) => return Ok(0.0),
        (false, true) => return Ok(1.0),
        (false, false) => {}
    }
    let boost = match (x.boost, y.boost) {
        (Some(a), Some(b)) => boost_difference(a, b)?,
        (Some(a), None) => a.uniform.ln() / a.shape,
        (None, Some(b)) => -b.uniform.ln() / b.shape,
        (None, None) => 0.0,
    };
    // A restored boost can exceed the binary64 range. Both base terms
    // are bounded logs of finite operands, so its sign determines the result.
    if boost.is_infinite() {
        return Ok(if boost.is_sign_positive() { 1.0 } else { 0.0 });
    }
    let local = if x.t.abs() <= 0.5 && y.t.abs() <= 0.5 {
        ((x.t - y.t) / (1.0 + y.t)).ln_1p()
    } else {
        log_ratio(x.base, y.base)
    };
    let delta = neumaier_sum([log_ratio(x.d, y.d), 3.0 * local, boost])?;
    let t = (-delta.abs()).exp();
    let small = t / (1.0 + t);
    Ok(if delta < 0.0 { small } else { 1.0 - small })
}

/// Difference of log(U)/shape terms, expressed over the smaller shape.
/// Common enormous boosts cancel before division.
fn boost_difference(a: Boost, b: Boost) -> Result<f64> {
    if a.shape > b.shape {
        return Ok(-boost_difference(b, a)?);
    }
    let log_u_ratio = log_ratio(a.uniform, b.uniform);
    if a.shape.to_bits() == b.shape.to_bits() {
        return Ok(log_u_ratio / a.shape);
    }
    let (coefficient, log_b, direct) = if a.shape >= 0.5 * b.shape {
        ((b.shape - a.shape) / b.shape, b.uniform.ln(), log_u_ratio)
    } else {
        (-a.shape / b.shape, b.uniform.ln(), a.uniform.ln())
    };
    let numerator = coefficient.mul_add(log_b, direct);
    // When the two terms nearly cancel, their transcendental and division
    // rounding can hide the sign. Error if the small shape amplifies it.
    let uncertainty = (coefficient * log_b).abs().max(direct.abs()) * (8.0 * f64::EPSILON);
    if numerator.abs() <= uncertainty && uncertainty / a.shape > 8.0 * f64::EPSILON {
        return Err(Error::Internal(
            "Beta log ratio is unresolved after boost cancellation".into(),
        ));
    }
    Ok(numerator / a.shape)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Restore a standalone unit-scale Gamma only for these bulk moment checks.
    fn sample_gamma(rng: &mut dyn RandomNumberGenerator, shape: f64) -> Result<f64> {
        validate_shape(shape)?;
        let g = draw_gamma(rng, shape, GAMMA_MAX_ATTEMPTS)?;
        let boost = g.boost.map_or(0.0, |b| b.uniform.ln() / b.shape);
        Ok((g.d.ln() + 3.0 * g.base.ln() + boost).exp())
    }

    #[derive(Clone, Copy)]
    enum Draw {
        Normal(f64),
        Uniform(f64),
    }

    struct Script(std::collections::VecDeque<Draw>);

    impl RandomNumberGenerator for Script {
        fn normal(&mut self, mean: f64, std_dev: f64) -> Result<f64> {
            assert_eq!((mean, std_dev), (0.0, 1.0));
            match self.0.pop_front() {
                Some(Draw::Normal(x)) => Ok(x),
                _ => panic!("unexpected normal request"),
            }
        }

        fn uniform(&mut self) -> f64 {
            match self.0.pop_front() {
                Some(Draw::Uniform(u)) => u,
                _ => panic!("unexpected uniform request"),
            }
        }
    }

    fn scripted(alpha: f64, beta: f64, draws: &[Draw]) -> Result<f64> {
        let mut script = Script(draws.iter().copied().collect());
        let result = sample_beta(&mut script, alpha, beta);
        assert!(
            script.0.is_empty(),
            "sampler did not consume the expected sequence"
        );
        result
    }

    #[test]
    fn rejects_malformed_draws_at_each_stage_and_checks_shapes_before_drawing() {
        use Draw::{Normal, Uniform};
        assert!(scripted(1.0, f64::NAN, &[]).is_err());
        for bad in [1.0, -0.01, f64::INFINITY, f64::NAN] {
            assert!(scripted(1.0, 1.0, &[Uniform(bad)]).is_err());
            assert!(scripted(0.5, 2.0, &[Uniform(bad)]).is_err());
            assert!(scripted(2.0, 2.0, &[Normal(0.0), Uniform(bad)]).is_err());
        }
        assert!(scripted(2.0, 2.0, &[Normal(f64::NAN)]).is_err());
        assert_eq!(scripted(1.0, 1.0, &[Uniform(0.0)]).unwrap(), 0.0);
    }

    #[test]
    fn valid_zero_boosts_consume_both_base_draws() {
        use Draw::{Normal, Uniform};
        for (u, v, expected) in [
            (0.0, 0.5, Some(0.0)),
            (0.5, 0.0, Some(1.0)),
            (0.0, 0.0, None),
        ] {
            let draws = [
                Uniform(u),
                Normal(0.0),
                Uniform(0.0),
                Uniform(v),
                Normal(0.0),
                Uniform(0.0),
            ];
            assert_eq!(scripted(0.5, 0.5, &draws).ok(), expected);
        }
    }

    #[test]
    fn extreme_shapes_preserve_scale_and_boost_differences() {
        use Draw::{Normal, Uniform};
        let draws = [Normal(0.0), Uniform(0.5), Normal(0.0), Uniform(0.5)];
        assert_eq!(scripted(1e308, 1e308, &draws).unwrap(), 0.5);
        assert!((scripted(1e308, 5e307, &draws).unwrap() - 2.0 / 3.0).abs() < f64::EPSILON);

        // Choose the first accepted cubic proposal twice the second. Even
        // identical unrepresentably large log-boosts must cancel exactly.
        let d = 1.0_f64 - 1.0 / 3.0;
        let x = 3.0 * d.sqrt() * (2.0_f64.cbrt() - 1.0);
        for shape in [2.0_f64.powi(-1000), f64::from_bits(1)] {
            let draws = [
                Uniform(0.5),
                Normal(x),
                Uniform(0.0),
                Uniform(0.5),
                Normal(0.0),
                Uniform(0.0),
            ];
            assert!((scripted(shape, shape, &draws).unwrap() - 2.0 / 3.0).abs() < 3e-16);
        }
    }

    #[test]
    fn logistic_restores_subnormal_tail_and_last_bit_below_one() {
        let base = GammaDraw {
            d: 1.0,
            t: 0.0,
            base: 1.0,
            boost: None,
        };
        for magnitude in [37.0, 710.0, 745.0, 746.0] {
            let boosted = GammaDraw {
                boost: Some(Boost {
                    uniform: (-1.0_f64).exp(),
                    shape: 1.0 / magnitude,
                }),
                ..base
            };
            let small = gamma_ratio(boosted, base).unwrap();
            let large = gamma_ratio(base, boosted).unwrap();
            assert_eq!(large, 1.0 - small);
            match magnitude as u32 {
                37 => assert!(large < 1.0 && (small / 8.533047625744066e-17 - 1.0).abs() < 1e-14),
                710 => assert!((small / 4.47628622567513e-309 - 1.0).abs() < 2e-13),
                745 => assert_eq!(small.to_bits(), 1),
                _ => assert_eq!(small.to_bits(), 0),
            }
        }
    }

    #[test]
    fn positive_fused_base_survives_rounded_minus_one() {
        use Draw::{Normal, Uniform};
        let d = 1.0_f64 - 1.0 / 3.0;
        let c = (1.0 / d.sqrt()) / 3.0;
        let x = -1.0 / c;
        assert_eq!(c * x, -1.0);
        assert!(c.mul_add(x, 1.0) > 0.0);
        let mut script = Script([Normal(x), Uniform(0.0)].into());
        let draw = draw_gamma(&mut script, 1.0, 1).unwrap();
        assert_eq!(draw.base, c.mul_add(x, 1.0));
        assert!(script.0.is_empty());
    }

    #[test]
    fn small_displacement_rejection_is_resolved_before_retrying() {
        use Draw::{Normal, Uniform};
        let x = 1e16;
        let d = 1e64_f64;
        let t = (1.0 / d.sqrt()) / 3.0 * x;
        assert!((log_acceptance(d, t, 1.0 + t).unwrap() + 0.00925925925925926).abs() < 1e-16);
        let mut script = Script([Normal(x), Uniform(0.999), Normal(0.0), Uniform(0.1)].into());
        let g = draw_gamma(&mut script, d, 2).unwrap();
        assert_eq!(g.t, 0.0);
        assert!(script.0.is_empty());
        for t in [-0.5000000001_f64, -0.5, 0.5, 0.5000000001] {
            let expected = 3.0 * t.ln_1p() + t * (-3.0 + t * (1.5 - t));
            assert!((log_acceptance(1.0, t, 1.0 + t).unwrap() - expected).abs() < 5e-16);
        }
    }

    #[test]
    fn beta_two_one_cdf_across_seeds() {
        use crate::math::random::Pcg64Rng;
        const N: usize = 8192;
        for seed in [41, 735, 2019] {
            let mut rng = Pcg64Rng::new(seed, 0);
            let draws: Vec<_> = (0..N)
                .map(|_| sample_beta(&mut rng, 2.0, 1.0).unwrap())
                .collect();
            for x in [0.1_f64, 0.45, 0.9] {
                let p = x * x; // analytic CDF of Beta(2,1)
                let empirical = draws.iter().filter(|&&v| v <= x).count() as f64 / N as f64;
                let se = (p * (1.0 - p) / N as f64).sqrt();
                assert!(
                    (empirical - p).abs() <= 6.0 * se,
                    "seed={seed}, x={x}, CDF={empirical}"
                );
            }
        }
    }

    struct RejectingRng;

    impl RandomNumberGenerator for RejectingRng {
        fn uniform(&mut self) -> f64 {
            0.5
        }

        fn normal(&mut self, _mean: f64, _std_dev: f64) -> crate::Result<f64> {
            Ok(-10.0)
        }
    }

    #[test]
    fn gamma_rejection_loop_has_a_hard_limit() {
        let mut rng = RejectingRng;
        let error =
            draw_gamma(&mut rng, 1.0, 3).expect_err("deterministic rejection must terminate");
        assert!(error.to_string().contains("attempt"));
    }

    #[test]
    fn test_sample_beta() {
        use crate::math::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42, 0);

        // Test uniform case (alpha=1, beta=1)
        let uniform_sample = sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 1.0, 1.0)
            .expect("Beta(1,1) should succeed");
        assert!((0.0..=1.0).contains(&uniform_sample));

        // Test that samples are in [0, 1]
        let samples: Vec<f64> = (0..100)
            .map(|_| {
                sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 2.0, 2.0)
                    .expect("Beta(2,2) should succeed")
            })
            .collect();
        for sample in samples {
            assert!((0.0..=1.0).contains(&sample));
        }
    }

    #[test]
    fn test_sample_beta_statistics() {
        use crate::math::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(12345, 0);
        let n_samples = 10_000;

        // Test Beta(4, 2) - expected mean = 4/(4+2) = 0.6667
        let alpha: f64 = 4.0;
        let beta_param: f64 = 2.0;
        let expected_mean = alpha / (alpha + beta_param);
        let expected_var =
            (alpha * beta_param) / ((alpha + beta_param).powi(2) * (alpha + beta_param + 1.0));

        let samples: Vec<f64> = (0..n_samples)
            .map(|_| {
                sample_beta(
                    &mut rng as &mut dyn RandomNumberGenerator,
                    alpha,
                    beta_param,
                )
                .expect("Beta(4,2) should succeed")
            })
            .collect();

        let sample_mean = samples.iter().sum::<f64>() / n_samples as f64;
        let sample_var = samples
            .iter()
            .map(|x| (x - sample_mean).powi(2))
            .sum::<f64>()
            / (n_samples - 1) as f64;

        // Allow 5% relative error for mean (statistical tolerance)
        assert!(
            (sample_mean - expected_mean).abs() < 0.05 * expected_mean,
            "Beta(4,2) mean: expected {:.4}, got {:.4}",
            expected_mean,
            sample_mean
        );

        // Allow 20% relative error for variance (higher tolerance due to sampling variance)
        assert!(
            (sample_var - expected_var).abs() < 0.20 * expected_var,
            "Beta(4,2) variance: expected {:.4}, got {:.4}",
            expected_var,
            sample_var
        );
    }

    #[test]
    fn test_sample_beta_small_shape() {
        use crate::math::random::Pcg64Rng;

        // Test with shape parameters < 1 (uses shape augmentation)
        let mut rng = Pcg64Rng::new(9999, 0);
        let samples: Vec<f64> = (0..1000)
            .map(|_| {
                sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 0.5, 0.5)
                    .expect("Beta(0.5,0.5) should succeed")
            })
            .collect();

        // All samples should be in [0, 1]
        for sample in &samples {
            assert!(
                (0.0..=1.0).contains(sample),
                "Beta(0.5, 0.5) sample {} out of bounds",
                sample
            );
        }

        // Beta(0.5, 0.5) is the arcsine distribution with mean = 0.5
        let sample_mean = samples.iter().sum::<f64>() / samples.len() as f64;
        assert!(
            (sample_mean - 0.5).abs() < 0.1,
            "Beta(0.5, 0.5) mean: expected ~0.5, got {:.4}",
            sample_mean
        );
    }

    #[test]
    fn test_sample_beta_validation() {
        use crate::math::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42, 0);

        // Invalid alpha
        assert!(sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 0.0, 1.0).is_err());
        assert!(sample_beta(&mut rng as &mut dyn RandomNumberGenerator, -1.0, 1.0).is_err());

        // Invalid beta
        assert!(sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 1.0, 0.0).is_err());
        assert!(sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 1.0, -1.0).is_err());
    }

    // Gamma Distribution Tests

    #[test]
    fn test_sample_gamma_basic() {
        use crate::math::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42, 0);

        // All samples should be non-negative
        for _ in 0..100 {
            let x = sample_gamma(&mut rng as &mut dyn RandomNumberGenerator, 2.0)
                .expect("Gamma(2.0) should succeed");
            assert!(x >= 0.0, "Gamma sample should be non-negative, got {}", x);
        }
    }

    #[test]
    fn test_sample_gamma_small_shape() {
        use crate::math::random::Pcg64Rng;

        // Test with shape < 1 (uses shape augmentation)
        let mut rng = Pcg64Rng::new(42, 0);

        for _ in 0..100 {
            let x = sample_gamma(&mut rng as &mut dyn RandomNumberGenerator, 0.5)
                .expect("Gamma(0.5) should succeed");
            assert!(
                x >= 0.0,
                "Gamma(0.5) sample should be non-negative, got {}",
                x
            );
        }
    }

    #[test]
    fn test_sample_gamma_statistics() {
        use crate::math::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(12345, 0);
        let shape = 3.0; // Mean = shape, Variance = shape (for rate=1)
        let n_samples = 10_000;

        let samples: Vec<f64> = (0..n_samples)
            .map(|_| {
                sample_gamma(&mut rng as &mut dyn RandomNumberGenerator, shape)
                    .expect("Gamma(3.0) should succeed")
            })
            .collect();

        let sample_mean = samples.iter().sum::<f64>() / n_samples as f64;

        // Allow 5% relative error
        assert!(
            (sample_mean - shape).abs() < 0.05 * shape,
            "Gamma({}) mean: expected {:.4}, got {:.4}",
            shape,
            shape,
            sample_mean
        );
    }

    #[test]
    fn test_sample_gamma_validation() {
        use crate::math::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42, 0);

        // Invalid shape
        assert!(sample_gamma(&mut rng as &mut dyn RandomNumberGenerator, 0.0).is_err());
        assert!(sample_gamma(&mut rng as &mut dyn RandomNumberGenerator, -1.0).is_err());
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(sample_gamma(&mut rng, invalid).is_err());
            assert!(sample_beta(&mut rng, invalid, 1.0).is_err());
            assert!(sample_beta(&mut rng, 1.0, invalid).is_err());
        }
    }
}
