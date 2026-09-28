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
//!   [`Copula::tail_dependence`](finstack_quant_models::correlation::copula::Copula::tail_dependence)
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
//! the tranche pricing path (`CdsTranchePricer::config().copula_spec`), so the
//! reported λ_L is always computed by the exact model implementation rather
//! than a re-derived local formula that could drift from it.

use crate::instruments::credit_derivatives::cds_tranche::pricing::CdsTranchePricer;
use crate::instruments::credit_derivatives::cds_tranche::CdsTranche;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::Result;
use std::sync::Arc;

/// Calculator for tail dependence coefficient.
///
/// Returns the lower tail dependence coefficient λ_L of the copula model
/// being used for tranche pricing. This is a diagnostic metric that
/// indicates whether the model captures joint extreme defaults.
pub(crate) struct TailDependenceCalculator;

fn credit_index_for_tail_dependence(
    tranche: &CdsTranche,
    market: &MarketContext,
) -> Result<Arc<CreditIndexData>> {
    market
        .get_credit_index(&tranche.credit_index_id)
        .map_err(|error| {
            finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                id: format!(
                    "Credit index '{}' required for tranche '{}' tail dependence: {error}",
                    tranche.credit_index_id, tranche.id
                ),
            })
        })
}

impl MetricCalculator for TailDependenceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche = context.instrument_as::<CdsTranche>()?;

        let index_data = credit_index_for_tail_dependence(tranche, &context.curves)?;
        let correlation = index_data
            .base_correlation_curve
            .correlation(tranche.detach_pct);

        // Build the copula from the same configuration the pricing path uses
        // and delegate to its canonical tail-dependence implementation.
        // Models without a closed-form λ_L (e.g. Random Factor Loading)
        // return NaN per the trait contract.
        let pricer = CdsTranchePricer::new();
        let copula = pricer
            .get_config()
            .copula_spec
            .build()
            .map_err(|e| finstack_quant_core::Error::Validation(e.to_string()))?;

        Ok(copula.tail_dependence(correlation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_credit_index_is_an_error_not_nan() {
        let tranche = CdsTranche::example().expect("example");
        let error = credit_index_for_tail_dependence(&tranche, &MarketContext::new())
            .expect_err("missing credit index must fail");
        assert!(error.to_string().contains(tranche.credit_index_id.as_str()));
    }
}
