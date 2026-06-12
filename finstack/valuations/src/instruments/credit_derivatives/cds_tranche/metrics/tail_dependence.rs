//! Tail dependence metric for copula diagnostics.
//!
//! Measures the probability of joint extreme defaults - a key indicator
//! of whether the copula model adequately captures stress scenarios.
//!
//! # Definition
//!
//! Lower tail dependence coefficient:
//! ```text
//! λ_L = lim_{u→0} P(U₂ ≤ u | U₁ ≤ u)
//! ```
//!
//! - **Gaussian copula**: λ_L = 0 (no tail dependence)
//! - **Student-t copula**: λ_L > 0 (positive tail dependence)
//! - **Random Factor Loading**: no closed form — reported as `NaN` per the
//!   [`Copula::tail_dependence`](crate::correlation::copula::Copula::tail_dependence)
//!   contract (use `RandomFactorLoadingCopula::stress_correlation_proxy` for a
//!   heuristic stress gauge)
//!
//! # Financial Interpretation
//!
//! - λ_L = 0: Extreme joint defaults are "rare" (Gaussian assumption)
//! - λ_L > 0: Extreme joint defaults cluster (realistic for stress)
//!
//! Higher tail dependence means:
//! - Equity tranches: Higher expected loss in stress
//! - Senior tranches: Higher unexpected loss risk
//!
//! # Implementation
//!
//! Delegates to the copula built from the same pricer configuration used by
//! the tranche pricing path (`CDSTranchePricer::config().copula_spec`), so the
//! reported λ_L is always computed by the exact model implementation rather
//! than a re-derived local formula that could drift from it.

use crate::instruments::credit_derivatives::cds_tranche::pricer::CDSTranchePricer;
use crate::instruments::credit_derivatives::cds_tranche::CDSTranche;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Calculator for tail dependence coefficient.
///
/// Returns the lower tail dependence coefficient λ_L of the copula model
/// being used for tranche pricing. This is a diagnostic metric that
/// indicates whether the model captures joint extreme defaults.
pub(crate) struct TailDependenceCalculator;

impl MetricCalculator for TailDependenceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche = context
            .instrument
            .as_any()
            .downcast_ref::<CDSTranche>()
            .ok_or(finstack_core::Error::Input(
                finstack_core::InputError::Invalid,
            ))?;

        // Get the credit index data to determine correlation
        let Ok(index_data) = context.curves.get_credit_index(&tranche.credit_index_id) else {
            return Ok(f64::NAN);
        };
        let correlation = index_data
            .base_correlation_curve
            .correlation(tranche.detach_pct);

        // Build the copula from the same configuration the pricing path uses
        // and delegate to its canonical tail-dependence implementation.
        // Models without a closed-form λ_L (e.g. Random Factor Loading)
        // return NaN per the trait contract.
        let pricer = CDSTranchePricer::new();
        let copula = pricer
            .config()
            .copula_spec
            .build()
            .map_err(|e| finstack_core::Error::Validation(e.to_string()))?;

        Ok(copula.tail_dependence(correlation))
    }
}
