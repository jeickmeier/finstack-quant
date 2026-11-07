//! Adaptive conditioning-factor integration for bounded pool expectations.

use super::pricer::{CdsTranchePricer, INTEGRATION_MAX_DEPTH, INTEGRATION_TOLERANCE};
use finstack_quant_core::math::integration::{adaptive_simpson, adaptive_simpson_pair};
use finstack_quant_core::math::special_functions::norm_pdf;
use finstack_quant_core::{Error, Result};
use finstack_quant_models::correlation::{CopulaSpec, FactorPairIntegrand};
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

/// Integrate two Gaussian expectations on shared nodes while preserving each
/// component's scalar error budget and the first callback error.
fn integrate_normal_pair(
    f: &dyn Fn(f64) -> Result<(f64, f64)>,
    tolerance: f64,
) -> Result<(f64, f64)> {
    let failure = RefCell::new(None);
    let checked = |z| {
        if failure.borrow().is_some() {
            return (0.0, 0.0);
        }
        match f(z) {
            Ok((first, second)) if first.is_finite() && second.is_finite() => {
                let density = norm_pdf(z);
                (first * density, second * density)
            }
            Ok(_) => {
                *failure.borrow_mut() = Some(Error::Validation(format!(
                    "non-finite tranche integrand at conditioning value {z}"
                )));
                (0.0, 0.0)
            }
            Err(error) => {
                *failure.borrow_mut() = Some(error);
                (0.0, 0.0)
            }
        }
    };
    let lower = -NORMAL_LIMIT;
    let upper = NORMAL_LIMIT;
    let intervals = (upper - lower).ceil().max(1.0) as usize;
    let step = (upper - lower) / intervals as f64;
    let mut result = (0.0, 0.0);
    for i in 0..intervals {
        let left = lower + i as f64 * step;
        let right = lower + (i + 1) as f64 * step;
        let part = adaptive_simpson_pair(
            checked,
            left,
            right,
            tolerance / intervals as f64,
            INTEGRATION_MAX_DEPTH,
        );
        if let Some(error) = failure.borrow_mut().take() {
            return Err(error);
        }
        let (first, second) = part?;
        result.0 += first;
        result.1 += second;
    }
    Ok(result)
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
    /// Gaussian adaptive integration and Student-t product-Gauss pricing each
    /// accumulate both components on shared nodes. RFL and multifactor paths
    /// retain their established separate nested integrals.
    pub(super) fn integrate_factors_pair(&self, f: &FactorPairIntegrand<'_>) -> Result<(f64, f64)> {
        match self.copula_spec {
            CopulaSpec::Gaussian => {
                integrate_normal_pair(&|z| f(&[z]), INTEGRATION_TOLERANCE * 0.5)
            }
            CopulaSpec::StudentT { .. } => self.copula().try_integrate_pair(f),
            CopulaSpec::RandomFactorLoading { .. } | CopulaSpec::MultiFactor => Ok((
                self.integrate_factors(&|factors| f(factors).map(|pair| pair.0))?,
                self.integrate_factors(&|factors| f(factors).map(|pair| pair.1))?,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::math::special_functions::norm_cdf;
    use std::cell::Cell;

    #[test]
    fn gaussian_pair_matches_normal_mass_and_second_moment() {
        let pricer = CdsTranchePricer::new();
        for swapped in [false, true] {
            let (first, second) = pricer
                .integrate_factors_pair(&|factors| {
                    let square = factors[0] * factors[0];
                    Ok(if swapped {
                        (square, 1.0)
                    } else {
                        (1.0, square)
                    })
                })
                .expect("Gaussian moments");
            assert!((first - 1.0).abs() < 2e-10);
            assert!((second - 1.0).abs() < 2e-10);
        }
    }

    #[test]
    fn gaussian_pair_preserves_small_localized_component() {
        let pricer = CdsTranchePricer::new();
        let boundary = 0.123_456_7;
        let amplitude = 1e-4;
        let expected = amplitude * (1.0 - norm_cdf(boundary));
        for swapped in [false, true] {
            let (first, second) = pricer
                .integrate_factors_pair(&|factors| {
                    let small = if factors[0] >= boundary {
                        amplitude
                    } else {
                        0.0
                    };
                    Ok(if swapped { (small, 1.0) } else { (1.0, small) })
                })
                .expect("localized Gaussian integral");
            let (mass, small) = if swapped {
                (second, first)
            } else {
                (first, second)
            };
            assert!((mass - 1.0).abs() < 2e-10);
            assert!((small - expected).abs() < 2e-10);
        }
    }

    #[test]
    fn gaussian_pair_shares_conditional_evaluations() {
        let pricer = CdsTranchePricer::new();
        let paired_calls = Cell::new(0);
        let paired = pricer
            .integrate_factors_pair(&|factors| {
                paired_calls.set(paired_calls.get() + 1);
                let value = factors[0] * factors[0];
                Ok((value, value))
            })
            .expect("pair");
        let scalar_calls = Cell::new(0);
        for component in [paired.0, paired.1] {
            let scalar = pricer
                .integrate_factors(&|factors| {
                    scalar_calls.set(scalar_calls.get() + 1);
                    Ok(factors[0] * factors[0])
                })
                .expect("scalar");
            assert!((component - scalar).abs() < 2e-10);
        }
        assert!(paired_calls.get() < scalar_calls.get());
    }

    #[test]
    fn gaussian_pair_preserves_first_callback_error() {
        let calls = Cell::new(0);
        let result = CdsTranchePricer::new().integrate_factors_pair(&|_| {
            let call = calls.get();
            calls.set(call + 1);
            Err(Error::Validation(if call == 0 {
                "first domain failure".into()
            } else {
                "later domain failure".into()
            }))
        });
        assert_eq!(calls.get(), 1);
        assert!(
            matches!(result, Err(Error::Validation(message)) if message == "first domain failure")
        );
    }

    #[test]
    fn gaussian_pair_rejects_nonfinite_value_in_either_component() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for pair in [(value, 1.0), (1.0, value)] {
                let result = CdsTranchePricer::new().integrate_factors_pair(&|_| Ok(pair));
                assert!(
                    matches!(result, Err(Error::Validation(message)) if message.starts_with("non-finite tranche integrand"))
                );
            }
        }
    }

    #[test]
    fn other_gaussian_family_pairs_keep_scalar_results_bitwise() {
        for spec in [
            CopulaSpec::RandomFactorLoading {
                loading_volatility: 0.2,
            },
            CopulaSpec::MultiFactor,
        ] {
            let pricer = CdsTranchePricer::with_copula(spec).expect("copula");
            let pair = pricer
                .integrate_factors_pair(&|factors| Ok((factors[0].sin(), factors[1].cos())))
                .expect("pair");
            let first = pricer
                .integrate_factors(&|factors| Ok(factors[0].sin()))
                .expect("first scalar");
            let second = pricer
                .integrate_factors(&|factors| Ok(factors[1].cos()))
                .expect("second scalar");
            assert_eq!(pair.0.to_bits(), first.to_bits());
            assert_eq!(pair.1.to_bits(), second.to_bits());
        }
    }

    #[test]
    fn production_credit_audit_non_finite_integrands_fail() {
        assert!(
            CdsTranchePricer::new()
                .integrate_factors(&|_| Ok(f64::NAN))
                .is_err()
        );
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
