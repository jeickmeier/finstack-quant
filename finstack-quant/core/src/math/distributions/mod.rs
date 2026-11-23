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
//! - **Beta**: Bounded \[0,1\] values (recovery rates, correlations)
//! - **Chi-Squared**: Variance estimation, CIR model, hypothesis testing
//!
//! # Numerical Stability
//!
//! - Log-space calculations prevent overflow for large parameters
//! - Binomial PMF via mode-centered outward ratios and compensated normalization
//! - Defensive checks for boundary conditions (p=0, p=1, k>n)
//! - Gamma-function and incomplete-gamma evaluation delegates to the `statrs` crate
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
//! ## Binomial probability mass function
//!
//! ```
//! use finstack_quant_core::math::distributions::binomial_pmf_all;
//!
//! // P(X = k) for every k where X ~ Binomial(10, 0.5)
//! let pmf = binomial_pmf_all(10, 0.5).expect("valid probability");
//! assert!((pmf[5] - 0.24609375).abs() < 1e-6);
//! ```
//!
//! ## Beta sampling for recovery-rate uncertainty
//!
//! ```
//! use finstack_quant_core::math::distributions::sample_beta;
//! use finstack_quant_core::math::random::Pcg64Rng;
//! use finstack_quant_core::math::random::RandomNumberGenerator;
//!
//! let mut rng = Pcg64Rng::new(42, 0);
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

mod binomial;
pub use binomial::{binomial_pmf_all, binomial_pmf_all_into};

mod beta_gamma;
pub use beta_gamma::sample_beta;

mod chi_squared;
pub use chi_squared::chi_squared_quantile;

#[cfg(test)]
mod tests;
