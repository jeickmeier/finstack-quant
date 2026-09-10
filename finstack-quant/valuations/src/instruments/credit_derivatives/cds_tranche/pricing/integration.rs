//! Adaptive conditioning-factor integration for bounded pool expectations.

use super::config::CDSTranchePricer;
use finstack_quant_core::math::integration::adaptive_simpson;
use finstack_quant_core::math::{chi_squared_quantile, ln_gamma, norm_pdf};
use finstack_quant_core::{Error, Result};
use finstack_quant_models::correlation::copula::CopulaSpec;
use std::cell::RefCell;

/// The omitted two-sided normal mass is below 1.524e-23. Every capped pool
/// integrand is bounded by one portfolio notional, so this is also a loss bound.
const NORMAL_LIMIT: f64 = 10.0;

/// Partition the conditioning axis before refinement so an initially broad
/// interval cannot skip a localized transition. Each leaf receives a share
/// of the global budget; the core integrator fails if it cannot meet it.
fn integrate_interval(
    f: &dyn Fn(f64) -> Result<f64>,
    lower: f64,
    upper: f64,
    tolerance: f64,
    max_depth: usize,
) -> Result<f64> {
    let failure = RefCell::new(None);
    let checked = |x| {
        if failure.borrow().is_some() {
            return 0.0;
        }
        match f(x) {
            Ok(value) if value.is_finite() => value,
            Ok(_) => {
                *failure.borrow_mut() = Some(Error::Validation(format!(
                    "non-finite tranche integrand at conditioning value {x}"
                )));
                0.0
            }
            Err(error) => {
                *failure.borrow_mut() = Some(error);
                0.0
            }
        }
    };
    let intervals = (upper - lower).ceil().max(1.0) as usize;
    let step = (upper - lower) / intervals as f64;
    let mut result = 0.0;
    for i in 0..intervals {
        let left = lower + i as f64 * step;
        let right = lower + (i + 1) as f64 * step;
        let part = adaptive_simpson(
            checked,
            left,
            right,
            tolerance / intervals as f64,
            max_depth,
        );
        if let Some(error) = failure.borrow_mut().take() {
            return Err(error);
        }
        result += part?;
    }
    Ok(result)
}

fn integrate_normal(
    f: &dyn Fn(f64) -> Result<f64>,
    tolerance: f64,
    max_depth: usize,
) -> Result<f64> {
    integrate_interval(
        &|z| Ok(f(z)? * norm_pdf(z)),
        -NORMAL_LIMIT,
        NORMAL_LIMIT,
        tolerance,
        max_depth,
    )
}

impl CDSTranchePricer {
    /// Integrate a bounded expectation over every copula conditioning factor.
    /// Nested quadrature and omitted factor tails consume separate shares of
    /// `integration_tolerance`. Conditional-name approximation or convolution
    /// discretization error is separate from this quadrature budget.
    pub(super) fn integrate_factors(&self, f: &dyn Fn(&[f64]) -> Result<f64>) -> Result<f64> {
        let tolerance = self.params.integration_tolerance;
        let depth = self.params.integration_max_depth;
        match self.params.copula_spec {
            CopulaSpec::Gaussian => integrate_normal(&|z| f(&[z]), tolerance * 0.5, depth),
            CopulaSpec::StudentT { degrees_of_freedom } => {
                // Shared W ~ Gamma(nu/2, nu/2). Integrate log(W), preserving
                // conditional independence given both Z and W. Explicit
                // quantiles bound omitted Gamma mass by tolerance/8.
                let tail = tolerance / 16.0;
                let lower =
                    (chi_squared_quantile(tail, degrees_of_freedom)? / degrees_of_freedom).ln();
                let upper = (chi_squared_quantile(1.0 - tail, degrees_of_freedom)?
                    / degrees_of_freedom)
                    .ln();
                if !lower.is_finite() || !upper.is_finite() || lower >= upper {
                    return Err(Error::Validation(
                        "could not resolve finite Student-t mixing integration bounds".to_owned(),
                    ));
                }
                let shape = degrees_of_freedom / 2.0;
                let log_normalization = shape * shape.ln() - ln_gamma(shape);
                integrate_interval(
                    &|log_w| {
                        let w = log_w.exp();
                        let density = (log_normalization + shape * log_w - shape * w).exp();
                        Ok(density * integrate_normal(&|z| f(&[z, w]), tolerance / 8.0, depth)?)
                    },
                    lower,
                    upper,
                    tolerance / 8.0,
                    depth,
                )
            }
            CopulaSpec::RandomFactorLoading { .. } | CopulaSpec::MultiFactor => integrate_normal(
                &|second| integrate_normal(&|z| f(&[z, second]), tolerance / 8.0, depth),
                tolerance / 8.0,
                depth,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::credit_derivatives::cds_tranche::CDSTranchePricerConfig;

    #[test]
    fn production_credit_audit_unconverged_integrals_fail() {
        let pricer = CDSTranchePricer::with_params(CDSTranchePricerConfig {
            integration_max_depth: 0,
            ..Default::default()
        })
        .expect("config");
        assert!(pricer.integrate_factors(&|_| Ok(1.0)).is_err());
        assert!(CDSTranchePricer::new()
            .integrate_factors(&|_| Ok(f64::NAN))
            .is_err());
    }

    #[test]
    fn production_credit_audit_all_factor_measures_preserve_mass() {
        for spec in [
            CopulaSpec::Gaussian,
            CopulaSpec::StudentT {
                degrees_of_freedom: 4.0,
            },
            CopulaSpec::RandomFactorLoading {
                loading_volatility: 0.2,
            },
            CopulaSpec::MultiFactor,
        ] {
            let pricer = CDSTranchePricer::with_params(CDSTranchePricerConfig {
                copula_spec: spec.clone(),
                integration_tolerance: 1e-8,
                ..Default::default()
            })
            .expect("config");
            let mass = pricer.integrate_factors(&|_| Ok(1.0)).expect("mass");
            assert!((mass - 1.0).abs() < 1e-8, "{spec:?}: mass={mass}");
        }
    }
}
