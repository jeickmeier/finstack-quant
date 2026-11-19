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
mod tests {
    use super::*;

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
    fn chi_squared_quantile_matches_exponential_closed_form_in_both_tails() {
        // Chi-square(2) is Exp(rate=1/2); this reference does not use an
        // incomplete-gamma implementation or the quantile being tested.
        for p in [1e-300_f64, 1e-100, 1e-12, 1e-6, 0.5, 0.99, 1.0 - 1e-12] {
            let expected = -2.0 * (-p).ln_1p();
            let actual = chi_squared_quantile(p, 2.0).expect("valid exponential quantile");
            assert!(
                (actual / expected - 1.0).abs() < 1e-11,
                "p={p}: actual={actual}, expected={expected}"
            );
        }
    }

    #[test]
    fn chi_squared_quantile_resolves_four_degree_lower_tail_mass() {
        // For df=4, F(x) = 1 - exp(-y)*(1+y), y=x/2. Evaluate its
        // independent Taylor expansion to avoid cancellation in tiny tails.
        for p in [1e-300, 1e-100, 1e-12, 1e-6] {
            let quantile = chi_squared_quantile(p, 4.0).expect("valid lower tail");
            let y = quantile / 2.0;
            let cdf =
                y * y * (0.5 + y * (-1.0 / 3.0 + y * (1.0 / 8.0 + y * (-1.0 / 30.0 + y / 144.0))));
            assert!(
                (cdf / p - 1.0).abs() < 1e-10,
                "p={p}: quantile={quantile}, cdf={cdf}"
            );
        }
    }

    #[test]
    fn chi_squared_quantile_supports_fractional_degrees_and_checks_subnormal_results() {
        use statrs::function::gamma::ln_gamma;

        for (df, p) in [(0.01, 0.5), (0.1, 0.5), (0.1, 1e-12), (2.00001, 1e-6)] {
            let quantile = chi_squared_quantile(p, df).expect("valid fractional df");
            assert!(quantile.is_finite() && quantile > 0.0);
            let a = df / 2.0;
            let y = quantile / 2.0;
            // Independent quadrature of the defining gamma integral after
            // substituting t = y*u^(1/a). The transformed integrand is smooth
            // even for a < 1, without a singular endpoint or CDF recurrence.
            let intervals = 1_024;
            let integral: f64 = (0..intervals)
                .map(|i| {
                    let u = (f64::from(i) + 0.5) / f64::from(intervals);
                    (-y * u.powf(1.0 / a)).exp()
                })
                .sum::<f64>()
                / f64::from(intervals);
            let cdf = (a * y.ln() - ln_gamma(a + 1.0)).exp() * integral;
            assert!(
                (cdf / p - 1.0).abs() < 1e-10,
                "df={df}, p={p}: quantile={quantile}, integral CDF={cdf}"
            );
        }
        assert_eq!(chi_squared_quantile(0.0, 0.01).expect("zero p"), 0.0);
        assert!(chi_squared_quantile(1e-12, 0.01).is_err());
        assert!(chi_squared_quantile(0.5, f64::from_bits(1)).is_err());
        let smallest = f64::from_bits(1);
        assert_eq!(
            chi_squared_quantile(smallest, 2.0).expect("subnormal quantile"),
            2.0 * smallest
        );
    }

    #[test]
    fn chi_squared_quantile_verifies_relative_upper_tail_accuracy() {
        use statrs::distribution::{ChiSquared, ContinuousCDF};

        let p = 1.0 - 1e-12;
        for df in [0.01, 0.1, 1.0, 4.0, 30.0, 1_000.0] {
            let x = chi_squared_quantile(p, df).expect("valid upper tail");
            let survival = ChiSquared::new(df).expect("valid df").sf(x);
            assert!(
                (survival / (1.0 - p) - 1.0).abs() < 1e-9,
                "df={df}: survival={survival}, target={}",
                1.0 - p
            );
        }
    }

    #[test]
    fn test_chi_squared_quantile_validation() {
        // Invalid p
        assert!(chi_squared_quantile(-0.1, 5.0).is_err());
        assert_eq!(
            chi_squared_quantile(1.0, 5.0).expect("upper endpoint"),
            f64::INFINITY
        );
        assert!(chi_squared_quantile(1.5, 5.0).is_err());

        // Invalid df
        assert!(chi_squared_quantile(0.5, 0.0).is_err());
        assert!(chi_squared_quantile(0.5, -1.0).is_err());
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(chi_squared_quantile(0.5, invalid).is_err());
            assert!(chi_squared_quantile(invalid, 5.0).is_err());
        }
        for unsupported in [1e8 + 1.0, 1e16, f64::MAX] {
            assert!(chi_squared_quantile(0.5, unsupported).is_err());
            assert_eq!(chi_squared_quantile(0.0, unsupported).expect("zero p"), 0.0);
        }
    }
}
