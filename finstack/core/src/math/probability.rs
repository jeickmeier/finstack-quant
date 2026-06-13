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
//! - Demarta, S., & McNeil, A. J. (2005). "The t Copula and Related Copulas."

/// Variance threshold below which a Bernoulli variable is treated as degenerate
/// (i.e., probability is effectively 0 or 1, making correlation meaningless).
const DEGENERATE_VARIANCE_THRESHOLD: f64 = 1e-10;

/// Compute joint probabilities for two correlated Bernoulli random variables.
///
/// Given marginal probabilities p1 and p2 with correlation ρ, returns
/// the four joint probabilities (p11, p10, p01, p00) where:
/// - p11 = P(X₁=1, X₂=1)
/// - p10 = P(X₁=1, X₂=0)
/// - p01 = P(X₁=0, X₂=1)
/// - p00 = P(X₁=0, X₂=0)
///
/// The correlation is automatically clamped to the feasible Fréchet-Hoeffding bounds
/// to ensure valid joint probabilities while exactly preserving the marginals.
///
/// # Arguments
/// * `p1` - Marginal probability P(X₁=1), clamped to [0, 1]
/// * `p2` - Marginal probability P(X₂=1), clamped to [0, 1]
/// * `correlation` - Correlation between X₁ and X₂, clamped to feasible bounds
///
/// # Returns
/// Tuple (p11, p10, p01, p00) that sums to 1.0 and exactly preserves marginals.
///
/// # Example
/// ```
/// use finstack_core::math::probability::joint_probabilities;
///
/// let (p11, p10, p01, p00) = joint_probabilities(0.6, 0.4, 0.3);
/// assert!((p11 + p10 + p01 + p00 - 1.0).abs() < 1e-10);
/// // Marginals are exactly preserved:
/// assert!((p11 + p10 - 0.6).abs() < 1e-10);
/// assert!((p11 + p01 - 0.4).abs() < 1e-10);
/// ```
#[must_use]
pub fn joint_probabilities(p1: f64, p2: f64, correlation: f64) -> (f64, f64, f64, f64) {
    // Clamp marginal probabilities to valid range
    let p1 = p1.clamp(0.0, 1.0);
    let p2 = p2.clamp(0.0, 1.0);

    // Handle degenerate cases (zero variance)
    let var1 = p1 * (1.0 - p1);
    let var2 = p2 * (1.0 - p2);

    if var1 < DEGENERATE_VARIANCE_THRESHOLD || var2 < DEGENERATE_VARIANCE_THRESHOLD {
        // Degenerate case: at least one probability is 0 or 1
        // Return independent joint probabilities (correlation is meaningless)
        return (
            p1 * p2,
            p1 * (1.0 - p2),
            (1.0 - p1) * p2,
            (1.0 - p1) * (1.0 - p2),
        );
    }

    // Clamp correlation to Fréchet-Hoeffding bounds to ensure valid probabilities
    let (rho_min, rho_max) = correlation_bounds(p1, p2);
    let rho = correlation.clamp(rho_min, rho_max);

    // Compute covariance from clamped correlation
    let cov = rho * (var1 * var2).sqrt();

    // Joint probabilities via Bernoulli coupling
    // With correlation clamped to feasible bounds, these are guaranteed to be in [0, 1]
    let p11 = p1 * p2 + cov;
    let p10 = p1 * (1.0 - p2) - cov;
    let p01 = (1.0 - p1) * p2 - cov;
    let p00 = (1.0 - p1) * (1.0 - p2) + cov;

    (p11, p10, p01, p00)
}

/// Correlated Bernoulli distribution for scenario generation.
///
/// Provides methods for working with correlated binary outcomes,
/// useful for tree-based pricing and analytical calculations.
#[derive(Clone, Debug)]
pub struct CorrelatedBernoulli {
    p1: f64,
    p2: f64,
    requested_correlation: f64,
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
    /// The correlation is automatically clamped to the Fréchet-Hoeffding bounds
    /// for the given marginal probabilities to ensure valid joint probabilities.
    ///
    /// # Arguments
    /// * `p1` - Marginal probability of first event, clamped to [0, 1]
    /// * `p2` - Marginal probability of second event, clamped to [0, 1]
    /// * `correlation` - Correlation between events, clamped to feasible bounds
    #[must_use]
    pub fn new(p1: f64, p2: f64, correlation: f64) -> Self {
        let p1 = p1.clamp(0.0, 1.0);
        let p2 = p2.clamp(0.0, 1.0);
        let requested_correlation = correlation.clamp(-1.0, 1.0);
        let (rho_min, rho_max) = correlation_bounds(p1, p2);
        let effective_correlation = requested_correlation.clamp(rho_min, rho_max);
        let (p11, p10, p01, p00) = joint_probabilities(p1, p2, requested_correlation);
        Self {
            p1,
            p2,
            requested_correlation,
            correlation: effective_correlation,
            p11,
            p10,
            p01,
            p00,
        }
    }

    /// Sample a pair of correlated binary outcomes given a uniform random value.
    ///
    /// Returns (x1, x2) where each is 0 or 1.
    ///
    /// # Arguments
    /// * `u` - Uniform random value in [0, 1]
    pub fn sample_from_uniform(&self, u: f64) -> (u8, u8) {
        if u < self.p11 {
            (1, 1)
        } else if u < self.p11 + self.p10 {
            (1, 0)
        } else if u < self.p11 + self.p10 + self.p01 {
            (0, 1)
        } else {
            (0, 0)
        }
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

    /// Get the caller-requested correlation before Fréchet clamping.
    pub fn requested_correlation(&self) -> f64 {
        self.requested_correlation
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
        if self.p1 > 0.0 {
            self.p11 / self.p1
        } else {
            self.p2
        }
    }

    /// Get the conditional probability P(X₁=1 | X₂=1).
    pub fn conditional_p1_given_x2(&self) -> f64 {
        if self.p2 > 0.0 {
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
#[must_use]
pub fn correlation_bounds(p1: f64, p2: f64) -> (f64, f64) {
    let var1 = p1 * (1.0 - p1);
    let var2 = p2 * (1.0 - p2);

    if var1 < DEGENERATE_VARIANCE_THRESHOLD || var2 < DEGENERATE_VARIANCE_THRESHOLD {
        return (0.0, 0.0);
    }

    let std_prod = (var1 * var2).sqrt();

    // Maximum covariance: min(p1, p2) - p1*p2 = min(p1, p2)(1 - max(p1, p2)/min(p1, p2))
    let cov_max = p1.min(p2) - p1 * p2;

    // Minimum covariance: max(p1 + p2 - 1, 0) - p1*p2
    let cov_min = (p1 + p2 - 1.0).max(0.0) - p1 * p2;

    let rho_max = cov_max / std_prod;
    let rho_min = cov_min / std_prod;

    (rho_min.clamp(-1.0, 1.0), rho_max.clamp(-1.0, 1.0))
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
            let (p11, p10, p01, p00) = joint_probabilities(p1, p2, corr);
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
            (0.3, 0.7, 0.5),
            (0.8, 0.2, -0.5),
        ];

        for (p1, p2, corr) in test_cases {
            let (p11, p10, p01, p00) = joint_probabilities(p1, p2, corr);

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

            // Sum must be exactly 1
            let _ = p00;
        }
    }

    #[test]
    fn test_joint_probabilities_extreme_correlations() {
        // Test that extreme correlations are clamped to feasible bounds
        let (p1, p2) = (0.3, 0.7);
        let (rho_min, rho_max) = correlation_bounds(p1, p2);

        // Request correlation beyond bounds - should still produce valid marginals
        let (p11, p10, p01, _p00) = joint_probabilities(p1, p2, 2.0); // Way above max
        assert!((p11 + p10 - p1).abs() < 1e-10);
        assert!((p11 + p01 - p2).abs() < 1e-10);

        let (p11, p10, p01, _p00) = joint_probabilities(p1, p2, -2.0); // Way below min
        assert!((p11 + p10 - p1).abs() < 1e-10);
        assert!((p11 + p01 - p2).abs() < 1e-10);

        // At exact bounds
        let (p11, p10, _, _) = joint_probabilities(p1, p2, rho_max);
        assert!((p11 + p10 - p1).abs() < 1e-10);

        let (p11, p10, _, _) = joint_probabilities(p1, p2, rho_min);
        assert!((p11 + p10 - p1).abs() < 1e-10);
    }

    #[test]
    fn test_joint_probabilities_independent() {
        let (p1, p2) = (0.4, 0.6);
        let (p11, p10, p01, p00) = joint_probabilities(p1, p2, 0.0);

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
        let _ = p10;
        let _ = p01;
    }

    #[test]
    fn test_positive_correlation_increases_joint() {
        let (p1, p2) = (0.5, 0.5);
        let (p11_pos, _, _, _) = joint_probabilities(p1, p2, 0.5);
        let (p11_zero, _, _, _) = joint_probabilities(p1, p2, 0.0);
        let (p11_neg, _, _, _) = joint_probabilities(p1, p2, -0.5);

        // Positive correlation should increase P(both happen)
        assert!(p11_pos > p11_zero);
        assert!(p11_zero > p11_neg);
    }

    #[test]
    fn test_correlated_bernoulli_creation() {
        let dist = CorrelatedBernoulli::new(0.5, 0.5, 0.5);
        assert!((dist.p1() - 0.5).abs() < 1e-10);
        assert!((dist.p2() - 0.5).abs() < 1e-10);
        assert!((dist.correlation() - 0.5).abs() < 1e-10);
        assert!((dist.requested_correlation() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_correlated_bernoulli_reports_effective_correlation() {
        let dist = CorrelatedBernoulli::new(0.05, 0.95, 0.9);
        assert!(dist.requested_correlation() > dist.correlation());
    }

    #[test]
    fn test_sample_from_uniform() {
        let dist = CorrelatedBernoulli::new(0.5, 0.5, 0.0);

        // At u=0, should get (1, 1)
        let (x1, x2) = dist.sample_from_uniform(0.0);
        assert_eq!((x1, x2), (1, 1));

        // At u=0.99, should get (0, 0)
        let (x1, x2) = dist.sample_from_uniform(0.99);
        assert_eq!((x1, x2), (0, 0));
    }

    #[test]
    fn test_sample_from_uniform_covers_all_joint_buckets() {
        let dist = CorrelatedBernoulli::new(0.5, 0.5, 0.0);

        assert_eq!(dist.joint_probabilities(), (0.25, 0.25, 0.25, 0.25));
        assert_eq!(dist.sample_from_uniform(0.125), (1, 1));
        assert_eq!(dist.sample_from_uniform(0.375), (1, 0));
        assert_eq!(dist.sample_from_uniform(0.625), (0, 1));
        assert_eq!(dist.sample_from_uniform(0.875), (0, 0));
    }

    #[test]
    fn test_correlation_bounds() {
        let (rho_min, rho_max) = correlation_bounds(0.5, 0.5);

        // For p1=p2=0.5, bounds should be symmetric around 0
        assert!(rho_min < 0.0);
        assert!(rho_max > 0.0);

        // Perfect correlation possible when p1=p2=0.5
        assert!((rho_max - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_conditional_probabilities() {
        let dist = CorrelatedBernoulli::new(0.6, 0.4, 0.5);

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
        let dist = CorrelatedBernoulli::new(0.6, 0.4, 0.3);
        let (p11, p10, p01, p00) = dist.joint_probabilities();

        assert!((p11 + p10 + p01 + p00 - 1.0).abs() < 1e-10);
        assert_eq!(p11, dist.joint_p11());
        assert_eq!(p10, dist.joint_p10());
        assert_eq!(p01, dist.joint_p01());
        assert_eq!(p00, dist.joint_p00());
    }
}
