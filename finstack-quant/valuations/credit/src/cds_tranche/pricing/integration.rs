//! Adaptive conditioning-factor integration for bounded pool expectations.

use super::config::{CdsTranchePricer, INTEGRATION_MAX_DEPTH, INTEGRATION_TOLERANCE};
use finstack_quant_core::math::integration::adaptive_simpson;
use finstack_quant_core::math::norm_pdf;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::correlation::copula::{CopulaSpec, FactorPairIntegrand};
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
            INTEGRATION_MAX_DEPTH,
        );
        if let Some(error) = failure.borrow_mut().take() {
            return Err(error);
        }
        result += part?;
    }
    Ok(result)
}

fn integrate_normal(f: &dyn Fn(f64) -> Result<f64>, tolerance: f64) -> Result<f64> {
    integrate_interval(
        &|z| Ok(f(z)? * norm_pdf(z)),
        -NORMAL_LIMIT,
        NORMAL_LIMIT,
        tolerance,
    )
}

impl CdsTranchePricer {
    /// Integrate a bounded expectation over every copula conditioning factor.
    /// Nested quadrature and omitted factor tails consume separate shares of
    /// `INTEGRATION_TOLERANCE`. Conditional convolution discretization error
    /// is separate from this quadrature budget.
    pub(super) fn integrate_factors(&self, f: &dyn Fn(&[f64]) -> Result<f64>) -> Result<f64> {
        let tolerance = INTEGRATION_TOLERANCE;
        match self.copula_spec {
            CopulaSpec::Gaussian => integrate_normal(&|z| f(&[z]), tolerance * 0.5),
            CopulaSpec::StudentT { .. } => self.copula().try_integrate_fn(f),
            CopulaSpec::RandomFactorLoading { .. } | CopulaSpec::MultiFactor => integrate_normal(
                &|second| integrate_normal(&|z| f(&[z, second]), tolerance / 8.0),
                tolerance / 8.0,
            ),
        }
    }

    /// Integrate a two-valued bounded expectation over the copula factors.
    ///
    /// Student-t product-Gauss pricing accumulates both components in one
    /// node sweep. Gaussian-family paths evaluate each component separately.
    pub(super) fn integrate_factors_pair(&self, f: &FactorPairIntegrand<'_>) -> Result<(f64, f64)> {
        match self.copula_spec {
            CopulaSpec::StudentT { .. } => self.copula().try_integrate_pair(f),
            _ => Ok((
                self.integrate_factors(&|factors| f(factors).map(|pair| pair.0))?,
                self.integrate_factors(&|factors| f(factors).map(|pair| pair.1))?,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_credit_audit_non_finite_integrands_fail() {
        assert!(CdsTranchePricer::new()
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
            let pricer = CdsTranchePricer::with_copula(spec.clone()).expect("copula");
            let mass = pricer.integrate_factors(&|_| Ok(1.0)).expect("mass");
            assert!((mass - 1.0).abs() < 1e-8, "{spec:?}: mass={mass}");
        }
    }

    #[test]
    fn student_t_uses_product_gauss_node_count() {
        let pricer = CdsTranchePricer::with_copula(CopulaSpec::student_t(6.0).expect("valid df"))
            .expect("copula");
        let evals = std::cell::Cell::new(0usize);
        let mass = pricer
            .integrate_factors(&|_| {
                evals.set(evals.get() + 1);
                Ok(1.0)
            })
            .expect("mass");
        assert!((mass - 1.0).abs() < 1e-8, "mass={mass}");
        let n = evals.get();
        assert!(
            (100..=400).contains(&n),
            "Student-t product Gauss should evaluate at most 20×20 nodes, got {n}"
        );
    }

    #[test]
    fn student_t_pair_matches_two_scalar_integrals() {
        let pricer = CdsTranchePricer::with_copula(CopulaSpec::student_t(6.0).expect("valid df"))
            .expect("copula");
        let (first, second) = pricer
            .integrate_factors_pair(&|factors| {
                let z = factors[0];
                Ok((z * z, 1.0))
            })
            .expect("pair");
        let scalar_first = pricer
            .integrate_factors(&|factors| Ok(factors[0] * factors[0]))
            .expect("first");
        let scalar_second = pricer.integrate_factors(&|_| Ok(1.0)).expect("second");
        assert!((first - scalar_first).abs() < 1e-12);
        assert!((second - scalar_second).abs() < 1e-12);
    }
}
