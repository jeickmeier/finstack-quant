//! Factor-correlated default model.
//!
//! Follows the canonical copula sign convention: a LOW systematic factor
//! (`Z < 0`) is the stress state, so a positive `factor_loading` scales the
//! base default curve UP when `Z` falls (`exp(−loading·Z·σ)`).

use super::traits::{MacroCreditFactors, StochasticDefault};
use crate::cashflow::builder::specs::DefaultModelSpec;

/// Default model that shocks a deterministic default curve by a systematic factor.
#[derive(Debug, Clone)]
pub(crate) struct FactorCorrelatedDefault {
    base_spec: DefaultModelSpec,
    factor_loading: f64,
    cdr_volatility: f64,
}

impl FactorCorrelatedDefault {
    /// Create a factor-correlated default model.
    pub(crate) fn new(
        base_spec: DefaultModelSpec,
        factor_loading: f64,
        cdr_volatility: f64,
    ) -> Self {
        Self {
            base_spec,
            factor_loading: factor_loading.clamp(-1.0, 1.0),
            cdr_volatility: cdr_volatility.clamp(0.0, 1.0),
        }
    }

    fn base_mdr_at_seasoning(&self, seasoning: u32) -> f64 {
        self.base_spec.mdr(seasoning).unwrap_or(0.0).clamp(0.0, 1.0)
    }
}

impl StochasticDefault for FactorCorrelatedDefault {
    fn conditional_mdr(
        &self,
        seasoning: u32,
        factors: &[f64],
        _macro_factors: &MacroCreditFactors,
    ) -> f64 {
        let base_mdr = self.base_mdr_at_seasoning(seasoning);
        if base_mdr <= f64::EPSILON {
            return 0.0;
        }

        let z = factors.first().copied().unwrap_or(0.0);
        let shock = (-self.factor_loading * z * self.cdr_volatility).exp();
        (base_mdr * shock).clamp(0.0, 0.50)
    }

    /// Reports `|factor_loading|` as a correlation PROXY: this model has no
    /// separate asset-correlation parameter — the loading is the only
    /// dependence parameter, and under a one-factor structure the implied
    /// asset correlation is the squared loading, not the loading itself.
    /// Callers needing a true asset correlation should use a copula-based
    /// model.
    fn correlation(&self) -> f64 {
        self.factor_loading.abs().clamp(0.0, 0.99)
    }

    fn model_name(&self) -> &'static str {
        "Factor-Correlated Default"
    }

    fn expected_mdr(&self, seasoning: u32) -> f64 {
        self.base_mdr_at_seasoning(seasoning)
    }
}
