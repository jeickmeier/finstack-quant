//! Joint probability utilities for correlated events.
//!
//! Provides functions for computing joint probabilities of correlated Bernoulli
//! random variables, useful for tree-based pricing and scenario generation.
//!
//! # Mathematical Framework
//!
//! Given two Bernoulli random variables X₁ ~ Bern(p₁) and X₂ ~ Bern(p₂)
//! with correlation ρ, we need joint probabilities:
//!
//! - P(X₁=1, X₂=1) = p₁₁
//! - P(X₁=1, X₂=0) = p₁₀
//! - P(X₁=0, X₂=1) = p₀₁
//! - P(X₁=0, X₂=0) = p₀₀
//!
//! The covariance constraint is:
//! ```text
//! Cov(X₁, X₂) = ρ · √(Var(X₁) · Var(X₂)) = ρ · √(p₁(1-p₁) · p₂(1-p₂))
//! ```
//!
//! # Bernoulli Coupling
//!
//! The standard approach for inducing correlation:
//! ```text
//! p₁₁ = p₁ · p₂ + Cov(X₁, X₂)
//! p₁₀ = p₁ · (1-p₂) - Cov(X₁, X₂)
//! p₀₁ = (1-p₁) · p₂ - Cov(X₁, X₂)
//! p₀₀ = (1-p₁) · (1-p₂) + Cov(X₁, X₂)
//! ```
//!
//! # References
//!
//! - Lancaster, H. O. (1957). "Some properties of the bivariate normal distribution."
//! - Demarta, S., & McNeil, A. J. (2005). "The t Copula and Related Copulas." `docs/REFERENCES.md#demarta-mcneil-2005-t-copula` `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`

/// Correlated Bernoulli distribution for scenario generation.
///
/// Provides methods for working with correlated binary outcomes,
/// useful for tree-based pricing and analytical calculations.
#[derive(Clone, Debug)]
pub struct CorrelatedBernoulli {
    p1: f64,
    p2: f64,
    correlation: f64,
    // Precomputed joint probabilities
    p11: f64,
    p10: f64,
    p01: f64,
    p00: f64,
}

impl CorrelatedBernoulli {
    /// Create a correlated Bernoulli distribution.
    ///
    /// # Arguments
    /// * `p1` - Finite probability of the first event in [0, 1].
    /// * `p2` - Finite probability of the second event in [0, 1].
    /// * `correlation` - Pearson correlation within the attainable bounds of the marginals.
    ///   For deterministic marginals supply zero.
    ///
    /// # Errors
    ///
    /// Returns a validation error for nonfinite or infeasible inputs.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::math::probability::CorrelatedBernoulli;
    /// let distribution = CorrelatedBernoulli::new(0.6, 0.4, 0.3)?;
    /// let (p11, p10, p01, p00) = distribution.joint_probabilities();
    /// assert!((p11 + p10 - 0.6).abs() < 1e-10);
    /// assert!((p11 + p01 - 0.4).abs() < 1e-10);
    /// assert!((p11 + p10 + p01 + p00 - 1.0).abs() < 1e-10);
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    #[must_use = "construction validation errors must be handled"]
    pub fn new(p1: f64, p2: f64, correlation: f64) -> crate::Result<Self> {
        if !(-1.0..=1.0).contains(&correlation) {
            return Err(crate::Error::Validation(format!(
                "Bernoulli correlation must be finite and in [-1, 1], got {correlation}"
            )));
        }
        let q1 = 1.0 - p1;
        let q2 = 1.0 - p2;
        let (lo, hi) = correlation_bounds(p1, p2)?;
        if !(lo..=hi).contains(&correlation) {
            return Err(crate::Error::Validation(format!(
                "infeasible Bernoulli correlation {correlation} outside [{lo}, {hi}]"
            )));
        }
        let independent = [p1 * p2, p1 * q2, q1 * p2, q1 * q2];
        // At fixed marginals each joint cell is linear in correlation. Interpolate
        // between independence and the exact Fréchet table to keep zero cells
        // nonnegative at the attainable endpoints.
        let [p11, p10, p01, p00] = if correlation == 0.0 {
            independent
        } else {
            let (weight, endpoint) = if correlation > 0.0 {
                let joint = p1.min(p2);
                (
                    correlation / hi,
                    [joint, p1 - joint, p2 - joint, q1.min(q2)],
                )
            } else {
                let excess = p1.min(p2) - (1.0 - p1.max(p2));
                (
                    correlation / lo,
                    [excess.max(0.0), p1.min(q2), p2.min(q1), (-excess).max(0.0)],
                )
            };
            std::array::from_fn(|i| weight.mul_add(endpoint[i], (1.0 - weight) * independent[i]))
        };
        Ok(Self {
            p1,
            p2,
            correlation,
            p11,
            p10,
            p01,
            p00,
        })
    }

    /// Sample a pair of correlated binary outcomes given a uniform random value.
    ///
    /// Returns (x1, x2) where each is 0 or 1.
    ///
    /// # Arguments
    /// * `u` - Uniform random value in [0, 1]
    ///
    /// The half-open bucket comparisons map `u` deterministically to the
    /// stored joint distribution. This is an inverse-CDF transform, not a
    /// random-number generator; callers supply independent uniforms when they
    /// need independent draws.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] if `u` is `NaN`, infinite, less
    /// than zero, or greater than one. Both endpoints are valid: `0.0` selects
    /// the first non-empty bucket and `1.0` selects `(0, 0)`.
    pub fn sample_from_uniform(&self, u: f64) -> crate::Result<(u8, u8)> {
        if !u.is_finite() || !(0.0..=1.0).contains(&u) {
            return Err(crate::Error::Validation(format!(
                "uniform sample must be finite and in [0, 1], got {u}"
            )));
        }
        let outcome = if u < self.p11 {
            (1, 1)
        } else if u < self.p11 + self.p10 {
            (1, 0)
        } else if u < self.p11 + self.p10 + self.p01 {
            (0, 1)
        } else {
            (0, 0)
        };
        Ok(outcome)
    }

    /// Get the marginal probability of event 1.
    pub fn p1(&self) -> f64 {
        self.p1
    }

    /// Get the marginal probability of event 2.
    pub fn p2(&self) -> f64 {
        self.p2
    }

    /// Get the correlation.
    pub fn correlation(&self) -> f64 {
        self.correlation
    }

    /// Get the joint probability P(X₁=1, X₂=1).
    pub fn joint_p11(&self) -> f64 {
        self.p11
    }

    /// Get the joint probability P(X₁=1, X₂=0).
    pub fn joint_p10(&self) -> f64 {
        self.p10
    }

    /// Get the joint probability P(X₁=0, X₂=1).
    pub fn joint_p01(&self) -> f64 {
        self.p01
    }

    /// Get the joint probability P(X₁=0, X₂=0).
    pub fn joint_p00(&self) -> f64 {
        self.p00
    }

    /// Get the conditional probability P(X₂=1 | X₁=1).
    pub fn conditional_p2_given_x1(&self) -> f64 {
        if self.p1 > 0.0 && self.correlation != 0.0 {
            self.p11 / self.p1
        } else {
            self.p2
        }
    }

    /// Get the conditional probability P(X₁=1 | X₂=1).
    pub fn conditional_p1_given_x2(&self) -> f64 {
        if self.p2 > 0.0 && self.correlation != 0.0 {
            self.p11 / self.p2
        } else {
            self.p1
        }
    }

    /// Get all four joint probabilities as a tuple.
    pub fn joint_probabilities(&self) -> (f64, f64, f64, f64) {
        (self.p11, self.p10, self.p01, self.p00)
    }
}

/// Compute the achievable correlation bounds for given marginal probabilities.
///
/// The Fréchet-Hoeffding bounds constrain the feasible correlation range:
/// ```text
/// ρ_min ≤ ρ ≤ ρ_max
/// ```
///
/// These bounds ensure that all joint probabilities remain in [0, 1].
///
/// # Arguments
/// * `p1` - Marginal probability P(X₁=1)
/// * `p2` - Marginal probability P(X₂=1)
///
/// # Returns
/// Tuple (ρ_min, ρ_max) of achievable correlation bounds
/// for the supplied Bernoulli marginals. A degenerate marginal has zero
/// variance, so the function returns `(0.0, 0.0)` instead of attempting an
/// undefined correlation division.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] if either marginal is `NaN`, infinite,
/// or outside `[0, 1]`. It does not take a requested correlation and therefore
/// never clamps one.
#[must_use = "correlation-bound validation errors must be handled"]
pub fn correlation_bounds(p1: f64, p2: f64) -> crate::Result<(f64, f64)> {
    for (name, p) in [("p1", p1), ("p2", p2)] {
        if !(0.0..=1.0).contains(&p) {
            return Err(crate::Error::Validation(format!(
                "Bernoulli marginal {name} must be finite and in [0, 1], got {p}"
            )));
        }
    }
    let m = p1.min(p2);
    let big = p1.max(p2);
    if m == 0.0 || big >= 1.0 {
        return Ok((0.0, 0.0));
    }
    let upper = (m / big).sqrt() * ((1.0 - big) / (1.0 - m)).sqrt();
    let lower = if m <= 1.0 - big {
        -(m / (1.0 - big)).sqrt() * (big / (1.0 - m)).sqrt()
    } else {
        -((1.0 - big) / m).sqrt() * ((1.0 - m) / big).sqrt()
    };
    // A rounded bound can reach unity for adjacent but distinct marginals.
    // Perfect correlation requires identical events, and perfect negative
    // correlation requires exactly complementary marginals.
    let upper = if m < big {
        upper.min(1.0_f64.next_down())
    } else {
        1.0
    };
    let lower = if (m - (1.0 - big)) == 0.0 {
        -1.0
    } else {
        lower.max((-1.0_f64).next_up())
    };
    Ok((lower, upper))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_joint_probabilities_sum_to_one() {
        let test_cases = vec![
            (0.5, 0.5, 0.0),
            (0.6, 0.4, 0.3),
            (0.3, 0.7, -0.2),
            (0.1, 0.9, 0.0),
            (0.5, 0.5, 0.5),
        ];

        for (p1, p2, corr) in test_cases {
            let (p11, p10, p01, p00) = CorrelatedBernoulli::new(p1, p2, corr)
                .expect("valid probabilities")
                .joint_probabilities();
            let sum = p11 + p10 + p01 + p00;
            assert!(
                (sum - 1.0).abs() < 1e-10,
                "Sum {} should be 1.0 for p1={}, p2={}, corr={}",
                sum,
                p1,
                p2,
                corr
            );
        }
    }

    #[test]
    fn test_joint_probabilities_marginals() {
        let test_cases = vec![
            (0.6, 0.4, 0.3),
            (0.5, 0.5, 0.8),
            (0.1, 0.9, -0.2),
            (0.3, 0.7, 0.4),
            (0.8, 0.2, -0.5),
        ];

        for (p1, p2, corr) in test_cases {
            let (p11, p10, p01, p00) = CorrelatedBernoulli::new(p1, p2, corr)
                .expect("valid probabilities")
                .joint_probabilities();

            // Check marginal p1 = p11 + p10 (must be exact)
            let computed_p1 = p11 + p10;
            assert!(
                (computed_p1 - p1).abs() < 1e-10,
                "Marginal p1 {} should equal {} for corr={}",
                computed_p1,
                p1,
                corr
            );

            // Check marginal p2 = p11 + p01 (must be exact)
            let computed_p2 = p11 + p01;
            assert!(
                (computed_p2 - p2).abs() < 1e-10,
                "Marginal p2 {} should equal {} for corr={}",
                computed_p2,
                p2,
                corr
            );

            // All probabilities must be non-negative
            assert!(p11 >= 0.0, "p11 must be non-negative");
            assert!(p10 >= 0.0, "p10 must be non-negative");
            assert!(p01 >= 0.0, "p01 must be non-negative");
            assert!(p00 >= 0.0, "p00 must be non-negative");

            assert!((p11 + p10 + p01 + p00 - 1.0).abs() <= 2.0 * f64::EPSILON);
        }
    }

    #[test]
    fn test_joint_probabilities_extreme_correlations() {
        // Equal interior marginals admit both perfect endpoints at one half.
        let (p1, p2) = (0.5, 0.5);
        let (rho_min, rho_max) = correlation_bounds(p1, p2).expect("valid marginals");

        // Correlations outside the mathematical [-1, 1] domain are rejected.
        assert!(CorrelatedBernoulli::new(p1, p2, 2.0).is_err());
        assert!(CorrelatedBernoulli::new(p1, p2, -2.0).is_err());

        // At exact bounds
        let (p11, p10, _, _) = CorrelatedBernoulli::new(p1, p2, rho_max)
            .expect("upper feasible bound")
            .joint_probabilities();
        assert!((p11 + p10 - p1).abs() < 1e-10);

        let (p11, p10, _, _) = CorrelatedBernoulli::new(p1, p2, rho_min)
            .expect("lower feasible bound")
            .joint_probabilities();
        assert!((p11 + p10 - p1).abs() < 1e-10);
    }

    #[test]
    fn unequal_marginals_accept_both_reported_bounds() {
        for (p, q) in [
            (0.2, 0.6),
            (0.12, 0.03),
            (0.8, 0.9),
            (0.5, 0.5_f64.next_up()),
        ] {
            let (low, high) = correlation_bounds(p, q).unwrap();
            for rho in [low, low.next_up(), high.next_down(), high] {
                let (both, first, second, neither) = CorrelatedBernoulli::new(p, q, rho)
                    .unwrap()
                    .joint_probabilities();
                assert!(
                    [both, first, second, neither]
                        .iter()
                        .all(|cell| (0.0..=1.0).contains(cell))
                );
                assert!((both + first - p).abs() <= f64::EPSILON);
                assert!((both + second - q).abs() <= f64::EPSILON);
            }
            assert!(CorrelatedBernoulli::new(p, q, low.next_down()).is_err());
            assert!(CorrelatedBernoulli::new(p, q, high.next_up()).is_err());
        }
    }

    #[test]
    fn test_joint_probabilities_independent() {
        let (p1, p2) = (0.4, 0.6);
        let (p11, p10, p01, p00) = CorrelatedBernoulli::new(p1, p2, 0.0)
            .expect("valid probabilities")
            .joint_probabilities();

        // With zero correlation, should be independent
        assert!(
            (p11 - p1 * p2).abs() < 1e-6,
            "p11 {} should equal p1*p2 = {} for independent",
            p11,
            p1 * p2
        );
        assert!(
            (p00 - (1.0 - p1) * (1.0 - p2)).abs() < 1e-6,
            "p00 {} should equal (1-p1)*(1-p2)",
            p00
        );
        assert!((p10 - p1 * (1.0 - p2)).abs() <= f64::EPSILON);
        assert!((p01 - (1.0 - p1) * p2).abs() <= f64::EPSILON);
    }

    #[test]
    fn test_positive_correlation_increases_joint() {
        let (p1, p2) = (0.5, 0.5);
        let (p11_pos, _, _, _) = CorrelatedBernoulli::new(p1, p2, 0.5)
            .expect("positive correlation")
            .joint_probabilities();
        let (p11_zero, _, _, _) = CorrelatedBernoulli::new(p1, p2, 0.0)
            .expect("zero correlation")
            .joint_probabilities();
        let (p11_neg, _, _, _) = CorrelatedBernoulli::new(p1, p2, -0.5)
            .expect("negative correlation")
            .joint_probabilities();

        // Positive correlation should increase P(both happen)
        assert!(p11_pos > p11_zero);
        assert!(p11_zero > p11_neg);
    }

    #[test]
    fn test_correlated_bernoulli_creation() {
        let dist = CorrelatedBernoulli::new(0.5, 0.5, 0.5).expect("valid distribution");
        assert!((dist.p1() - 0.5).abs() < 1e-10);
        assert!((dist.p2() - 0.5).abs() < 1e-10);
        assert!((dist.correlation() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn rare_marginals_preserve_feasible_dependence() {
        for p in [2.0_f64.powi(-35), 2.0_f64.powi(-40)] {
            let perfect = CorrelatedBernoulli::new(p, p, 1.0).expect("equal marginals");
            assert_eq!(perfect.joint_p11(), p);
            assert_eq!(perfect.conditional_p2_given_x1(), 1.0);
            assert_eq!(
                CorrelatedBernoulli::new(p, p, 0.0).unwrap().joint_p11(),
                p * p
            );
            assert!(CorrelatedBernoulli::new(p, p, -1.0).is_err());
        }
        assert!(CorrelatedBernoulli::new(0.05, 0.95, 0.9).is_err());
    }

    #[test]
    fn test_sample_from_uniform() {
        let dist = CorrelatedBernoulli::new(0.5, 0.5, 0.0).expect("valid distribution");

        // At u=0, should get (1, 1)
        let (x1, x2) = dist.sample_from_uniform(0.0).expect("valid uniform");
        assert_eq!((x1, x2), (1, 1));

        // At u=0.99, should get (0, 0)
        let (x1, x2) = dist.sample_from_uniform(0.99).expect("valid uniform");
        assert_eq!((x1, x2), (0, 0));
    }

    #[test]
    fn test_sample_from_uniform_covers_all_joint_buckets() {
        let dist = CorrelatedBernoulli::new(0.5, 0.5, 0.0).expect("valid distribution");

        assert_eq!(dist.joint_probabilities(), (0.25, 0.25, 0.25, 0.25));
        assert_eq!(dist.sample_from_uniform(0.125).expect("sample"), (1, 1));
        assert_eq!(dist.sample_from_uniform(0.375).expect("sample"), (1, 0));
        assert_eq!(dist.sample_from_uniform(0.625).expect("sample"), (0, 1));
        assert_eq!(dist.sample_from_uniform(0.875).expect("sample"), (0, 0));
    }

    #[test]
    fn correlated_bernoulli_rejects_invalid_inputs() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(CorrelatedBernoulli::new(invalid, 0.5, 0.0).is_err());
            assert!(CorrelatedBernoulli::new(0.5, invalid, 0.0).is_err());
            assert!(CorrelatedBernoulli::new(0.5, 0.5, invalid).is_err());
        }
        assert!(CorrelatedBernoulli::new(-0.1, 0.5, 0.0).is_err());
        assert!(CorrelatedBernoulli::new(0.5, 1.1, 0.0).is_err());
        assert!(CorrelatedBernoulli::new(0.5, 0.5, 1.1).is_err());

        let dist = CorrelatedBernoulli::new(0.5, 0.5, 0.0).expect("valid distribution");
        for invalid in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
            assert!(dist.sample_from_uniform(invalid).is_err());
        }
    }

    #[test]
    fn test_correlation_bounds() {
        let (rho_min, rho_max) = correlation_bounds(0.5, 0.5).expect("valid marginals");

        // For p1=p2=0.5, bounds should be symmetric around 0
        assert!(rho_min < 0.0);
        assert!(rho_max > 0.0);

        // Perfect correlation possible when p1=p2=0.5
        assert!((rho_max - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_conditional_probabilities() {
        let dist = CorrelatedBernoulli::new(0.6, 0.4, 0.5).expect("valid distribution");

        // Conditional should be higher than marginal with positive correlation
        let cond_p2_given_x1 = dist.conditional_p2_given_x1();
        assert!(
            cond_p2_given_x1 > dist.p2(),
            "P(X2|X1) {} should exceed P(X2) {} with positive correlation",
            cond_p2_given_x1,
            dist.p2()
        );
    }

    #[test]
    fn test_joint_probabilities_accessor() {
        let dist = CorrelatedBernoulli::new(0.6, 0.4, 0.3).expect("valid distribution");
        let (p11, p10, p01, p00) = dist.joint_probabilities();

        assert!((p11 + p10 + p01 + p00 - 1.0).abs() < 1e-10);
        assert_eq!(p11, dist.joint_p11());
        assert_eq!(p10, dist.joint_p10());
        assert_eq!(p01, dist.joint_p01());
        assert_eq!(p00, dist.joint_p00());
    }
}
