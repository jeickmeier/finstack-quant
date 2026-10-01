//! Factor-correlated default model.
//!
//! Follows the canonical copula sign convention: a LOW systematic factor
//! (`Z < 0`) is the stress state, so a positive `factor_loading` scales the
//! base default curve UP when `Z` falls (`exp(−loading·Z·σ)`).

use super::traits::{MacroCreditFactors, StochasticDefault};
use finstack_quant_cashflows::builder::specs::DefaultModelSpec;

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
        let base_mdr = self.base_mdr_at_seasoning(seasoning);
        if base_mdr <= f64::EPSILON {
            return 0.0;
        }
        let sigma = (self.factor_loading * self.cdr_volatility).abs();
        if sigma == 0.0 {
            return base_mdr.min(0.50);
        }
        // E[min(cap, b exp(sigma Z))] has an exact truncated-lognormal
        // expression. The shock sign does not affect its unconditional mean.
        let cutoff = (0.50 / base_mdr).ln() / sigma;
        base_mdr * (0.5 * sigma * sigma).exp() * finstack_quant_core::math::norm_cdf(cutoff - sigma)
            + 0.50 * finstack_quant_core::math::norm_cdf(-cutoff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_mdr_matches_conditional_log_normal_mean_including_cap() {
        for base_cdr in [0.20, 0.99] {
            let model =
                FactorCorrelatedDefault::new(DefaultModelSpec::constant_cdr(base_cdr), 1.0, 1.0);
            let step = 0.0001;
            let integrated: f64 = (0..200_000)
                .map(|i| {
                    let z = -10.0 + (f64::from(i) + 0.5) * step;
                    let density = (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
                    model.conditional_mdr(30, &[z], &MacroCreditFactors::default()) * density * step
                })
                .sum();
            assert!((model.expected_mdr(30) - integrated).abs() < 1e-8);
        }
    }

    #[test]
    fn deterministic_expected_mdr_matches_the_conditional_cap() {
        let model = FactorCorrelatedDefault::new(DefaultModelSpec::constant_cdr(1.0), 1.0, 0.0);
        assert_eq!(
            model.expected_mdr(30),
            model.conditional_mdr(30, &[0.0], &MacroCreditFactors::default())
        );
    }
}
