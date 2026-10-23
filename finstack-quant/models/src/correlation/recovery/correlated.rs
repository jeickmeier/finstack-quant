//! Logistic-normal recovery with systematic and independent obligor shocks.
//!
//! The location `m` is the recovery at zero total shock. The local scale
//! `sigma` is not the standard deviation of the bounded recovery distribution.

use super::{validate_correlated_inputs, validate_finite_recovery_input, RecoveryModel, Result};
use finstack_quant_core::math::{
    integration::adaptive_simpson,
    special_functions::{norm_cdf, norm_pdf},
};

/// Bounded logistic-normal recovery with systematic and obligor variation.
///
/// ```text
/// R(z,e) = sigmoid(logit(m) + sigma/[m(1-m)] * (rho*z + sqrt(1-rho²)*e))
/// ```
///
/// `z` and `e` are independent standard normals. Positive `rho` lowers
/// recovery in low-factor credit stress. Conditional recovery is `E_e[R(z,e)]`;
/// unconditional recovery integrates both normals and is independent of `rho`.
/// Locations zero and one, and zero scale, give exact constant distributions.
#[derive(Debug, Clone)]
pub struct CorrelatedRecovery {
    mean_recovery: f64,
    recovery_volatility: f64,
    factor_correlation: f64,
    unconditional_expected_recovery: f64,
}

impl CorrelatedRecovery {
    /// Construct the canonical logistic-normal recovery distribution.
    ///
    /// # Arguments
    ///
    /// * `mean` - Zero-total-shock recovery location, finite in [0, 1], in
    ///   decimals. This is not the unconditional expected recovery.
    /// * `vol` - Local shock scale, finite in [0, 0.5], in recovery decimals;
    ///   this is not the realized recovery standard deviation.
    /// * `corr` - Systematic Gaussian loading, finite in [-1, 1]. Positive
    ///   values lower recovery when the systematic credit factor falls.
    ///
    /// # Errors
    ///
    /// Returns a validation error for invalid parameters, or a core computation
    /// error if integration of the unconditional mean fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_models::correlation::{CorrelatedRecovery, RecoveryModel};
    /// let model = CorrelatedRecovery::new(0.4, 0.25, 0.4)?;
    /// assert_eq!(model.sample_recovery(0.0, 0.0)?, 0.4);
    /// assert!(model.conditional_recovery(-2.0)? < model.conditional_recovery(2.0)?);
    /// # Ok::<(), finstack_quant_models::correlation::Error>(())
    /// ```
    pub fn new(mean: f64, vol: f64, corr: f64) -> Result<Self> {
        validate_correlated_inputs(mean, vol, corr)?;
        let mut model = Self {
            mean_recovery: mean,
            recovery_volatility: vol,
            factor_correlation: corr,
            unconditional_expected_recovery: mean,
        };
        if model.is_stochastic() {
            model.unconditional_expected_recovery = model.gaussian_expectation(0.0, 1.0)?;
        }
        Ok(model)
    }

    fn center(&self) -> f64 {
        self.mean_recovery.ln() - (-self.mean_recovery).ln_1p()
    }

    fn logit_scale(&self) -> f64 {
        self.recovery_volatility / (self.mean_recovery * (1.0 - self.mean_recovery))
    }

    fn sample_unchecked(&self, shock: f64) -> f64 {
        if !self.is_stochastic() || shock == 0.0 {
            return self.mean_recovery;
        }
        let scale = self.logit_scale();
        let displacement = if scale.is_finite() {
            scale * shock
        } else {
            // Divide before multiplying: vol*shock can underflow even when
            // the physical shock represents a finite logit displacement.
            (shock / (self.mean_recovery * (1.0 - self.mean_recovery))) * self.recovery_volatility
        };
        sigmoid(self.center() + displacement)
    }

    /// Integrate an affine Gaussian shock with a supplied mean and deviation.
    fn gaussian_expectation(&self, shock_mean: f64, shock_std: f64) -> Result<f64> {
        if !self.is_stochastic() || shock_std == 0.0 {
            return Ok(self.sample_unchecked(shock_mean));
        }
        let scale = self.logit_scale();
        let deviation = scale * shock_std;
        let baseline = self.sample_unchecked(shock_mean);
        if deviation <= 1.0 {
            // Pair corrections around the exact baseline. Integrating the
            // density itself would lose exact constants in the Gaussian tails.
            let correction = adaptive_simpson(
                |x| {
                    (self.sample_unchecked(shock_mean + shock_std * x)
                        + self.sample_unchecked(shock_mean - shock_std * x)
                        - 2.0 * baseline)
                        * norm_pdf(x)
                },
                0.0,
                10.0,
                1e-12,
                24,
            )?;
            return Ok(baseline + correction);
        }

        // Integration by parts: E[S(C+d*e)] = integral S'(x) Phi((C-x)/d) dx.
        // Pair around Phi(C/d); the logistic tail beyond 36 is below 5e-16.
        // This coordinate remains resolved for arbitrarily steep logits.
        let a = shock_mean / shock_std + self.center() / deviation;
        let base = norm_cdf(a);
        let correction = adaptive_simpson(
            |x| {
                let tail = (-x).exp();
                let logistic_density = tail / (1.0 + tail).powi(2);
                logistic_density
                    * (norm_cdf(a + x / deviation) + norm_cdf(a - x / deviation) - 2.0 * base)
            },
            0.0,
            36.0,
            1e-12,
            24,
        )?;
        Ok(base + correction)
    }
}

fn sigmoid(value: f64) -> f64 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let numerator = value.exp();
        numerator / (1.0 + numerator)
    }
}

impl RecoveryModel for CorrelatedRecovery {
    fn expected_recovery(&self) -> f64 {
        self.unconditional_expected_recovery
    }

    fn conditional_recovery(&self, market_factor: f64) -> Result<f64> {
        validate_finite_recovery_input("market_factor", market_factor)?;
        if self.factor_correlation == 0.0 {
            return Ok(self.unconditional_expected_recovery);
        }
        self.gaussian_expectation(
            self.factor_correlation * market_factor,
            (1.0 - self.factor_correlation * self.factor_correlation).sqrt(),
        )
    }

    fn sample_recovery(&self, market_factor: f64, idiosyncratic_normal: f64) -> Result<f64> {
        validate_finite_recovery_input("market_factor", market_factor)?;
        validate_finite_recovery_input("idiosyncratic_normal", idiosyncratic_normal)?;
        let residual = (1.0 - self.factor_correlation * self.factor_correlation).sqrt();
        Ok(self.sample_unchecked(
            self.factor_correlation * market_factor + residual * idiosyncratic_normal,
        ))
    }

    fn has_idiosyncratic_dispersion(&self) -> bool {
        self.is_stochastic() && self.factor_correlation.abs() < 1.0
    }

    fn is_stochastic(&self) -> bool {
        self.recovery_volatility > 0.0 && self.mean_recovery > 0.0 && self.mean_recovery < 1.0
    }

    fn recovery_volatility(&self) -> f64 {
        self.recovery_volatility
    }

    fn model_name(&self) -> &'static str {
        "Market-Correlated Stochastic Recovery"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent composite Simpson rule evaluating the law directly, with
    // neither the production expectation transform nor its sampling helper.
    fn dense_mean(mean: f64, vol: f64, rho: f64, z: f64) -> f64 {
        let steps = 40_000;
        let h = 20.0 / f64::from(steps);
        let law = |e: f64| {
            let center = (mean / (1.0 - mean)).ln();
            let shock = rho * z + (1.0 - rho * rho).sqrt() * e;
            let recovery = 1.0 / (1.0 + (-center - vol / (mean * (1.0 - mean)) * shock).exp());
            recovery * (-e * e / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt()
        };
        let sum: f64 = (1..steps)
            .map(|i| law(-10.0 + f64::from(i) * h) * if i % 2 == 0 { 2.0 } else { 4.0 })
            .sum();
        h / 3.0 * (law(-10.0) + sum + law(10.0))
    }

    #[test]
    fn conditional_and_unconditional_means_match_independent_quadrature() {
        for mean in [0.01, 0.4, 0.5, 0.99] {
            for vol in [0.01, 0.25, 0.5] {
                for rho in [-1.0, -0.4, 0.0, 0.4, 1.0] {
                    let model = CorrelatedRecovery::new(mean, vol, rho).expect("valid parameters");
                    let unconditional = dense_mean(mean, vol, 0.0, 0.0);
                    assert!((model.expected_recovery() - unconditional).abs() < 1e-10);
                    for z in [-2.0, 0.0, 2.0] {
                        let actual = model.conditional_recovery(z).expect("conditional mean");
                        let expected = dense_mean(mean, vol, rho, z);
                        assert!(
                            (actual - expected).abs() < 1e-10,
                            "{mean}/{vol}/{rho}/{z}: {actual} vs {expected}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn samples_use_both_shocks_and_location_is_not_conditional_mean() {
        let model = CorrelatedRecovery::new(0.4, 0.25, 0.4).expect("model");
        assert_eq!(model.sample_recovery(0.0, 0.0).expect("sample"), 0.4);
        assert!((model.conditional_recovery(0.0).expect("mean") - 0.4).abs() > 1e-3);
        for (z, e) in [(-2.0, 1.0), (0.0, -1.0), (2.0, -0.5)] {
            let shock = 0.4 * z + (1.0_f64 - 0.16).sqrt() * e;
            let expected = 1.0 / (1.0 + (-(0.4_f64 / 0.6).ln() - 0.25 / 0.24 * shock).exp());
            assert!((model.sample_recovery(z, e).expect("sample") - expected).abs() < 1e-14);
        }
        let independent = CorrelatedRecovery::new(0.4, 0.25, 0.0).expect("model");
        assert!(independent.has_idiosyncratic_dispersion());
        assert_ne!(
            independent.sample_recovery(0.0, -1.0).expect("sample"),
            independent.sample_recovery(0.0, 1.0).expect("sample")
        );
        assert_eq!(
            independent.conditional_recovery(4.0).expect("mean"),
            independent.expected_recovery()
        );
    }

    #[test]
    fn constant_and_perfect_systematic_cases_are_exact() {
        for (mean, vol) in [(0.0, 0.5), (1.0, 0.5), (0.4, 0.0)] {
            let model = CorrelatedRecovery::new(mean, vol, 0.4).expect("model");
            assert!(!model.is_stochastic());
            assert!(!model.has_idiosyncratic_dispersion());
            assert_eq!(model.expected_recovery(), mean);
            for z in [-100.0, 0.0, 100.0] {
                assert_eq!(model.conditional_recovery(z).expect("mean"), mean);
                assert_eq!(model.sample_recovery(z, -z).expect("sample"), mean);
            }
        }
        for rho in [-1.0, 1.0] {
            let model = CorrelatedRecovery::new(0.4, 0.25, rho).expect("model");
            assert!(!model.has_idiosyncratic_dispersion());
            for z in [-2.0, 0.0, 2.0] {
                assert_eq!(
                    model.conditional_recovery(z).expect("mean"),
                    model.sample_recovery(z, 7.0).expect("sample")
                );
            }
        }
    }

    #[test]
    fn tiny_locations_and_scales_do_not_overflow_or_erase_shocks() {
        for mean in [f64::from_bits(1), f64::MIN_POSITIVE, 0.01, 0.99] {
            let model = CorrelatedRecovery::new(mean, 0.5, 0.4).expect("model");
            assert!(model.expected_recovery().is_finite());
            for z in [-100.0, 0.0, 100.0] {
                assert!((0.0..=1.0).contains(&model.conditional_recovery(z).expect("mean")));
            }
            assert_eq!(model.sample_recovery(0.0, 0.0).expect("sample"), mean);
        }
        let model = CorrelatedRecovery::new(f64::from_bits(1), 0.5, 1.0).expect("model");
        let tiny = f64::from_bits(1);
        let actual = model.sample_recovery(tiny, 0.0).expect("sample");
        assert!(actual >= tiny && actual.is_finite());
    }

    #[test]
    fn nonfinite_factors_are_errors_for_every_model() {
        for model in [
            RecoverySpec::constant(0.4)
                .expect("spec")
                .build()
                .expect("model"),
            RecoverySpec::market_correlated(0.4, 0.25, 0.4)
                .expect("spec")
                .build()
                .expect("model"),
        ] {
            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(model.conditional_recovery(bad).is_err());
                assert!(model.conditional_lgd(bad).is_err());
                assert!(model.sample_recovery(bad, 0.0).is_err());
                assert!(model.sample_recovery(0.0, bad).is_err());
            }
        }
    }

    use crate::correlation::RecoverySpec;
}
