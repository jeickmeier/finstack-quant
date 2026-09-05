//! Probability distribution functions and sampling algorithms.
//!
//! Provides implementations of discrete and continuous probability distributions
//! used in financial modeling, risk management, and Monte Carlo simulations.
//! All implementations use numerically stable algorithms.
//!
//! # Distributions
//!
//! ## Discrete Distributions
//! - **Binomial**: Binary outcomes (coin flips, defaults)
//!
//! ## Continuous Distributions
//! - **Gamma**: Shape-scale family, variance modeling
//! - **Beta**: Bounded \[0,1\] values (recovery rates, correlations)
//! - **Chi-Squared**: Variance estimation, CIR model, hypothesis testing
//!
//! # Numerical Stability
//!
//! - Log-space calculations prevent overflow for large parameters
//! - Stirling's approximation for factorials when n ≥ 20
//! - Defensive checks for boundary conditions (p=0, p=1, k>n)
//! - Uses battle-tested `statrs` crate for PDF/CDF implementations
//!
//! # Use Cases
//!
//! - **Credit modeling**: Binomial for defaults in a homogeneous pool
//! - **Recovery simulation**: Beta for recovery-rate uncertainty
//! - **Interest rates**: Chi-Squared for CIR model variance process
//! - **Bayesian inference**: Beta as conjugate prior for Bernoulli
//!
//! # Examples
//!
//! ## Binomial probability calculation
//!
//! ```
//! use finstack_quant_core::math::distributions::binomial_probability;
//!
//! // Calculate P(X = 5) where X ~ Binomial(10, 0.5)
//! let prob = binomial_probability(10, 5, 0.5);
//! assert!((prob - 0.24609375).abs() < 1e-6);
//! ```
//!
//! ## Beta sampling for recovery-rate uncertainty
//!
//! ```
//! use finstack_quant_core::math::distributions::sample_beta;
//! use finstack_quant_core::math::random::Pcg64Rng;
//! use finstack_quant_core::math::RandomNumberGenerator;
//!
//! let mut rng = Pcg64Rng::new(42);
//!
//! // Recovery rate ~ Beta(4, 2), peaked around 65%
//! let recovery = sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 4.0, 2.0)?;
//! assert!((0.0..=1.0).contains(&recovery));
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! ## Chi-squared quantile for CIR variance
//!
//! ```
//! use finstack_quant_core::math::distributions::chi_squared_quantile;
//!
//! // 95th percentile with one degree of freedom.
//! let x_95 = chi_squared_quantile(0.95, 1.0)?;
//! assert!((x_95 - 3.841).abs() < 0.01);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # References
//!
//! - **Binomial Distribution**:
//!   - Johnson, N. L., Kotz, S., & Kemp, A. W. (1993). *Univariate Discrete Distributions*
//!     (2nd ed.). Wiley. Chapter 3. `docs/REFERENCES.md#press-numerical-recipes`
//!
//! - **Continuous Distributions**:
//!   - Johnson, N. L., Kotz, S., & Balakrishnan, N. (1994, 1995). *Continuous Univariate
//!     Distributions, Volumes 1 & 2* (2nd ed.). Wiley. `docs/REFERENCES.md#press-numerical-recipes`
//!
//! - **Gamma Sampling**:
//!   - Marsaglia, G., & Tsang, W. W. (2000). "A Simple Method for Generating Gamma
//!     Variables." *ACM Transactions on Mathematical Software*, 26(3), 363-372. `docs/REFERENCES.md#press-numerical-recipes`

use super::random::RandomNumberGenerator;

/// Generate the complete binomial distribution P(X=k) for k = 0, 1, ..., n.
///
/// Returns a normalized probability vector where `dist[k]` = P(X = k).
/// Uses log-space arithmetic to prevent overflow for large n.
///
/// # Mathematical Definition
///
/// ```text
/// dist[k] = P(X = k) = C(n,k) * p^k * (1-p)^(n-k)
/// ```
///
/// # Arguments
///
/// * `n` - Number of independent trials (≥ 0)
/// * `p` - Probability of success on each trial (0 ≤ p ≤ 1)
///
/// # Returns
///
/// Vector of probabilities `[P(X=0), P(X=1), ..., P(X=n)]` with length n+1.
/// The vector sums to 1.0 (normalized).
///
/// # Use Cases
///
/// - **Credit modeling**: Loss distribution for homogeneous pool of n obligors
/// - **Portfolio analytics**: Number of defaults given conditional default probability
/// - **Structured credit**: Default distribution for CDO/CLO tranches
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if `p` is NaN.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::binomial_distribution;
///
/// // Fair coin: distribution of heads in 10 flips
/// let dist = binomial_distribution(10, 0.5).unwrap();
/// assert_eq!(dist.len(), 11); // P(X=0), P(X=1), ..., P(X=10)
/// assert!((dist[5] - 0.24609375).abs() < 1e-6); // P(X=5)
///
/// // Credit portfolio: default distribution with 5% PD
/// let loss_dist = binomial_distribution(100, 0.05).unwrap();
/// assert_eq!(loss_dist.len(), 101);
/// // Most probability mass around 5 defaults
/// assert!(loss_dist[5] > loss_dist[0]);
/// assert!(loss_dist[5] > loss_dist[20]);
/// ```
///
/// # References
///
/// - Johnson, N. L., Kotz, S., & Kemp, A. W. (1993). *Univariate Discrete Distributions*
///   (2nd ed.). Wiley. Chapter 3. `docs/REFERENCES.md#press-numerical-recipes`
pub fn binomial_distribution(n: usize, p: f64) -> crate::Result<Vec<f64>> {
    use statrs::distribution::{Binomial, Discrete};

    // NaN must surface as a validation error, not a silent point mass at 0.
    if p.is_nan() {
        return Err(crate::Error::Validation(
            "binomial_distribution: success probability p must not be NaN".to_string(),
        ));
    }

    if p <= 0.0 {
        // All probability on k=0
        let mut dist = vec![0.0; n + 1];
        dist[0] = 1.0;
        return Ok(dist);
    }
    if p >= 1.0 {
        // All probability on k=n
        let mut dist = vec![0.0; n + 1];
        dist[n] = 1.0;
        return Ok(dist);
    }

    let mut dist = Binomial::new(p, n as u64)
        .map(|binom| (0..=n as u64).map(|k| binom.pmf(k)).collect::<Vec<_>>())
        .map_err(|e| {
            crate::Error::Validation(format!(
                "binomial_distribution: invalid parameters n={n}, p={p}: {e}"
            ))
        })?;

    // Renormalize when floating-point PMF mass drifts off 1.
    let sum: f64 = dist.iter().sum();
    if sum > 0.0 && (sum - 1.0).abs() > 1e-10 {
        for prob in &mut dist {
            *prob /= sum;
        }
    }
    Ok(dist)
}

/// Calculate binomial probability P(X = k) where X ~ Binomial(n, p).
///
/// Computes the probability mass function for the binomial distribution using
/// the battle-tested `statrs` crate implementation. The binomial distribution
/// models the number of successes in n independent Bernoulli trials.
///
/// # Mathematical Definition
///
/// ```text
/// P(X = k) = C(n,k) * p^k * (1-p)^(n-k)
///
/// where C(n,k) = n! / (k! * (n-k)!)
/// ```
///
/// # Arguments
///
/// * `n` - Number of independent trials (≥ 0)
/// * `k` - Number of successes (0 ≤ k ≤ n)
/// * `p` - Probability of success on each trial (0 ≤ p ≤ 1)
///
/// # Returns
///
/// Probability P(X = k) ∈ [0, 1]
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::binomial_probability;
///
/// // Fair coin: P(5 heads in 10 flips)
/// let prob = binomial_probability(10, 5, 0.5);
/// assert!((prob - 0.24609375).abs() < 1e-6);
///
/// // Credit portfolio: P(5 defaults in 100 names with 5% PD)
/// let default_prob = binomial_probability(100, 5, 0.05);
/// ```
///
/// # Implementation
///
/// This is a thin wrapper around `statrs::distribution::Binomial::pmf`, which
/// provides numerically stable computation with proper edge case handling.
///
/// # References
///
/// - Johnson, N. L., Kotz, S., & Kemp, A. W. (1993). *Univariate Discrete Distributions*
///   (2nd ed.). Wiley. Chapter 3. `docs/REFERENCES.md#press-numerical-recipes`
pub fn binomial_probability(n: usize, k: usize, p: f64) -> f64 {
    use statrs::distribution::{Binomial, Discrete};

    if k > n {
        return 0.0;
    }
    if p <= 0.0 {
        return if k == 0 { 1.0 } else { 0.0 };
    }
    if p >= 1.0 {
        return if k == n { 1.0 } else { 0.0 };
    }

    // statrs::distribution::Binomial::new(p, n) where p is success probability and n is trials
    match Binomial::new(p, n as u64) {
        Ok(binom) => binom.pmf(k as u64),
        Err(_) => 0.0,
    }
}

/// Compute the full binomial PMF `P(K = k)` for every `k` in `0..=n` in one pass.
///
/// Returns a vector `pmf` of length `n + 1` where `pmf[k]` is the probability of
/// exactly `k` successes in `n` independent trials each with success probability
/// `p`. This is mathematically equivalent to calling [`binomial_probability`] for
/// every `k`, but evaluates the whole distribution in `O(n)` arithmetic via the
/// forward recurrence
///
/// ```text
/// P(0) = (1 - p)^n
/// P(k + 1) = P(k) · (n - k) / (k + 1) · p / (1 - p)
/// ```
///
/// instead of constructing `n + 1` separate distributions. Prefer this when an
/// algorithm needs the PMF across a range of `k` for fixed `(n, p)` — for example
/// summing a portfolio loss over the number of defaults.
///
/// # Arguments
///
/// * `n` - Number of trials.
/// * `p` - Success probability. Values `<= 0` and `>= 1` collapse to the
///   degenerate distributions (all mass on `k = 0` and `k = n` respectively),
///   matching [`binomial_probability`].
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::{binomial_pmf_all, binomial_probability};
///
/// let pmf = binomial_pmf_all(10, 0.3)?;
/// assert_eq!(pmf.len(), 11);
/// for k in 0..=10 {
///     assert!((pmf[k] - binomial_probability(10, k, 0.3)).abs() < 1e-12);
/// }
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `p` is NaN.
///
/// # References
///
/// - Johnson, N. L., Kotz, S., & Kemp, A. W. (1993). *Univariate Discrete Distributions*
///   (2nd ed.). Wiley. Chapter 3. `docs/REFERENCES.md#press-numerical-recipes`
pub fn binomial_pmf_all(n: usize, p: f64) -> crate::Result<Vec<f64>> {
    let mut pmf = Vec::new();
    binomial_pmf_all_into(&mut pmf, n, p)?;
    Ok(pmf)
}

/// In-place variant of [`binomial_pmf_all`] that writes the PMF into `out`
/// (cleared and resized to `n + 1`) rather than allocating a fresh vector.
///
/// Hot loops that evaluate the binomial PMF many times — e.g. a CDS-tranche
/// factor-quadrature integrand, which calls this once per Gauss-Hermite node
/// per payment date — can pass a reusable scratch buffer to avoid an
/// allocation on every evaluation.
///
/// # Arguments
///
/// * `out` - Reusable output buffer cleared and resized to `n + 1`, then filled
///   with probabilities for zero through `n` successes.
/// * `n` - Number of independent Bernoulli trials.
/// * `p` - Per-trial success probability. Values at or beyond `0.0` and `1.0`
///   produce the corresponding degenerate distribution.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `p` is NaN. The output buffer is
/// left unchanged when validation fails.
pub fn binomial_pmf_all_into(out: &mut Vec<f64>, n: usize, p: f64) -> crate::Result<()> {
    if p.is_nan() {
        return Err(crate::Error::Validation(
            "binomial_pmf_all: success probability p must not be NaN".to_string(),
        ));
    }

    out.clear();
    out.resize(n + 1, 0.0);
    if p <= 0.0 {
        out[0] = 1.0;
        return Ok(());
    }
    if p >= 1.0 {
        out[n] = 1.0;
        return Ok(());
    }

    let log_ratio = p.ln() - (1.0 - p).ln();
    let mut log_prob = n as f64 * (1.0 - p).ln();
    out[0] = log_prob.exp();
    for k in 0..n {
        log_prob += ((n - k) as f64).ln() - ((k + 1) as f64).ln() + log_ratio;
        out[k + 1] = log_prob.exp();
    }
    Ok(())
}

/// Calculate log factorial ln(n!) with automatic method selection.
///
/// Uses exact calculation for small n and Stirling's approximation for large n
/// to balance accuracy and numerical stability.
///
/// # Algorithm
///
/// - **n < 20**: Exact via Σ ln(i) for i = 2..n
/// - **n ≥ 20**: Stirling's approximation
///
/// # Arguments
///
/// * `n` - Non-negative integer
///
/// # Returns
///
/// ln(n!)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::log_factorial;
///
/// assert_eq!(log_factorial(0), 0.0); // 0! = 1, ln(1) = 0
/// assert!((log_factorial(5) - (2.0_f64.ln() + 3.0_f64.ln() + 4.0_f64.ln() + 5.0_f64.ln())).abs() < 1e-10);
/// ```
pub fn log_factorial(n: usize) -> f64 {
    statrs::function::factorial::ln_factorial(n as u64)
}

/// Sample from Beta(α, β) distribution using the gamma ratio method.
///
/// Generates random samples from the Beta distribution, commonly used for
/// modeling random variables constrained to \[0,1\] such as recovery rates,
/// default correlations, and prepayment rates.
///
/// # Distribution Properties
///
/// ```text
/// Beta(α, β) with α, β > 0:
/// - Support: [0, 1]
/// - Mean: α / (α + β)
/// - Mode: (α - 1) / (α + β - 2)  for α, β > 1
///
/// Shape parameter effects:
/// - α = β = 1: Uniform[0,1]
/// - α > β: Right-skewed (mode near 1)
/// - α < β: Left-skewed (mode near 0)
/// - α = β > 1: Symmetric, bell-shaped
///   ```
///
/// # Arguments
///
/// * `rng` - Random number generator implementing [`RandomNumberGenerator`]
/// * `alpha` - First shape parameter (α > 0)
/// * `beta` - Second shape parameter (β > 0)
///
/// # Returns
///
/// Random sample x ∈ [0, 1] from Beta(α, β)
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if α ≤ 0 or β ≤ 0.
///
/// # Algorithm
///
/// Uses the gamma ratio method (Devroye, 1986):
/// If X ~ Gamma(α, 1) and Y ~ Gamma(β, 1), then X/(X+Y) ~ Beta(α, β)
///
/// Gamma samples are generated using Marsaglia & Tsang's method for shape ≥ 1,
/// with Ahrens-Dieter transformation for shape < 1.
///
/// # Use Cases
///
/// - **Recovery rates**: Beta(4, 2) models senior unsecured recovery ~60-70%
/// - **Default correlation**: Beta(2, 5) for low but uncertain correlation
/// - **Prepayment rates**: Beta shapes for mortgage prepayment speed
/// - **Bayesian priors**: Conjugate prior for Bernoulli/binomial likelihood
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::sample_beta;
/// use finstack_quant_core::math::random::Pcg64Rng;
/// use finstack_quant_core::math::RandomNumberGenerator;
///
/// let mut rng = Pcg64Rng::new(42);
///
/// // Sample recovery rate: Beta(4, 2) peaked around 65%
/// let recovery = sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 4.0, 2.0)?;
/// assert!(recovery >= 0.0 && recovery <= 1.0);
///
/// // Uniform distribution: Beta(1, 1)
/// let uniform = sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 1.0, 1.0)?;
/// assert!(uniform >= 0.0 && uniform <= 1.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # References
///
/// - Johnson, N. L., Kotz, S., & Balakrishnan, N. (1995). *Continuous Univariate
///   Distributions, Volume 2* (2nd ed.). Wiley. Chapter 25 (Beta distribution). `docs/REFERENCES.md#press-numerical-recipes`
/// - Devroye, L. (1986). *Non-Uniform Random Variate Generation*. Springer.
///   Chapter 9 (Beta distribution sampling via gamma ratio).
/// - Marsaglia, G., & Tsang, W. W. (2000). "A Simple Method for Generating Gamma
///   Variables." *ACM Transactions on Mathematical Software*, 26(3), 363-372. `docs/REFERENCES.md#press-numerical-recipes`
pub fn sample_beta(
    rng: &mut dyn RandomNumberGenerator,
    alpha: f64,
    beta: f64,
) -> crate::Result<f64> {
    if !alpha.is_finite() || alpha <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Beta α parameter must be positive, got: {}",
            alpha
        )));
    }
    if !beta.is_finite() || beta <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Beta β parameter must be positive, got: {}",
            beta
        )));
    }

    // Special case: Beta(1, 1) = Uniform[0, 1]
    // Exact comparison: checking for exact caller-supplied parameter values.
    #[allow(clippy::float_cmp)]
    if alpha == 1.0 && beta == 1.0 {
        return Ok(rng.uniform());
    }

    // Use gamma ratio method: X/(X+Y) ~ Beta(α, β) where X ~ Gamma(α), Y ~ Gamma(β)
    let x = sample_gamma_unchecked(rng, alpha)?;
    let y = sample_gamma_unchecked(rng, beta)?;

    // Guard against division by zero or near-zero denominator.
    // Both gamma samples can underflow to 0 for very small shape parameters.
    // In the α, β → 0 limit Beta(α, β) converges to the two-point
    // distribution on {0, 1} with P(1) = α/(α+β) (Johnson, Kotz &
    // Balakrishnan 1995, Ch. 25), so sample that limit rather than
    // collapsing to the mean 0.5.
    let sum = x + y;
    if !sum.is_finite() || sum <= 0.0 {
        let p_one = alpha / (alpha + beta);
        return Ok(if rng.uniform() < p_one { 1.0 } else { 0.0 });
    }
    Ok(x / sum)
}

/// Sample from Gamma(shape, 1) distribution using Marsaglia-Tsang method.
///
/// Generates random samples from the Gamma distribution with shape parameter α
/// and rate parameter 1. For Gamma(α, β), multiply the result by 1/β.
///
/// # Arguments
///
/// * `rng` - Random number generator
/// * `shape` - Shape parameter (α > 0)
///
/// # Returns
///
/// Random sample from Gamma(shape, 1)
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if shape ≤ 0.
///
/// # Algorithm
///
/// Uses Marsaglia & Tsang's rejection method for shape ≥ 1, with Ahrens-Dieter
/// transformation for shape < 1.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::sample_gamma;
/// use finstack_quant_core::math::random::Pcg64Rng;
/// use finstack_quant_core::math::RandomNumberGenerator;
///
/// let mut rng = Pcg64Rng::new(42);
/// let sample = sample_gamma(&mut rng as &mut dyn RandomNumberGenerator, 2.0)?;
/// assert!(sample >= 0.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # References
///
/// - Marsaglia, G., & Tsang, W. W. (2000). "A Simple Method for Generating Gamma
///   Variables." *ACM Transactions on Mathematical Software*, 26(3), 363-372. `docs/REFERENCES.md#press-numerical-recipes`
pub fn sample_gamma(rng: &mut dyn RandomNumberGenerator, shape: f64) -> crate::Result<f64> {
    if !shape.is_finite() || shape <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Gamma shape parameter must be positive, got: {}",
            shape
        )));
    }

    sample_gamma_unchecked(rng, shape)
}

const GAMMA_MAX_REJECTION_ATTEMPTS: usize = 100_000;

/// Internal unchecked gamma sampling (assumes shape > 0).
fn sample_gamma_unchecked(rng: &mut dyn RandomNumberGenerator, shape: f64) -> crate::Result<f64> {
    sample_gamma_with_max_attempts(rng, shape, GAMMA_MAX_REJECTION_ATTEMPTS)
}

fn sample_gamma_with_max_attempts(
    rng: &mut dyn RandomNumberGenerator,
    shape: f64,
    max_attempts: usize,
) -> crate::Result<f64> {
    if shape < 1.0 {
        // Ahrens-Dieter transformation for shape < 1:
        // If X ~ Gamma(shape + 1), then X * U^(1/shape) ~ Gamma(shape)
        let u = rng.uniform();
        // Clamp u away from 0 to prevent ln(0) issues
        let u_safe = u.max(1e-300);
        return Ok(
            sample_gamma_with_max_attempts(rng, shape + 1.0, max_attempts)?
                * u_safe.powf(1.0 / shape),
        );
    }

    // Marsaglia-Tsang method for shape >= 1
    let d = shape - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();

    for _ in 0..max_attempts {
        // Generate normal variate using Box-Muller
        let x = rng.normal(0.0, 1.0);
        let v = 1.0 + c * x;

        if v > 0.0 {
            let v = v * v * v; // v^3
            let u = rng.uniform();
            let x2 = x * x;

            // Squeeze test (fast accept)
            if u < 1.0 - 0.0331 * x2 * x2 {
                return Ok(d * v);
            }

            // Full rejection test
            // Clamp u and v away from 0 to prevent ln(0)
            let u_safe = u.max(1e-300);
            let v_safe = v.max(1e-300);
            if u_safe.ln() < 0.5 * x2 + d * (1.0 - v_safe + v_safe.ln()) {
                return Ok(d * v);
            }
        }
        // Reject and retry
    }
    Err(crate::Error::Validation(format!(
        "Gamma rejection sampler exceeded {max_attempts} attempts"
    )))
}

/// Quantile function (inverse CDF) of the Chi-Squared distribution.
///
/// Returns the value x such that P(X ≤ x) = p.
///
/// # Arguments
///
/// * `p` - Probability in [0, 1)
/// * `df` - Degrees of freedom (k > 0)
///
/// # Returns
///
/// Quantile x such that F(x; k) = p
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if:
/// - p ∉ [0, 1)
/// - df ≤ 0
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::chi_squared_quantile;
///
/// // 95th percentile for df=1 is approximately 3.841
/// let x_95 = chi_squared_quantile(0.95, 1.0)?;
/// assert!((x_95 - 3.841).abs() < 0.01);
///
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn chi_squared_quantile(p: f64, df: f64) -> crate::Result<f64> {
    use statrs::distribution::{ChiSquared, ContinuousCDF};

    if !(0.0..1.0).contains(&p) {
        return Err(crate::Error::Validation(format!(
            "Probability p must be in [0, 1), got: {}",
            p
        )));
    }
    if df <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Chi-squared degrees of freedom must be positive, got: {}",
            df
        )));
    }

    match ChiSquared::new(df) {
        Ok(chi2) => Ok(chi2.inverse_cdf(p)),
        Err(_) => Err(crate::Error::Validation(
            "Failed to create chi-squared distribution".to_string(),
        )),
    }
}

// Student's t Distribution (Sampler)

#[cfg(test)]
mod tests {
    use super::*;

    struct RejectingRng;

    impl RandomNumberGenerator for RejectingRng {
        fn uniform(&mut self) -> f64 {
            0.5
        }

        fn normal(&mut self, _mean: f64, _std_dev: f64) -> f64 {
            -10.0
        }

        fn bernoulli(&mut self, _p: f64) -> bool {
            false
        }
    }

    #[test]
    fn gamma_rejection_loop_has_a_hard_limit() {
        let mut rng = RejectingRng;
        let error = sample_gamma_with_max_attempts(&mut rng, 1.0, 3)
            .expect_err("deterministic rejection must terminate");
        assert!(error.to_string().contains("attempt"));
    }

    #[test]
    fn test_binomial_probability() {
        // Test known values
        assert!((binomial_probability(10, 5, 0.5) - 0.24609375).abs() < 1e-6);
        assert!((binomial_probability(5, 0, 0.1) - 0.59049).abs() < 1e-6);

        // Test edge cases
        assert_eq!(binomial_probability(10, 0, 0.0), 1.0);
        assert_eq!(binomial_probability(10, 10, 1.0), 1.0);
        assert_eq!(binomial_probability(10, 5, 0.0), 0.0);
    }

    #[test]
    fn test_log_factorial() {
        // Test small values (exact calculation)
        assert!((log_factorial(1) - 0.0).abs() < 1e-12);
        assert!(
            (log_factorial(5) - (2.0_f64.ln() + 3.0_f64.ln() + 4.0_f64.ln() + 5.0_f64.ln())).abs()
                < 1e-12
        );

        // Test large values (Stirling approximation)
        let log_100_factorial = log_factorial(100);
        assert!(log_100_factorial > 360.0 && log_100_factorial < 365.0);
    }

    #[test]
    fn test_sample_beta() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

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
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(12345);
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
        use super::super::random::Pcg64Rng;

        // Test with shape parameters < 1 (uses Ahrens-Dieter transformation)
        let mut rng = Pcg64Rng::new(9999);
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
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

        // Invalid alpha
        assert!(sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 0.0, 1.0).is_err());
        assert!(sample_beta(&mut rng as &mut dyn RandomNumberGenerator, -1.0, 1.0).is_err());

        // Invalid beta
        assert!(sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 1.0, 0.0).is_err());
        assert!(sample_beta(&mut rng as &mut dyn RandomNumberGenerator, 1.0, -1.0).is_err());
    }

    #[test]
    fn test_binomial_distribution() {
        // Test basic distribution
        let dist = binomial_distribution(10, 0.5).unwrap();
        assert_eq!(dist.len(), 11);

        // Test P(X=5) for fair coin
        assert!(
            (dist[5] - 0.24609375).abs() < 1e-6,
            "P(X=5) = {}, expected 0.24609375",
            dist[5]
        );

        // Test normalization
        let sum: f64 = dist.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "Distribution sum = {}, expected 1.0",
            sum
        );

        // Test symmetry for p=0.5
        for k in 0..=5 {
            assert!(
                (dist[k] - dist[10 - k]).abs() < 1e-10,
                "P({}) = {} should equal P({}) = {}",
                k,
                dist[k],
                10 - k,
                dist[10 - k]
            );
        }
    }

    #[test]
    fn test_binomial_distribution_edge_cases() {
        // p = 0: all probability on k=0
        let dist_zero = binomial_distribution(5, 0.0).unwrap();
        assert!((dist_zero[0] - 1.0).abs() < 1e-10);
        for val in dist_zero.iter().skip(1) {
            assert!(*val < 1e-10);
        }

        // p = 1: all probability on k=n
        let dist_one = binomial_distribution(5, 1.0).unwrap();
        assert!((dist_one[5] - 1.0).abs() < 1e-10);
        for val in dist_one.iter().take(5) {
            assert!(*val < 1e-10);
        }

        // n = 0: single element
        let dist_n0 = binomial_distribution(0, 0.5).unwrap();
        assert_eq!(dist_n0.len(), 1);
        assert!((dist_n0[0] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_binomial_distribution_rejects_nan_p() {
        assert!(binomial_distribution(10, f64::NAN).is_err());
    }

    #[test]
    fn test_binomial_distribution_credit_portfolio() {
        // Typical credit portfolio: 100 names with 5% PD
        let dist = binomial_distribution(100, 0.05).unwrap();
        assert_eq!(dist.len(), 101);

        // Expected number of defaults = n * p = 5
        // Most probability mass should be around 5
        let expected_mean: f64 = (0..=100).map(|k| k as f64 * dist[k]).sum();
        assert!(
            (expected_mean - 5.0).abs() < 0.01,
            "Mean = {}, expected ~5.0",
            expected_mean
        );

        // Variance = n * p * (1-p) = 4.75
        let expected_var: f64 = (0..=100)
            .map(|k| (k as f64 - expected_mean).powi(2) * dist[k])
            .sum();
        assert!(
            (expected_var - 4.75).abs() < 0.01,
            "Variance = {}, expected ~4.75",
            expected_var
        );
    }

    // Exponential Distribution Tests

    // Log-Normal Distribution Tests

    #[test]
    fn binomial_pmf_all_matches_binomial_probability() {
        for &(n, p) in &[(10usize, 0.3f64), (50, 0.05), (126, 0.5), (20, 0.9)] {
            let pmf = binomial_pmf_all(n, p).expect("valid probability should produce a PMF");
            assert_eq!(pmf.len(), n + 1);
            for (k, &prob) in pmf.iter().enumerate() {
                assert!(
                    (prob - binomial_probability(n, k, p)).abs() < 1e-10,
                    "n={n}, k={k}, p={p}: recurrence {} vs statrs {}",
                    prob,
                    binomial_probability(n, k, p)
                );
            }
            let total: f64 = pmf.iter().sum();
            assert!((total - 1.0).abs() < 1e-9, "n={n}, p={p}: pmf sum={total}");
        }
    }

    #[test]
    fn binomial_pmf_all_large_n_keeps_probability_mass() {
        let pmf = binomial_pmf_all(2_000, 0.5).expect("valid probability should produce a PMF");
        let sum: f64 = pmf.iter().sum();
        let mode = pmf[1_000];

        assert!(
            (sum - 1.0).abs() < 1e-10,
            "large-n PMF must sum to 1, got {sum}"
        );
        assert!(
            mode > 0.0,
            "mode probability should be positive for n=2000, p=0.5"
        );
    }

    #[test]
    fn binomial_pmf_all_handles_degenerate_p() {
        let lo = binomial_pmf_all(5, 0.0).expect("zero should produce a degenerate PMF");
        assert_eq!(lo[0], 1.0);
        assert!(lo[1..].iter().all(|&x| x == 0.0));

        let hi = binomial_pmf_all(5, 1.0).expect("one should produce a degenerate PMF");
        assert_eq!(hi[5], 1.0);
        assert!(hi[..5].iter().all(|&x| x == 0.0));
    }

    #[test]
    fn binomial_pmf_all_rejects_nan_without_mutating_scratch() {
        let error = binomial_pmf_all(5, f64::NAN).expect_err("NaN must be rejected");
        assert!(matches!(error, crate::Error::Validation(_)));

        let mut scratch = vec![0.25, 0.75];
        let error = binomial_pmf_all_into(&mut scratch, 5, f64::NAN)
            .expect_err("in-place NaN must be rejected");
        assert!(matches!(error, crate::Error::Validation(_)));
        assert_eq!(scratch, vec![0.25, 0.75]);
    }

    // Gamma Distribution Tests

    #[test]
    fn test_sample_gamma_basic() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

        // All samples should be non-negative
        for _ in 0..100 {
            let x = sample_gamma(&mut rng as &mut dyn RandomNumberGenerator, 2.0)
                .expect("Gamma(2.0) should succeed");
            assert!(x >= 0.0, "Gamma sample should be non-negative, got {}", x);
        }
    }

    #[test]
    fn test_sample_gamma_small_shape() {
        use super::super::random::Pcg64Rng;

        // Test with shape < 1 (uses Ahrens-Dieter transformation)
        let mut rng = Pcg64Rng::new(42);

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
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(12345);
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
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

        // Invalid shape
        assert!(sample_gamma(&mut rng as &mut dyn RandomNumberGenerator, 0.0).is_err());
        assert!(sample_gamma(&mut rng as &mut dyn RandomNumberGenerator, -1.0).is_err());
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(sample_gamma(&mut rng, invalid).is_err());
            assert!(sample_beta(&mut rng, invalid, 1.0).is_err());
            assert!(sample_beta(&mut rng, 1.0, invalid).is_err());
        }
    }

    // Chi-Squared Distribution Tests

    #[test]
    fn test_chi_squared_quantile_roundtrip() {
        let df = 5.0;
        let test_probs = [0.1, 0.25, 0.5, 0.75, 0.9];

        for &p in &test_probs {
            let x = chi_squared_quantile(p, df).expect("Valid p and df");
            let p_back = {
                use statrs::distribution::{ChiSquared, ContinuousCDF};
                ChiSquared::new(df).expect("valid df").cdf(x)
            };
            assert!(
                (p - p_back).abs() < 1e-10,
                "Round-trip failed for p={}, df={}, got x={}, p_back={}",
                p,
                df,
                x,
                p_back
            );
        }
    }

    #[test]
    fn test_chi_squared_quantile_validation() {
        // Invalid p
        assert!(chi_squared_quantile(-0.1, 5.0).is_err());
        assert!(chi_squared_quantile(1.0, 5.0).is_err());
        assert!(chi_squared_quantile(1.5, 5.0).is_err());

        // Invalid df
        assert!(chi_squared_quantile(0.5, 0.0).is_err());
        assert!(chi_squared_quantile(0.5, -1.0).is_err());
    }

    // Student's t Distribution Tests
}
