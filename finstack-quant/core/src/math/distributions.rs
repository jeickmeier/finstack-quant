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
//! - **Exponential**: Inter-arrival times, default timing, hazard rates
//! - **Gamma**: Shape-scale family, variance modeling
//! - **Beta**: Bounded \[0,1\] values (recovery rates, correlations)
//! - **Log-Normal**: Positive-valued quantities (asset prices, LGD)
//! - **Chi-Squared**: Variance estimation, CIR model, hypothesis testing
//! - **Student's t**: Fat-tailed returns, robust statistics
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
//! - **Credit modeling**: Binomial for defaults, Exponential for default timing
//! - **Recovery simulation**: Beta and Log-Normal for recovery rate uncertainty
//! - **Asset pricing**: Log-Normal for GBM price simulation
//! - **Risk metrics**: Student's t for fat-tailed VaR/ES
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
//! ## Exponential distribution for default timing
//!
//! ```
//! use finstack_quant_core::math::distributions::sample_exponential;
//! use finstack_quant_core::math::random::Pcg64Rng;
//! use finstack_quant_core::math::RandomNumberGenerator;
//!
//! let mut rng = Pcg64Rng::new(42);
//!
//! // Sample default time with 5% annual hazard rate (expected: 20 years)
//! let default_time = sample_exponential(&mut rng as &mut dyn RandomNumberGenerator, 0.05)?;
//! assert!(default_time >= 0.0);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! ## Log-Normal for asset price simulation
//!
//! ```
//! use finstack_quant_core::math::distributions::sample_lognormal;
//! use finstack_quant_core::math::random::Pcg64Rng;
//! use finstack_quant_core::math::RandomNumberGenerator;
//!
//! let mut rng = Pcg64Rng::new(42);
//!
//! // Sample price factor: S_T = S_0 * exp((μ-σ²/2)T + σ√T Z)
//! let price_factor = sample_lognormal(&mut rng as &mut dyn RandomNumberGenerator, 0.0, 0.2)?;
//! assert!(price_factor > 0.0);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! ## Student's t for heavy-tailed simulation
//!
//! ```
//! use finstack_quant_core::math::distributions::sample_student_t;
//! use finstack_quant_core::math::random::Pcg64Rng;
//! use finstack_quant_core::math::RandomNumberGenerator;
//!
//! let mut rng = Pcg64Rng::new(42);
//!
//! // Sample from t(5) - heavier tails than Normal
//! let t = sample_student_t(&mut rng as &mut dyn RandomNumberGenerator, 5.0)?;
//! assert!(t.is_finite());
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # References
//!
//! - **Binomial Distribution**:
//!   - Johnson, N. L., Kotz, S., & Kemp, A. W. (1993). *Univariate Discrete Distributions*
//!     (2nd ed.). Wiley. Chapter 3.
//!
//! - **Continuous Distributions**:
//!   - Johnson, N. L., Kotz, S., & Balakrishnan, N. (1994, 1995). *Continuous Univariate
//!     Distributions, Volumes 1 & 2* (2nd ed.). Wiley.
//!
//! - **Gamma Sampling**:
//!   - Marsaglia, G., & Tsang, W. W. (2000). "A Simple Method for Generating Gamma
//!     Variables." *ACM Transactions on Mathematical Software*, 26(3), 363-372.

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
///   (2nd ed.). Wiley. Chapter 3.
pub fn binomial_distribution(n: usize, p: f64) -> crate::Result<Vec<f64>> {
    use statrs::distribution::{Binomial, Discrete};

    // NaN must surface as a validation error, not a silent point mass at 0.
    if p.is_nan() {
        return Err(crate::Error::Validation(
            "binomial_distribution: success probability p must not be NaN".to_string(),
        ));
    }

    // Handle edge cases that would require special treatment
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

    // Create the Binomial distribution once and reuse for all k values
    // This avoids n+1 allocations of the distribution object
    let mut dist = Binomial::new(p, n as u64)
        .map(|binom| (0..=n as u64).map(|k| binom.pmf(k)).collect::<Vec<_>>())
        .map_err(|e| {
            crate::Error::Validation(format!(
                "binomial_distribution: invalid parameters n={n}, p={p}: {e}"
            ))
        })?;

    // Normalize (should already sum to ~1, but defensive for numerical edge cases)
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
///   (2nd ed.). Wiley. Chapter 3.
pub fn binomial_probability(n: usize, k: usize, p: f64) -> f64 {
    use statrs::distribution::{Binomial, Discrete};

    // Handle edge cases that statrs may not accept
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
        Err(_) => 0.0, // Invalid parameters (should not happen after edge case checks)
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
/// P(0)     = (1 - p)^n
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
/// let pmf = binomial_pmf_all(10, 0.3);
/// assert_eq!(pmf.len(), 11);
/// for k in 0..=10 {
///     assert!((pmf[k] - binomial_probability(10, k, 0.3)).abs() < 1e-12);
/// }
/// ```
///
/// # References
///
/// - Johnson, N. L., Kotz, S., & Kemp, A. W. (1993). *Univariate Discrete Distributions*
///   (2nd ed.). Wiley. Chapter 3.
#[must_use]
pub fn binomial_pmf_all(n: usize, p: f64) -> Vec<f64> {
    let mut pmf = vec![0.0; n + 1];
    if p <= 0.0 {
        pmf[0] = 1.0;
        return pmf;
    }
    if p >= 1.0 {
        pmf[n] = 1.0;
        return pmf;
    }

    let log_ratio = p.ln() - (1.0 - p).ln();
    let mut log_prob = n as f64 * (1.0 - p).ln();
    pmf[0] = log_prob.exp();
    for k in 0..n {
        log_prob += ((n - k) as f64).ln() - ((k + 1) as f64).ln() + log_ratio;
        pmf[k + 1] = log_prob.exp();
    }
    pmf
}

/// Calculate log of binomial coefficient ln(C(n,k)).
///
/// Computes the natural logarithm of "n choose k" using numerically stable
/// methods. For large factorials (n ≥ 20), uses Stirling's approximation to
/// avoid overflow.
///
/// # Mathematical Definition
///
/// ```text
/// ln C(n,k) = ln(n! / (k! * (n-k)!))
///           = ln(n!) - ln(k!) - ln((n-k)!)
/// ```
///
/// # Arguments
///
/// * `n` - Total number of items
/// * `k` - Number of items to choose
///
/// # Returns
///
/// ln(n choose k), or -∞ if k > n
///
/// # Numerical Method
///
/// - For n < 20: Exact calculation using sum of logarithms
/// - For n ≥ 20: Stirling's approximation ln(n!) ≈ n ln(n) - n + 0.5 ln(2πn)
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::log_binomial_coefficient;
///
/// // C(10, 5) = 252, ln(252) ≈ 5.5294
/// let log_coef = log_binomial_coefficient(10, 5);
/// assert!((log_coef - 252.0_f64.ln()).abs() < 1e-6);
/// ```
///
/// # References
///
/// - Abramowitz, M., & Stegun, I. A. (1964). *Handbook of Mathematical Functions*.
///   Formula 6.1.37 (Stirling's approximation).
pub fn log_binomial_coefficient(n: usize, k: usize) -> f64 {
    statrs::function::factorial::ln_binomial(n as u64, k as u64)
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
/// ```
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
///   Distributions, Volume 2* (2nd ed.). Wiley. Chapter 25 (Beta distribution).
/// - Devroye, L. (1986). *Non-Uniform Random Variate Generation*. Springer.
///   Chapter 9 (Beta distribution sampling via gamma ratio).
/// - Marsaglia, G., & Tsang, W. W. (2000). "A Simple Method for Generating Gamma
///   Variables." *ACM Transactions on Mathematical Software*, 26(3), 363-372.
pub fn sample_beta(
    rng: &mut dyn RandomNumberGenerator,
    alpha: f64,
    beta: f64,
) -> crate::Result<f64> {
    if alpha <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Beta α parameter must be positive, got: {}",
            alpha
        )));
    }
    if beta <= 0.0 {
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
    // We use the unchecked version since we've already validated α, β > 0
    let x = sample_gamma_unchecked(rng, alpha);
    let y = sample_gamma_unchecked(rng, beta);

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

// ============================================================================
// Exponential Distribution
// ============================================================================

/// Sample from Exponential(λ) distribution using inverse CDF method.
///
/// The exponential distribution models the time between events in a Poisson
/// process, making it essential for default timing, hazard rates, and
/// inter-arrival time simulation.
///
/// # Mathematical Definition
///
/// ```text
/// Exponential(λ) with λ > 0:
/// - Support: [0, ∞)
/// - Mean: 1/λ
/// - Variance: 1/λ²
/// - PDF: f(x) = λ e^(-λx)
/// - CDF: F(x) = 1 - e^(-λx)
/// ```
///
/// # Algorithm
///
/// Uses inverse CDF method: X = -ln(U) / λ where U ~ Uniform(0,1)
///
/// # Arguments
///
/// * `rng` - Random number generator implementing [`RandomNumberGenerator`]
/// * `lambda` - Rate parameter (λ > 0), representing events per unit time
///
/// # Returns
///
/// Random sample x ∈ [0, ∞) from Exponential(λ)
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if λ ≤ 0.
///
/// # Use Cases
///
/// - **Default timing**: Time to default with constant hazard rate λ
/// - **CVA exposure**: Exposure profile simulation
/// - **Poisson processes**: Inter-arrival times between events
/// - **Prepayment modeling**: Time to prepayment event
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::sample_exponential;
/// use finstack_quant_core::math::random::Pcg64Rng;
/// use finstack_quant_core::math::RandomNumberGenerator;
///
/// let mut rng = Pcg64Rng::new(42);
///
/// // Sample default time with 5% annual hazard rate
/// let default_time = sample_exponential(&mut rng as &mut dyn RandomNumberGenerator, 0.05)?;
/// assert!(default_time >= 0.0);
///
/// // Expected mean is 1/λ = 20 years
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # References
///
/// - Johnson, N. L., Kotz, S., & Balakrishnan, N. (1994). *Continuous Univariate
///   Distributions, Volume 1* (2nd ed.). Wiley. Chapter 19 (Exponential distribution).
pub fn sample_exponential(rng: &mut dyn RandomNumberGenerator, lambda: f64) -> crate::Result<f64> {
    if lambda <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Exponential rate parameter λ must be positive, got: {}",
            lambda
        )));
    }

    // Inverse CDF method: X = -ln(U) / λ
    // Clamp u away from 0 to prevent ln(0) = -∞
    let u = rng.uniform().max(f64::MIN_POSITIVE);
    Ok(-u.ln() / lambda)
}

/// Probability density function (PDF) of the Exponential distribution.
///
/// # Mathematical Definition
///
/// ```text
/// f(x; λ) = λ e^(-λx)  for x ≥ 0
///         = 0          for x < 0
/// ```
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the PDF
/// * `lambda` - Rate parameter (λ > 0)
///
/// # Returns
///
/// Probability density f(x; λ) ≥ 0
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::exponential_pdf;
///
/// // PDF at x=0 equals λ
/// assert!((exponential_pdf(0.0, 2.0) - 2.0).abs() < 1e-10);
///
/// // PDF is always non-negative
/// assert!(exponential_pdf(1.0, 0.5) >= 0.0);
/// ```
#[must_use]
pub fn exponential_pdf(x: f64, lambda: f64) -> f64 {
    use statrs::distribution::{Continuous, Exp};

    if x < 0.0 || lambda <= 0.0 {
        return 0.0;
    }

    match Exp::new(lambda) {
        Ok(exp) => exp.pdf(x),
        Err(_) => 0.0,
    }
}

/// Cumulative distribution function (CDF) of the Exponential distribution.
///
/// # Mathematical Definition
///
/// ```text
/// F(x; λ) = 1 - e^(-λx)  for x ≥ 0
///         = 0            for x < 0
/// ```
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the CDF
/// * `lambda` - Rate parameter (λ > 0)
///
/// # Returns
///
/// Cumulative probability F(x; λ) ∈ [0, 1]
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::exponential_cdf;
///
/// // CDF at x=0 is 0
/// assert!((exponential_cdf(0.0, 1.0) - 0.0).abs() < 1e-10);
///
/// // CDF at x=∞ approaches 1
/// assert!(exponential_cdf(100.0, 1.0) > 0.99999);
///
/// // P(X ≤ 1/λ) ≈ 0.632 (one mean)
/// assert!((exponential_cdf(1.0, 1.0) - 0.6321205588).abs() < 1e-6);
/// ```
#[must_use]
pub fn exponential_cdf(x: f64, lambda: f64) -> f64 {
    use statrs::distribution::{ContinuousCDF, Exp};

    if x < 0.0 {
        return 0.0;
    }
    if lambda <= 0.0 {
        return 0.0;
    }

    match Exp::new(lambda) {
        Ok(exp) => exp.cdf(x),
        Err(_) => 0.0,
    }
}

/// Quantile function (inverse CDF) of the Exponential distribution.
///
/// Returns the value x such that P(X ≤ x) = p.
///
/// # Mathematical Definition
///
/// ```text
/// F⁻¹(p; λ) = -ln(1 - p) / λ  for p ∈ [0, 1)
/// ```
///
/// # Arguments
///
/// * `p` - Probability in [0, 1)
/// * `lambda` - Rate parameter (λ > 0)
///
/// # Returns
///
/// Quantile x such that F(x; λ) = p
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if:
/// - p ∉ [0, 1)
/// - λ ≤ 0
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::{exponential_quantile, exponential_cdf};
///
/// // Median of Exp(1) is ln(2) ≈ 0.693
/// let median = exponential_quantile(0.5, 1.0)?;
/// assert!((median - 0.6931471805599453).abs() < 1e-10);
///
/// // Round-trip: CDF(quantile(p)) = p
/// let p = 0.75;
/// let x = exponential_quantile(p, 2.0)?;
/// assert!((exponential_cdf(x, 2.0) - p).abs() < 1e-10);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn exponential_quantile(p: f64, lambda: f64) -> crate::Result<f64> {
    use statrs::distribution::{ContinuousCDF, Exp};

    if !(0.0..1.0).contains(&p) {
        return Err(crate::Error::Validation(format!(
            "Probability p must be in [0, 1), got: {}",
            p
        )));
    }
    if lambda <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Exponential rate parameter λ must be positive, got: {}",
            lambda
        )));
    }

    match Exp::new(lambda) {
        Ok(exp) => Ok(exp.inverse_cdf(p)),
        Err(_) => Err(crate::Error::Validation(
            "Failed to create exponential distribution".to_string(),
        )),
    }
}

// ============================================================================
// Log-Normal Distribution
// ============================================================================

/// Sample from LogNormal(μ, σ) distribution.
///
/// The log-normal distribution is essential for modeling asset prices under
/// geometric Brownian motion, recovery rates, and other positive-valued
/// financial quantities with multiplicative noise.
///
/// # Mathematical Definition
///
/// ```text
/// LogNormal(μ, σ) with σ ≥ 0:
/// - Support: (0, ∞)
/// - If X ~ LogNormal(μ, σ), then ln(X) ~ Normal(μ, σ²)
/// - Mean: exp(μ + σ²/2)
/// - Variance: (exp(σ²) - 1) * exp(2μ + σ²)
/// - Mode: exp(μ - σ²)
/// ```
///
/// # Practical Parameter Ranges
///
/// | Parameter | Typical Range | Notes |
/// |-----------|---------------|-------|
/// | μ (mu) | [-10, 10] | Very negative μ (< -10) produces samples approaching 0; very positive μ (> 10) produces huge samples |
/// | σ (sigma) | [0.01, 3.0] | Financial volatilities rarely exceed 300%; σ < 0.01 is nearly deterministic |
///
/// **Warning for extreme μ values:**
/// - μ → -∞: Samples approach 0, which may cause downstream precision issues
/// - μ → +∞: Samples become very large, potentially causing overflow
///
/// For asset price simulation, typical values are:
/// - **Equity**: μ ∈ [-0.1, 0.3], σ ∈ [0.15, 0.50]
/// - **FX**: μ ∈ [-0.05, 0.05], σ ∈ [0.05, 0.20]
/// - **Commodity**: μ ∈ [-0.2, 0.2], σ ∈ [0.20, 0.60]
///
/// # Algorithm
///
/// Uses the transformation: X = exp(μ + σ*Z) where Z ~ N(0,1)
///
/// # Arguments
///
/// * `rng` - Random number generator implementing [`RandomNumberGenerator`]
/// * `mu` - Location parameter (mean of underlying normal). Practical range: [-10, 10]
/// * `sigma` - Scale parameter (std dev of underlying normal, σ ≥ 0). Practical range: [0.01, 3.0]
///
/// # Returns
///
/// Random sample x ∈ (0, ∞) from LogNormal(μ, σ)
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if σ < 0.
///
/// # Use Cases
///
/// - **Asset prices**: Stock prices under GBM: S_T = S_0 * exp((μ-σ²/2)T + σW_T)
/// - **Recovery rates**: Positive LGD modeling
/// - **Income/wealth**: Economic distributions
/// - **Jump sizes**: Multiplicative jump factors in Merton model
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::sample_lognormal;
/// use finstack_quant_core::math::random::Pcg64Rng;
/// use finstack_quant_core::math::RandomNumberGenerator;
///
/// let mut rng = Pcg64Rng::new(42);
///
/// // Sample asset price ratio (current price = 100)
/// let price_factor = sample_lognormal(&mut rng as &mut dyn RandomNumberGenerator, 0.0, 0.2)?;
/// let new_price = 100.0 * price_factor;
/// assert!(new_price > 0.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # References
///
/// - Johnson, N. L., Kotz, S., & Balakrishnan, N. (1994). *Continuous Univariate
///   Distributions, Volume 1* (2nd ed.). Wiley. Chapter 14 (Lognormal distribution).
pub fn sample_lognormal(
    rng: &mut dyn RandomNumberGenerator,
    mu: f64,
    sigma: f64,
) -> crate::Result<f64> {
    if sigma < 0.0 {
        return Err(crate::Error::Validation(format!(
            "Log-normal σ must be non-negative, got: {}",
            sigma
        )));
    }

    // X = exp(μ + σ*Z) where Z ~ N(0,1)
    let z = rng.normal(0.0, 1.0);
    Ok((mu + sigma * z).exp())
}

/// Probability density function (PDF) of the Log-Normal distribution.
///
/// # Mathematical Definition
///
/// ```text
/// f(x; μ, σ) = (1 / (x σ √(2π))) * exp(-(ln(x) - μ)² / (2σ²))  for x > 0
///            = 0                                                  for x ≤ 0
/// ```
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the PDF (must be > 0)
/// * `mu` - Location parameter (mean of underlying normal)
/// * `sigma` - Scale parameter (std dev of underlying normal, σ > 0)
///
/// # Returns
///
/// Probability density f(x; μ, σ) ≥ 0
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::lognormal_pdf;
///
/// // PDF is 0 for non-positive x
/// assert_eq!(lognormal_pdf(0.0, 0.0, 1.0), 0.0);
/// assert_eq!(lognormal_pdf(-1.0, 0.0, 1.0), 0.0);
///
/// // PDF is positive for positive x
/// assert!(lognormal_pdf(1.0, 0.0, 1.0) > 0.0);
/// ```
#[must_use]
pub fn lognormal_pdf(x: f64, mu: f64, sigma: f64) -> f64 {
    use statrs::distribution::{Continuous, LogNormal};

    if x <= 0.0 || sigma <= 0.0 {
        return 0.0;
    }

    match LogNormal::new(mu, sigma) {
        Ok(ln) => ln.pdf(x),
        Err(_) => 0.0,
    }
}

/// Cumulative distribution function (CDF) of the Log-Normal distribution.
///
/// # Mathematical Definition
///
/// ```text
/// F(x; μ, σ) = Φ((ln(x) - μ) / σ)  for x > 0
///            = 0                    for x ≤ 0
/// ```
///
/// where Φ is the standard normal CDF.
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the CDF
/// * `mu` - Location parameter (mean of underlying normal)
/// * `sigma` - Scale parameter (std dev of underlying normal, σ > 0)
///
/// # Returns
///
/// Cumulative probability F(x; μ, σ) ∈ [0, 1]
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::lognormal_cdf;
///
/// // CDF at median exp(μ) = 0.5 when σ = 1
/// let median = 1.0_f64.exp(); // e^0 = 1 when μ=0
/// assert!((lognormal_cdf(1.0, 0.0, 1.0) - 0.5).abs() < 1e-10);
///
/// // CDF is 0 for non-positive x
/// assert_eq!(lognormal_cdf(0.0, 0.0, 1.0), 0.0);
/// ```
#[must_use]
pub fn lognormal_cdf(x: f64, mu: f64, sigma: f64) -> f64 {
    use statrs::distribution::{ContinuousCDF, LogNormal};

    if x <= 0.0 {
        return 0.0;
    }
    if sigma <= 0.0 {
        return 0.0;
    }

    match LogNormal::new(mu, sigma) {
        Ok(ln) => ln.cdf(x),
        Err(_) => 0.0,
    }
}

/// Quantile function (inverse CDF) of the Log-Normal distribution.
///
/// Returns the value x such that P(X ≤ x) = p.
///
/// # Mathematical Definition
///
/// ```text
/// F⁻¹(p; μ, σ) = exp(μ + σ * Φ⁻¹(p))
/// ```
///
/// where Φ⁻¹ is the standard normal quantile function.
///
/// # Arguments
///
/// * `p` - Probability in [0, 1)
/// * `mu` - Location parameter (mean of underlying normal)
/// * `sigma` - Scale parameter (std dev of underlying normal, σ > 0)
///
/// # Returns
///
/// Quantile x such that F(x; μ, σ) = p
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if:
/// - p ∉ [0, 1)
/// - σ ≤ 0
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::{lognormal_quantile, lognormal_cdf};
///
/// // Median is exp(μ)
/// let median = lognormal_quantile(0.5, 0.0, 1.0)?;
/// assert!((median - 1.0).abs() < 1e-10);
///
/// // Round-trip test
/// let p = 0.75;
/// let x = lognormal_quantile(p, 0.5, 0.3)?;
/// assert!((lognormal_cdf(x, 0.5, 0.3) - p).abs() < 1e-10);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn lognormal_quantile(p: f64, mu: f64, sigma: f64) -> crate::Result<f64> {
    use statrs::distribution::{ContinuousCDF, LogNormal};

    if !(0.0..1.0).contains(&p) {
        return Err(crate::Error::Validation(format!(
            "Probability p must be in [0, 1), got: {}",
            p
        )));
    }
    if sigma <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Log-normal σ must be positive, got: {}",
            sigma
        )));
    }

    match LogNormal::new(mu, sigma) {
        Ok(ln) => Ok(ln.inverse_cdf(p)),
        Err(_) => Err(crate::Error::Validation(
            "Failed to create log-normal distribution".to_string(),
        )),
    }
}

// ============================================================================
// Gamma Distribution (public helper)
// ============================================================================

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
///   Variables." *ACM Transactions on Mathematical Software*, 26(3), 363-372.
pub fn sample_gamma(rng: &mut dyn RandomNumberGenerator, shape: f64) -> crate::Result<f64> {
    if shape <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Gamma shape parameter must be positive, got: {}",
            shape
        )));
    }

    Ok(sample_gamma_unchecked(rng, shape))
}

/// Internal unchecked gamma sampling (assumes shape > 0).
fn sample_gamma_unchecked(rng: &mut dyn RandomNumberGenerator, shape: f64) -> f64 {
    if shape < 1.0 {
        // Ahrens-Dieter transformation for shape < 1:
        // If X ~ Gamma(shape + 1), then X * U^(1/shape) ~ Gamma(shape)
        let u = rng.uniform();
        // Clamp u away from 0 to prevent ln(0) issues
        let u_safe = u.max(1e-300);
        return sample_gamma_unchecked(rng, shape + 1.0) * u_safe.powf(1.0 / shape);
    }

    // Marsaglia-Tsang method for shape >= 1
    let d = shape - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();

    loop {
        // Generate normal variate using Box-Muller
        let x = rng.normal(0.0, 1.0);
        let v = 1.0 + c * x;

        if v > 0.0 {
            let v = v * v * v; // v^3
            let u = rng.uniform();
            let x2 = x * x;

            // Squeeze test (fast accept)
            if u < 1.0 - 0.0331 * x2 * x2 {
                return d * v;
            }

            // Full rejection test
            // Clamp u and v away from 0 to prevent ln(0)
            let u_safe = u.max(1e-300);
            let v_safe = v.max(1e-300);
            if u_safe.ln() < 0.5 * x2 + d * (1.0 - v_safe + v_safe.ln()) {
                return d * v;
            }
        }
        // Reject and retry
    }
}

// ============================================================================
// Chi-Squared Distribution
// ============================================================================

/// Sample from Chi-Squared(k) distribution.
///
/// The chi-squared distribution with k degrees of freedom is the distribution
/// of a sum of squares of k independent standard normal random variables.
/// It is fundamental for variance estimation, hypothesis testing, and the
/// CIR (Cox-Ingersoll-Ross) interest rate model.
///
/// # Mathematical Definition
///
/// ```text
/// χ²(k) with k > 0 (degrees of freedom):
/// - Support: [0, ∞)
/// - Mean: k
/// - Variance: 2k
/// - χ²(k) = Gamma(k/2, 2)
/// ```
///
/// # Algorithm
///
/// Uses the relationship χ²(k) = Gamma(k/2, 1) × 2, leveraging [`sample_gamma`].
///
/// # Arguments
///
/// * `rng` - Random number generator implementing [`RandomNumberGenerator`]
/// * `df` - Degrees of freedom (k > 0)
///
/// # Returns
///
/// Random sample x ∈ [0, ∞) from χ²(k)
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if df ≤ 0.
///
/// # Use Cases
///
/// - **Variance estimation**: Sum of squared residuals / σ² ~ χ²(n-p)
/// - **CIR model**: Interest rate variance process
/// - **Student's t distribution**: T = Z / √(V/k) where V ~ χ²(k)
/// - **Hypothesis testing**: Chi-squared test statistic
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::sample_chi_squared;
/// use finstack_quant_core::math::random::Pcg64Rng;
/// use finstack_quant_core::math::RandomNumberGenerator;
///
/// let mut rng = Pcg64Rng::new(42);
///
/// // Sample chi-squared with 5 degrees of freedom
/// let x = sample_chi_squared(&mut rng as &mut dyn RandomNumberGenerator, 5.0)?;
/// assert!(x >= 0.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # References
///
/// - Johnson, N. L., Kotz, S., & Balakrishnan, N. (1994). *Continuous Univariate
///   Distributions, Volume 1* (2nd ed.). Wiley. Chapter 18 (Chi-squared distribution).
pub fn sample_chi_squared(rng: &mut dyn RandomNumberGenerator, df: f64) -> crate::Result<f64> {
    if df <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Chi-squared degrees of freedom must be positive, got: {}",
            df
        )));
    }

    // χ²(k) = Gamma(k/2, 2) = 2 * Gamma(k/2, 1)
    // We use unchecked since df > 0 implies df/2 > 0
    Ok(2.0 * sample_gamma_unchecked(rng, df / 2.0))
}

/// Probability density function (PDF) of the Chi-Squared distribution.
///
/// # Mathematical Definition
///
/// ```text
/// f(x; k) = (x^(k/2-1) * e^(-x/2)) / (2^(k/2) * Γ(k/2))  for x > 0
///         = 0                                              for x ≤ 0
/// ```
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the PDF
/// * `df` - Degrees of freedom (k > 0)
///
/// # Returns
///
/// Probability density f(x; k) ≥ 0
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::chi_squared_pdf;
///
/// // PDF is 0 for non-positive x
/// assert_eq!(chi_squared_pdf(0.0, 1.0), 0.0);
/// assert_eq!(chi_squared_pdf(-1.0, 5.0), 0.0);
///
/// // PDF is positive for positive x
/// assert!(chi_squared_pdf(2.0, 3.0) > 0.0);
/// ```
#[must_use]
pub fn chi_squared_pdf(x: f64, df: f64) -> f64 {
    use statrs::distribution::{ChiSquared, Continuous};

    if x <= 0.0 || df <= 0.0 {
        return 0.0;
    }

    match ChiSquared::new(df) {
        Ok(chi2) => chi2.pdf(x),
        Err(_) => 0.0,
    }
}

/// Cumulative distribution function (CDF) of the Chi-Squared distribution.
///
/// # Mathematical Definition
///
/// ```text
/// F(x; k) = γ(k/2, x/2) / Γ(k/2)  for x > 0
///         = 0                       for x ≤ 0
/// ```
///
/// where γ is the lower incomplete gamma function.
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the CDF
/// * `df` - Degrees of freedom (k > 0)
///
/// # Returns
///
/// Cumulative probability F(x; k) ∈ [0, 1]
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::chi_squared_cdf;
///
/// // CDF at x=0 is 0
/// assert!((chi_squared_cdf(0.0, 5.0) - 0.0).abs() < 1e-10);
///
/// // For df=1, P(χ² ≤ 3.84) ≈ 0.95 (critical value for 95% test)
/// assert!((chi_squared_cdf(3.841, 1.0) - 0.95).abs() < 0.001);
/// ```
#[must_use]
pub fn chi_squared_cdf(x: f64, df: f64) -> f64 {
    use statrs::distribution::{ChiSquared, ContinuousCDF};

    if x <= 0.0 {
        return 0.0;
    }
    if df <= 0.0 {
        return 0.0;
    }

    match ChiSquared::new(df) {
        Ok(chi2) => chi2.cdf(x),
        Err(_) => 0.0,
    }
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
/// use finstack_quant_core::math::distributions::{chi_squared_quantile, chi_squared_cdf};
///
/// // 95th percentile for df=1 is approximately 3.841
/// let x_95 = chi_squared_quantile(0.95, 1.0)?;
/// assert!((x_95 - 3.841).abs() < 0.01);
///
/// // Round-trip test
/// let p = 0.90;
/// let x = chi_squared_quantile(p, 5.0)?;
/// assert!((chi_squared_cdf(x, 5.0) - p).abs() < 1e-10);
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

// ============================================================================
// Student's t Distribution (Sampler)
// ============================================================================

/// Sample from Student's t(ν) distribution.
///
/// The Student's t distribution is essential for modeling heavy-tailed returns,
/// fat-tailed risk metrics (VaR/ES), and robust statistical estimation. The
/// CDF and inverse CDF are already available in `special_functions`.
///
/// # Mathematical Definition
///
/// ```text
/// t(ν) with ν > 0 (degrees of freedom):
/// - Support: (-∞, ∞)
/// - Mean: 0 (for ν > 1)
/// - Variance: ν/(ν-2) (for ν > 2)
/// - Heavier tails than Normal; approaches Normal as ν → ∞
/// ```
///
/// # Algorithm
///
/// Uses the ratio method: T = Z / √(V/ν) where Z ~ N(0,1) and V ~ χ²(ν)
///
/// # Arguments
///
/// * `rng` - Random number generator implementing [`RandomNumberGenerator`]
/// * `df` - Degrees of freedom (ν > 0)
///
/// # Returns
///
/// Random sample from t(ν)
///
/// # Errors
///
/// Returns [`Error::Validation`](crate::Error::Validation) if df ≤ 0.
///
/// # Use Cases
///
/// - **Equity returns**: Fat-tailed return distributions (df ≈ 4-6)
/// - **VaR/CVaR**: Tail risk estimation
/// - **t-copula simulation**: Joint heavy-tailed defaults
/// - **Robust regression**: Outlier-resistant parameter estimation
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::distributions::sample_student_t;
/// use finstack_quant_core::math::random::Pcg64Rng;
/// use finstack_quant_core::math::RandomNumberGenerator;
///
/// let mut rng = Pcg64Rng::new(42);
///
/// // Sample from t-distribution with 5 degrees of freedom
/// let t = sample_student_t(&mut rng as &mut dyn RandomNumberGenerator, 5.0)?;
/// assert!(t.is_finite());
///
/// // Higher df approaches Normal
/// let t_high_df = sample_student_t(&mut rng as &mut dyn RandomNumberGenerator, 100.0)?;
/// assert!(t_high_df.is_finite());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
///
/// # References
///
/// - Johnson, N. L., Kotz, S., & Balakrishnan, N. (1995). *Continuous Univariate
///   Distributions, Volume 2* (2nd ed.). Wiley. Chapter 28 (Student's t distribution).
pub fn sample_student_t(rng: &mut dyn RandomNumberGenerator, df: f64) -> crate::Result<f64> {
    if df <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "Student's t degrees of freedom must be positive, got: {}",
            df
        )));
    }

    // T = Z / sqrt(V/ν) where Z ~ N(0,1), V ~ χ²(ν)
    let z = rng.normal(0.0, 1.0);
    // We use unchecked gamma since df > 0 implies df/2 > 0
    let v = 2.0 * sample_gamma_unchecked(rng, df / 2.0);

    Ok(z / (v / df).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_log_binomial_coefficient() {
        // Test known values
        // C(10, 5) = 252, ln(252) ≈ 5.53039
        let actual = log_binomial_coefficient(10, 5);
        let expected = 252.0_f64.ln(); // More precise expected value
        assert!(
            (actual - expected).abs() < 1e-4,
            "Expected {}, got {}",
            expected,
            actual
        );

        // Test edge cases
        assert_eq!(log_binomial_coefficient(5, 0), 0.0);
        assert_eq!(log_binomial_coefficient(5, 5), 0.0);
        assert_eq!(log_binomial_coefficient(3, 5), f64::NEG_INFINITY);
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

    // ========================================================================
    // Exponential Distribution Tests
    // ========================================================================

    #[test]
    fn test_sample_exponential_basic() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);
        let lambda = 2.0;

        // All samples should be non-negative
        for _ in 0..100 {
            let x = sample_exponential(&mut rng as &mut dyn RandomNumberGenerator, lambda)
                .expect("Exponential(2.0) should succeed");
            assert!(
                x >= 0.0,
                "Exponential sample should be non-negative, got {}",
                x
            );
        }
    }

    #[test]
    fn test_sample_exponential_statistics() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(12345);
        let lambda = 0.5; // Expected mean = 2.0, variance = 4.0
        let n_samples = 10_000;

        let samples: Vec<f64> = (0..n_samples)
            .map(|_| {
                sample_exponential(&mut rng as &mut dyn RandomNumberGenerator, lambda)
                    .expect("Exponential(0.5) should succeed")
            })
            .collect();

        let sample_mean = samples.iter().sum::<f64>() / n_samples as f64;
        let expected_mean = 1.0 / lambda;

        // Allow 5% relative error
        assert!(
            (sample_mean - expected_mean).abs() < 0.05 * expected_mean,
            "Exponential mean: expected {:.4}, got {:.4}",
            expected_mean,
            sample_mean
        );
    }

    #[test]
    fn test_sample_exponential_validation() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

        // Invalid lambda
        assert!(sample_exponential(&mut rng as &mut dyn RandomNumberGenerator, 0.0).is_err());
        assert!(sample_exponential(&mut rng as &mut dyn RandomNumberGenerator, -1.0).is_err());
    }

    #[test]
    fn test_exponential_pdf() {
        // PDF at x=0 equals λ
        assert!((exponential_pdf(0.0, 2.0) - 2.0).abs() < 1e-10);

        // PDF is non-negative
        assert!(exponential_pdf(1.0, 1.0) >= 0.0);

        // PDF for negative x is 0
        assert_eq!(exponential_pdf(-1.0, 1.0), 0.0);
    }

    #[test]
    fn test_exponential_cdf() {
        // CDF at 0 is 0
        assert!((exponential_cdf(0.0, 1.0) - 0.0).abs() < 1e-10);

        // P(X ≤ 1/λ) ≈ 0.632 (one mean)
        assert!((exponential_cdf(1.0, 1.0) - 0.6321205588).abs() < 1e-6);

        // CDF at infinity approaches 1
        assert!(exponential_cdf(100.0, 1.0) > 0.99999);
    }

    #[test]
    fn test_exponential_quantile_roundtrip() {
        let lambda = 1.5;
        let test_probs = [0.1, 0.25, 0.5, 0.75, 0.9];

        for &p in &test_probs {
            let x = exponential_quantile(p, lambda).expect("Valid p and lambda");
            let p_back = exponential_cdf(x, lambda);
            assert!(
                (p - p_back).abs() < 1e-10,
                "Round-trip failed for p={}, got x={}, p_back={}",
                p,
                x,
                p_back
            );
        }
    }

    #[test]
    fn test_exponential_quantile_validation() {
        // Invalid p
        assert!(exponential_quantile(-0.1, 1.0).is_err());
        assert!(exponential_quantile(1.0, 1.0).is_err());
        assert!(exponential_quantile(1.5, 1.0).is_err());

        // Invalid lambda
        assert!(exponential_quantile(0.5, 0.0).is_err());
        assert!(exponential_quantile(0.5, -1.0).is_err());
    }

    // ========================================================================
    // Log-Normal Distribution Tests
    // ========================================================================

    #[test]
    fn test_sample_lognormal_basic() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

        // All samples should be positive
        for _ in 0..100 {
            let x = sample_lognormal(&mut rng as &mut dyn RandomNumberGenerator, 0.0, 0.5)
                .expect("LogNormal(0, 0.5) should succeed");
            assert!(x > 0.0, "Log-normal sample should be positive, got {}", x);
        }
    }

    #[test]
    fn test_sample_lognormal_statistics() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(12345);
        let mu: f64 = 0.0;
        let sigma: f64 = 0.5;
        let n_samples = 10_000;

        // Expected mean = exp(μ + σ²/2)
        let expected_mean = (mu + sigma * sigma / 2.0).exp();

        let samples: Vec<f64> = (0..n_samples)
            .map(|_| {
                sample_lognormal(&mut rng as &mut dyn RandomNumberGenerator, mu, sigma)
                    .expect("LogNormal(0, 0.5) should succeed")
            })
            .collect();

        let sample_mean = samples.iter().sum::<f64>() / n_samples as f64;

        // Allow 5% relative error
        assert!(
            (sample_mean - expected_mean).abs() < 0.05 * expected_mean,
            "Log-normal mean: expected {:.4}, got {:.4}",
            expected_mean,
            sample_mean
        );
    }

    #[test]
    fn test_sample_lognormal_validation() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

        // Invalid sigma (negative)
        assert!(sample_lognormal(&mut rng as &mut dyn RandomNumberGenerator, 0.0, -0.1).is_err());

        // sigma = 0 is valid (degenerate distribution)
        assert!(sample_lognormal(&mut rng as &mut dyn RandomNumberGenerator, 0.0, 0.0).is_ok());
    }

    #[test]
    fn binomial_pmf_all_matches_binomial_probability() {
        for &(n, p) in &[(10usize, 0.3f64), (50, 0.05), (126, 0.5), (20, 0.9)] {
            let pmf = binomial_pmf_all(n, p);
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
        let pmf = binomial_pmf_all(2_000, 0.5);
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
        let lo = binomial_pmf_all(5, 0.0);
        assert_eq!(lo[0], 1.0);
        assert!(lo[1..].iter().all(|&x| x == 0.0));

        let hi = binomial_pmf_all(5, 1.0);
        assert_eq!(hi[5], 1.0);
        assert!(hi[..5].iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_lognormal_pdf() {
        // PDF is 0 for non-positive x
        assert_eq!(lognormal_pdf(0.0, 0.0, 1.0), 0.0);
        assert_eq!(lognormal_pdf(-1.0, 0.0, 1.0), 0.0);

        // PDF is positive for positive x
        assert!(lognormal_pdf(1.0, 0.0, 1.0) > 0.0);
    }

    #[test]
    fn test_lognormal_cdf() {
        // Median is exp(μ), so CDF(exp(μ)) = 0.5
        let mu: f64 = 0.5;
        let sigma: f64 = 0.3;
        let median = mu.exp();
        assert!((lognormal_cdf(median, mu, sigma) - 0.5).abs() < 1e-6);

        // CDF is 0 for non-positive x
        assert_eq!(lognormal_cdf(0.0, 0.0, 1.0), 0.0);
        assert_eq!(lognormal_cdf(-1.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn test_lognormal_quantile_roundtrip() {
        let mu = 0.5;
        let sigma = 0.3;
        let test_probs = [0.1, 0.25, 0.5, 0.75, 0.9];

        for &p in &test_probs {
            let x = lognormal_quantile(p, mu, sigma).expect("Valid p, mu, sigma");
            let p_back = lognormal_cdf(x, mu, sigma);
            assert!(
                (p - p_back).abs() < 1e-10,
                "Round-trip failed for p={}, got x={}, p_back={}",
                p,
                x,
                p_back
            );
        }
    }

    #[test]
    fn test_lognormal_quantile_validation() {
        // Invalid p
        assert!(lognormal_quantile(-0.1, 0.0, 1.0).is_err());
        assert!(lognormal_quantile(1.0, 0.0, 1.0).is_err());
        assert!(lognormal_quantile(1.5, 0.0, 1.0).is_err());

        // Invalid sigma
        assert!(lognormal_quantile(0.5, 0.0, 0.0).is_err());
        assert!(lognormal_quantile(0.5, 0.0, -1.0).is_err());
    }

    // ========================================================================
    // Gamma Distribution Tests
    // ========================================================================

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
    }

    // ========================================================================
    // Chi-Squared Distribution Tests
    // ========================================================================

    #[test]
    fn test_sample_chi_squared_basic() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

        // All samples should be non-negative
        for _ in 0..100 {
            let x = sample_chi_squared(&mut rng as &mut dyn RandomNumberGenerator, 5.0)
                .expect("Chi-squared(5.0) should succeed");
            assert!(
                x >= 0.0,
                "Chi-squared sample should be non-negative, got {}",
                x
            );
        }
    }

    #[test]
    fn test_sample_chi_squared_statistics() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(12345);
        let df = 5.0; // Mean = df, Variance = 2*df
        let n_samples = 10_000;

        let samples: Vec<f64> = (0..n_samples)
            .map(|_| {
                sample_chi_squared(&mut rng as &mut dyn RandomNumberGenerator, df)
                    .expect("Chi-squared(5.0) should succeed")
            })
            .collect();

        let sample_mean = samples.iter().sum::<f64>() / n_samples as f64;

        // Allow 5% relative error
        assert!(
            (sample_mean - df).abs() < 0.05 * df,
            "Chi-squared({}) mean: expected {:.4}, got {:.4}",
            df,
            df,
            sample_mean
        );
    }

    #[test]
    fn test_sample_chi_squared_validation() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

        // Invalid df
        assert!(sample_chi_squared(&mut rng as &mut dyn RandomNumberGenerator, 0.0).is_err());
        assert!(sample_chi_squared(&mut rng as &mut dyn RandomNumberGenerator, -1.0).is_err());
    }

    #[test]
    fn test_chi_squared_pdf() {
        // PDF is 0 for non-positive x
        assert_eq!(chi_squared_pdf(0.0, 1.0), 0.0);
        assert_eq!(chi_squared_pdf(-1.0, 5.0), 0.0);

        // PDF is positive for positive x
        assert!(chi_squared_pdf(2.0, 3.0) > 0.0);
    }

    #[test]
    fn test_chi_squared_cdf() {
        // CDF at 0 is 0
        assert!((chi_squared_cdf(0.0, 5.0) - 0.0).abs() < 1e-10);

        // For df=1, P(χ² ≤ 3.84) ≈ 0.95 (critical value)
        assert!((chi_squared_cdf(3.841, 1.0) - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_chi_squared_quantile_roundtrip() {
        let df = 5.0;
        let test_probs = [0.1, 0.25, 0.5, 0.75, 0.9];

        for &p in &test_probs {
            let x = chi_squared_quantile(p, df).expect("Valid p and df");
            let p_back = chi_squared_cdf(x, df);
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

    // ========================================================================
    // Student's t Distribution Tests
    // ========================================================================

    #[test]
    fn test_sample_student_t_basic() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

        // All samples should be finite
        for _ in 0..100 {
            let t = sample_student_t(&mut rng as &mut dyn RandomNumberGenerator, 5.0)
                .expect("StudentT(5.0) should succeed");
            assert!(
                t.is_finite(),
                "Student's t sample should be finite, got {}",
                t
            );
        }
    }

    #[test]
    fn test_sample_student_t_statistics() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(12345);
        let df = 10.0; // Mean = 0 (for df > 1), Variance = df/(df-2) = 1.25
        let n_samples = 10_000;

        let samples: Vec<f64> = (0..n_samples)
            .map(|_| {
                sample_student_t(&mut rng as &mut dyn RandomNumberGenerator, df)
                    .expect("StudentT(10.0) should succeed")
            })
            .collect();

        let sample_mean = samples.iter().sum::<f64>() / n_samples as f64;

        // Mean should be close to 0
        assert!(
            sample_mean.abs() < 0.05,
            "Student's t({}) mean should be ~0, got {:.4}",
            df,
            sample_mean
        );
    }

    #[test]
    fn test_sample_student_t_heavy_tails() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(12345);
        let n_samples = 10_000;

        // t(3) should have heavier tails than t(30)
        let samples_t3: Vec<f64> = (0..n_samples)
            .map(|_| {
                sample_student_t(&mut rng as &mut dyn RandomNumberGenerator, 3.0)
                    .expect("StudentT(3.0) should succeed")
            })
            .collect();

        let mut rng2 = Pcg64Rng::new(12345);
        let samples_t30: Vec<f64> = (0..n_samples)
            .map(|_| {
                sample_student_t(&mut rng2 as &mut dyn RandomNumberGenerator, 30.0)
                    .expect("StudentT(30.0) should succeed")
            })
            .collect();

        // Count extreme values (|x| > 3)
        let extreme_t3 = samples_t3.iter().filter(|&&x| x.abs() > 3.0).count();
        let extreme_t30 = samples_t30.iter().filter(|&&x| x.abs() > 3.0).count();

        // t(3) should have more extreme values due to heavier tails
        assert!(
            extreme_t3 > extreme_t30,
            "t(3) should have more extreme values than t(30): {} vs {}",
            extreme_t3,
            extreme_t30
        );
    }

    #[test]
    fn test_sample_student_t_deterministic() {
        use super::super::random::Pcg64Rng;

        let mut rng1 = Pcg64Rng::new(42);
        let mut rng2 = Pcg64Rng::new(42);

        let samples1: Vec<f64> = (0..10)
            .map(|_| {
                sample_student_t(&mut rng1 as &mut dyn RandomNumberGenerator, 5.0)
                    .expect("StudentT(5.0) should succeed")
            })
            .collect();
        let samples2: Vec<f64> = (0..10)
            .map(|_| {
                sample_student_t(&mut rng2 as &mut dyn RandomNumberGenerator, 5.0)
                    .expect("StudentT(5.0) should succeed")
            })
            .collect();

        // Same seed should produce identical sequences
        for (s1, s2) in samples1.iter().zip(samples2.iter()) {
            assert_eq!(s1, s2, "Student's t sampling should be deterministic");
        }
    }

    #[test]
    fn test_sample_student_t_validation() {
        use super::super::random::Pcg64Rng;

        let mut rng = Pcg64Rng::new(42);

        // Invalid df
        assert!(sample_student_t(&mut rng as &mut dyn RandomNumberGenerator, 0.0).is_err());
        assert!(sample_student_t(&mut rng as &mut dyn RandomNumberGenerator, -5.0).is_err());
    }
}
