//! Special mathematical functions for financial computation.
//!
//! Checked evaluations of the error function, positive-real log-Gamma,
//! normal probabilities and densities, and Student-t distributions.
//! Gaussian evaluations use the existing statrs kernels where applicable.
//! Student-t tails and inverse probabilities use bounded incomplete-beta
//! calculations.
//!
//! # Use Cases
//!
//! - **Option pricing**: Black-Scholes formula requires Φ(d₁) and Φ(d₂)
//! - **Implied volatility**: Inverse CDF needed for smile calibration
//! - **Risk metrics**: VaR calculation uses Φ⁻¹(confidence level)
//! - **Copula models**: Credit correlation and CDO tranching
//!
//! # Examples
//!
//! ## Basic usage
//!
//! ```
//! use finstack_quant_core::math::special_functions::{norm_cdf, norm_pdf, standard_normal_inv_cdf};
//!
//! // Standard normal CDF at zero should be 0.5
//! assert!((norm_cdf(0.0).expect("valid variate") - 0.5).abs() < 1e-6);
//!
//! // PDF at zero is 1/√(2π)
//! let expected = 1.0 / (2.0 * std::f64::consts::PI).sqrt();
//! assert!((norm_pdf(0.0).expect("valid variate") - expected).abs() < 1e-6);
//!
//! // Round-trip test for inverse CDF
//! let x = standard_normal_inv_cdf(0.84).expect("valid probability");
//! let p_back = norm_cdf(x).expect("valid variate");
//! assert!((p_back - 0.84).abs() < 1e-3);
//! ```
//!
//! ## Black-Scholes option pricing
//!
//! ```
//! use finstack_quant_core::math::special_functions::norm_cdf;
//!
//! // Simplified Black-Scholes call price
//! fn black_scholes_call(s: f64, k: f64, r: f64, vol: f64, t: f64) -> f64 {
//!     let d1 = ((s / k).ln() + (r + 0.5 * vol * vol) * t) / (vol * t.sqrt());
//!     let d2 = d1 - vol * t.sqrt();
//!
//!     s * norm_cdf(d1).expect("valid variate") - k * (-r * t).exp() * norm_cdf(d2).expect("valid variate")
//! }
//!
//! let call_price = black_scholes_call(100.0, 100.0, 0.05, 0.2, 1.0);
//! assert!(call_price > 0.0);
//! ```
//!
//! # References
//!
//! - **statrs crate**: The underlying implementation for erf and Gaussian CDF and inverse CDF.
//!   See <https://github.com/statrs-dev/statrs> for implementation details and accuracy benchmarks.
//!
//! - **Error Function**:
//!   - Abramowitz, M., & Stegun, I. A. (1964). *Handbook of Mathematical Functions*.
//!     National Bureau of Standards.
//!
//! - **Normal Distribution**:
//!   - Johnson, N. L., Kotz, S., & Balakrishnan, N. (1995). *Continuous Univariate
//!     Distributions, Volume 1* (2nd ed.). Wiley. Chapter 13. `docs/REFERENCES.md#press-numerical-recipes`
//!
//! - **Inverse Normal CDF**:
//!   - statrs 0.18 computes `inverse_cdf` via a Boost-Math-derived inverse
//!     complementary error function (`erfc_inv`) rational approximation, not
//!     Wichura's AS 241. Observed accuracy: ~5e-12 absolute on round-trips in
//!     the central region, ~1e-6 *relative* accuracy on the quantile in the
//!     far tails (p ≲ 1e-10).
//!   - Boost Math Toolkit, `erf_inv`/`erfc_inv` documentation:
//!     <https://www.boost.org/doc/libs/release/libs/math/doc/html/math_toolkit/sf_erf/error_inv.html>

mod normal;
mod student_t;

pub use normal::{
    erf, ln_gamma, log_norm_cdf, norm_cdf, norm_cdf_with_params, norm_pdf, norm_pdf_with_params,
    standard_normal_inv_cdf,
};
#[doc(hidden)]
pub use normal::{norm_cdf_unchecked, norm_pdf_unchecked};
pub use student_t::{student_t_cdf, student_t_inv_cdf};

#[cfg(test)]
mod tests;
