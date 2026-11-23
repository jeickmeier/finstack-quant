//! Error function, log-Gamma, and Gaussian distribution functions.

use std::sync::LazyLock;

use statrs::distribution::Normal;

/// Static standard normal distribution N(0, 1) for performance.
///
/// Creating `Normal::new(0.0, 1.0)` on every call to `norm_cdf`, `norm_pdf`, etc.
/// is wasteful in hot paths (Black-Scholes pricing, Monte Carlo, copula models).
/// This static instance is initialized once and reused across all calls.
///
/// # Safety Justification for `expect`
///
/// Built with `statrs`'s infallible `Normal::standard()`.
static STANDARD_NORMAL: LazyLock<Normal> = LazyLock::new(Normal::standard);

/// Error function.
///
/// Computes the error function using the highly accurate implementation from `statrs`.
///
/// # Definition
///
/// ```text
/// erf(x) = (2/√π) ∫₀ˣ e^(-t²) dt
/// ```
///
/// # Arguments
///
/// * `x` - Input value (any real number)
///
/// # Returns
///
/// The error function erf(x) in `[-1, 1]`. Infinite inputs return
/// the corresponding signed unit value.
///
/// # Errors
/// Returns a validation error if `x` is NaN.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::special_functions::erf;
///
/// // erf(0) ≈ 0
/// assert!(erf(0.0).expect("valid variate").abs() < 1e-6);
///
/// // The error function is odd.
/// let x = 1.5;
/// assert!((erf(-x).expect("valid variate") + erf(x).expect("valid variate")).abs() < 1e-6);
///
/// // erf(∞) → 1
/// assert!((erf(5.0).expect("valid variate") - 1.0).abs() < 1e-5);
/// ```
///
/// # Implementation
///
/// This is a thin wrapper around `statrs::function::erf::erf`.
#[inline]
pub fn erf(x: f64) -> crate::Result<f64> {
    if x.is_nan() {
        return Err(crate::Error::Validation(
            "erf requires a non-NaN variate".into(),
        ));
    }
    Ok(statrs::function::erf::erf(x))
}

/// Natural logarithm of the Gamma function.
///
/// Computes ln(Γ(x)) for x > 0.
///
/// # Arguments
///
/// * `x` - Input value (must be positive)
///
/// # Returns
///
/// Real ln(Γ(x)) for finite positive `x`, with exact zero at one and two.
///
/// # Errors
/// Returns a validation error if `x` is non-finite or nonpositive. Returns a
/// computation error if the logarithm exceeds the finite output range.
#[inline]
pub fn ln_gamma(x: f64) -> crate::Result<f64> {
    if !x.is_finite() || x <= 0.0 {
        return Err(crate::Error::Validation(format!(
            "log Gamma requires a finite positive argument, got {x}"
        )));
    }
    if x.to_bits() == 1.0_f64.to_bits() || x.to_bits() == 2.0_f64.to_bits() {
        return Ok(0.0);
    }
    let value = if x < 0.5 {
        statrs::function::gamma::ln_gamma(1.0 + x) - x.ln()
    } else {
        statrs::function::gamma::ln_gamma(x)
    };
    if !value.is_finite() {
        return Err(crate::Error::Internal(
            "log Gamma exceeds the finite output range".into(),
        ));
    }
    Ok(value)
}

/// Cumulative standard normal distribution function Φ(x).
///
/// Computes the probability that a standard normal random variable is less
/// than or equal to x.
///
/// # Definition
///
/// ```text
/// Φ(x) = P(Z ≤ x) where Z ~ N(0,1)
///      = (1/√(2π)) ∫_{-∞}^x e^(-t²/2) dt
///      = (1/2)[1 + erf(x/√2)]
/// ```
///
/// # Arguments
///
/// * `x` - Input value (any real number)
///
/// # Returns
///
/// Cumulative probability Φ(x) ∈ (0, 1)
///
/// # Numerical Stability
///
/// The `statrs` implementation uses highly accurate algorithms with proper tail handling,
/// which is critical for:
/// - Value-at-Risk with high confidence (99.9%)
/// - Credit correlation in copula models
/// - Base correlation calibration for CDO tranches
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::special_functions::norm_cdf;
///
/// // Φ(0) = 0.5 (median of standard normal)
/// assert!((norm_cdf(0.0).expect("valid variate") - 0.5).abs() < 1e-6);
///
/// // Φ(-x) = 1 - Φ(x) (symmetry)
/// let x = 1.96; // 97.5th percentile
/// assert!((norm_cdf(-x).expect("valid variate") + norm_cdf(x).expect("valid variate") - 1.0).abs() < 1e-6);
///
/// // 95% confidence interval: [-1.96, 1.96]
/// assert!((norm_cdf(1.96).expect("valid variate") - 0.975).abs() < 1e-3);
/// assert!((norm_cdf(-1.96).expect("valid variate") - 0.025).abs() < 1e-3);
/// ```
///
/// # Implementation
///
/// Uses a static standard normal distribution instance for performance in hot paths.
///
/// # Errors
/// Returns a validation error if `x` is NaN.
#[inline]
pub fn norm_cdf(x: f64) -> crate::Result<f64> {
    validate_normal_variate(x)?;
    Ok(norm_cdf_unchecked(x))
}

/// Standard-normal cumulative probability for a validated variate.
///
/// # Arguments
/// * `x` - A non-NaN standard-deviation variate from a validated model path.
#[doc(hidden)]
#[inline]
pub fn norm_cdf_unchecked(x: f64) -> f64 {
    use statrs::distribution::ContinuousCDF;
    STANDARD_NORMAL.cdf(x)
}

/// Natural logarithm of the standard normal cumulative probability.
///
/// Evaluates `ln(P(Z <= x))` without first forming a probability that can
/// underflow in the negative tail. Positive arguments use the complementary
/// tail with `ln_1p` to retain probabilities close to one.
///
/// # Arguments
///
/// * `x` - Standardized, dimensionless normal variate. Any finite real value
///   or infinity is accepted.
///
/// # Returns
///
/// Log probability in `[-infinity, 0]`. Negative infinity maps to negative
/// infinity and positive infinity maps to zero.
///
/// # Errors
/// Returns a validation error for NaN. Returns a computation error for
/// finite `x` whose log probability exceeds the representable range.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::special_functions::log_norm_cdf;
/// assert!((log_norm_cdf(0.0).expect("valid variate") + 2.0_f64.ln()).abs() < 1e-14);
/// assert!(log_norm_cdf(-40.0).expect("valid variate").is_finite());
/// ```
///
/// # Numerical method
///
/// Uses the existing normal CDF in the central region and the asymptotic
/// Mills-ratio expansion beyond ten standard deviations. For positive real
/// arguments the omitted remainder is bounded by the first omitted term;
/// see <https://dlmf.nist.gov/7.12.E1>.
pub fn log_norm_cdf(x: f64) -> crate::Result<f64> {
    validate_normal_variate(x)?;
    let value = if x >= 0.0 {
        (-norm_cdf_unchecked(-x)).ln_1p()
    } else if x > -10.0 {
        norm_cdf_unchecked(x).ln()
    } else {
        // Phi(-a) = phi(a)/a * (1 - 1/a² + 3/a⁴ - 15/a⁶ + ...).
        // At a >= 10 the terms reach machine precision well before their
        // asymptotic growth starts. Stop before growth even for future changes
        // to the switching point.
        let inverse_square = (1.0 / x).powi(2);
        let mut term = 1.0_f64;
        let mut sum = 1.0_f64;
        for order in 1..100 {
            let next = -term * f64::from(2 * order - 1) * inverse_square;
            if next.abs() >= term.abs() {
                break;
            }
            sum += next;
            if next.abs() <= f64::EPSILON * sum.abs() {
                break;
            }
            term = next;
        }
        -0.5 * x * x - (-x).ln() - 0.5 * (2.0 * std::f64::consts::PI).ln() + sum.ln()
    };
    if x.is_finite() && !value.is_finite() {
        return Err(crate::Error::Internal(
            "normal log probability exceeds the finite output range".into(),
        ));
    }
    Ok(value)
}

// Precomputed 1/√(2π) for direct norm_pdf evaluation.
const INV_SQRT_2PI: f64 = 0.398_942_280_401_432_7;

/// Standard normal probability density function φ(x).
///
/// Computes the probability density of the standard normal distribution at x.
///
/// # Definition
///
/// ```text
/// φ(x) = (1/√(2π)) e^(-x²/2)
/// ```
///
/// # Arguments
///
/// * `x` - Input value (any real number)
///
/// # Returns
///
/// Probability density φ(x) ∈ [0, 1/√(2π)]
/// Maximum value occurs at x = 0: φ(0) = 1/√(2π) ≈ 0.3989
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::special_functions::norm_pdf;
///
/// // Maximum at x = 0
/// let max_density = norm_pdf(0.0).expect("valid variate");
/// assert!((max_density - 0.3989).abs() < 1e-4);
///
/// // Symmetric: φ(-x) = φ(x)
/// let x = 1.5;
/// assert!((norm_pdf(-x).expect("valid variate") - norm_pdf(x).expect("valid variate")).abs() < 1e-6);
///
/// // Approximately zero in tails (φ(5.0) ≈ 1.49e-6)
/// assert!(norm_pdf(5.0).expect("valid variate") < 2e-6);
/// ```
///
/// # Use Cases
///
/// - Option Greeks (vega, gamma) in Black-Scholes
/// - Maximum likelihood estimation
/// - Kernel density estimation
/// - Heat kernel in diffusion processes
///
/// # Implementation
///
/// Direct computation using the formula φ(x) = (1/√(2π)) × e^(-x²/2) with a
/// precomputed constant, avoiding trait dispatch overhead from statrs.
///
/// # Errors
/// Returns a validation error if `x` is NaN.
#[inline]
pub fn norm_pdf(x: f64) -> crate::Result<f64> {
    validate_normal_variate(x)?;
    Ok(norm_pdf_unchecked(x))
}

#[inline]
/// Standard-normal density for a validated quadrature node.
///
/// # Arguments
/// * `x` - Non-NaN standard-deviation variate from a validated integration grid.
#[doc(hidden)]
pub fn norm_pdf_unchecked(x: f64) -> f64 {
    INV_SQRT_2PI * (-0.5 * x * x).exp()
}

fn validate_normal_variate(x: f64) -> crate::Result<()> {
    if x.is_nan() {
        return Err(crate::Error::Validation("x must not be NaN".into()));
    }
    Ok(())
}

fn standardize_normal(x: f64, mean: f64, std_dev: f64) -> crate::Result<f64> {
    if !std_dev.is_finite() || std_dev <= 0.0 {
        return Err(crate::Error::Validation(
            "std_dev must be finite and positive".into(),
        ));
    }
    if !mean.is_finite() {
        return Err(crate::Error::Validation(format!(
            "mean must be finite, got {mean}"
        )));
    }
    validate_normal_variate(x)?;
    if !x.is_finite() {
        return Ok(x);
    }
    let diff = x - mean;
    Ok(if diff.is_finite() {
        diff / std_dev
    } else {
        (x / std_dev) - (mean / std_dev)
    })
}

/// General normal cumulative distribution function.
///
/// Computes the CDF of `N(mean, std_dev^2)` by standardizing into the standard
/// normal and delegating to [`norm_cdf`]:
/// `Φ((x - mean) / std_dev)`.
///
/// `std_dev` is the standard deviation in the same units as `x` and `mean`,
/// not the variance. Infinite `x` values evaluate to their `0.0` / `1.0` tail
/// limits; `NaN` `x` or non-finite `mean` returns a validation error.
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the normal CDF (`±∞` accepted, `NaN` rejected).
/// * `mean` - Finite mean of the normal distribution, in the same units as `x`.
/// * `std_dev` - Strictly positive finite standard deviation, in the same units as
///   `x` and `mean`; this is not the variance.
///
/// # Errors
///
/// Returns an error if `std_dev` is non-finite or not strictly positive, `mean`
/// is non-finite, or `x` is `NaN`.
pub fn norm_cdf_with_params(x: f64, mean: f64, std_dev: f64) -> crate::Result<f64> {
    let z = standardize_normal(x, mean, std_dev)?;
    Ok(norm_cdf_unchecked(z))
}

/// General normal probability density function.
///
/// Computes the PDF of `N(mean, std_dev^2)` by standardizing into the standard
/// normal and applying the Jacobian scaling factor:
/// `φ((x - mean) / std_dev) / std_dev`.
///
/// `std_dev` is the standard deviation in the same units as `x` and `mean`,
/// not the variance. Infinite `x` values return `0.0`; `NaN` `x`, non-finite
/// `mean`, or invalid scale returns a validation error.
///
/// # Arguments
///
/// * `x` - Point at which to evaluate the normal density (`±∞` accepted, `NaN` rejected).
/// * `mean` - Finite mean of the normal distribution, in the same units as `x`.
/// * `std_dev` - Strictly positive finite standard deviation, in the same units as
///   `x` and `mean`; this is not the variance.
///
/// # Errors
///
/// Returns a validation error if `std_dev` is non-finite or not strictly
/// positive, `mean` is non-finite, or `x` is NaN. Density overflow returns a
/// computation error.
pub fn norm_pdf_with_params(x: f64, mean: f64, std_dev: f64) -> crate::Result<f64> {
    let z = standardize_normal(x, mean, std_dev)?;
    if !z.is_finite() {
        return Ok(0.0);
    }
    let standard_density = norm_pdf_unchecked(z);
    let direct = standard_density / std_dev;
    if standard_density.is_normal() && direct.is_normal() {
        return Ok(direct);
    }
    let log_density = -0.5 * (2.0 * std::f64::consts::PI).ln() - std_dev.ln() - 0.5 * z * z;
    let density = log_density.exp();
    if density.is_finite() {
        Ok(density)
    } else {
        Err(crate::Error::Internal(
            "normal density overflowed finite f64 range".to_string(),
        ))
    }
}

/// Inverse standard normal cumulative distribution function.
///
/// Computes the inverse of the standard normal CDF, returning the value x
/// such that Φ(x) = p. This function is particularly critical for:
/// - Base correlation calibration where tail behavior impacts conditional default probabilities
/// - Value-at-Risk calculations
/// - Implied volatility solving
///
/// # Arguments
/// * `p` - Probability in `[0, 1]`
///
/// # Returns
/// x such that Φ(x) = p
///
/// # Domain-edge convention
///
/// Exact boundary probabilities map to signed infinity, while strictly
/// out-of-domain inputs return a validation error:
/// - `p == 0.0` → `f64::NEG_INFINITY`
/// - `p == 1.0` → `f64::INFINITY`
/// - `p < 0.0`, `p > 1.0`, or `p.is_nan()` → validation error
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::special_functions::{standard_normal_inv_cdf, norm_cdf};
///
/// // Inverse of median should be zero
/// assert!((standard_normal_inv_cdf(0.5).expect("valid probability") - 0.0).abs() < 1e-6);
///
/// // Round-trip test
/// let x = standard_normal_inv_cdf(0.84).expect("valid probability");
/// let p_back = norm_cdf(x).expect("valid variate");
/// assert!((p_back - 0.84).abs() < 1e-6);
///
/// // Exact domain edges map to ±∞; out-of-domain inputs return errors
/// assert_eq!(standard_normal_inv_cdf(0.0).expect("valid probability"), f64::NEG_INFINITY);
/// assert_eq!(standard_normal_inv_cdf(1.0).expect("valid probability"), f64::INFINITY);
/// assert!(standard_normal_inv_cdf(-0.1).is_err());
/// assert!(standard_normal_inv_cdf(f64::NAN).is_err());
/// ```
///
/// # Implementation
///
/// Uses a static standard normal distribution instance for performance in hot
/// paths. statrs 0.18 evaluates the quantile via a Boost-Math-derived
/// `erfc_inv` rational approximation (not Wichura's AS 241). Delivered
/// accuracy: CDF–quantile round trips agree to
/// ~5e-12 absolute in the central region, and the quantile itself is accurate
/// to ~1e-6 relative in the far tails (p down to ~1e-300).
#[inline]
pub fn standard_normal_inv_cdf(p: f64) -> crate::Result<f64> {
    use statrs::distribution::ContinuousCDF;
    if !(0.0..=1.0).contains(&p) {
        return Err(crate::Error::Validation(format!(
            "normal quantile requires probability in [0, 1], got {p}"
        )));
    }
    #[allow(clippy::float_cmp)]
    if p == 0.0 {
        return Ok(f64::NEG_INFINITY);
    }
    #[allow(clippy::float_cmp)]
    if p == 1.0 {
        return Ok(f64::INFINITY);
    }
    let quantile = STANDARD_NORMAL.inverse_cdf(p);
    if !quantile.is_finite() || (quantile == 0.0 && p.to_bits() != 0.5_f64.to_bits()) {
        return Err(crate::Error::Internal(
            "normal quantile cannot be resolved in the finite output range".into(),
        ));
    }
    Ok(quantile)
}
