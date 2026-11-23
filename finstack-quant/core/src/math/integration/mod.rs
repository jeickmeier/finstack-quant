//! Numerical integration methods for financial mathematics.
//!
//! Implements deterministic quadrature rules and adaptive algorithms for
//! computing integrals that arise in option pricing, risk-neutral expectations,
//! and probability calculations.
//!
//! # Algorithms
//!
//! - **Gauss-Hermite**: Integration over normal distribution (Heston, SABR)
//! - **Gauss-Legendre**: Integration over finite intervals (payoff expectations)
//! - **Simpson's rule**: Classic adaptive quadrature
//!
//! # Use Cases
//!
//! - **Option pricing**: Semi-analytical methods requiring characteristic function integration
//! - **Heston model**: Fourier inversion for vanilla options
//! - **SABR**: Probability density integration for digital payoffs
//! - **Risk-neutral expectations**: Integrate payoff × density
//!
//! # Examples
//!
//! ```
//! use finstack_quant_core::math::integration::GaussHermiteQuadrature;
//!
//! // Integrate x² over standard normal (expected value = 1.0)
//! let quad = GaussHermiteQuadrature::new(7)?;
//! let integral = quad.integrate(|x| Ok(x * x))?;
//! assert!((integral - 1.0).abs() < 1e-12);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # References
//!
//! See [`docs/REFERENCES.md`](../../../../docs/REFERENCES.md) for canonical
//! anchors:
//!
//! - **Gaussian Quadrature**:
//!   - Abramowitz, M., & Stegun, I. A. (1964). *Handbook of Mathematical Functions*.
//!     Chapter 25 (Numerical Integration).
//! - Press, W. H., et al. (2007). *Numerical Recipes* (3rd ed.). Section 4.5.
//!   ([`press-numerical-recipes`](../../../../docs/REFERENCES.md#press-numerical-recipes)) `docs/REFERENCES.md#press-numerical-recipes`
//!
//! - **Adaptive Methods**:
//!   - Davis, P. J., & Rabinowitz, P. (1984). *Methods of Numerical Integration*
//!     (2nd ed.). Academic Press.
//!
//! - **Financial Applications**:
//!   - Lewis, A. L. (2000). *Option Valuation under Stochastic Volatility*.
//!     Finance Press. (Fourier methods and quadrature)

mod adaptive;
mod gauss_hermite;
mod gauss_legendre;
#[cfg(test)]
mod tests;

pub use adaptive::{adaptive_simpson, adaptive_simpson_pair};
pub use gauss_hermite::GaussHermiteQuadrature;
pub use gauss_legendre::{
    gauss_legendre_grid, gauss_legendre_integrate_adaptive, gauss_legendre_integrate_composite,
};
