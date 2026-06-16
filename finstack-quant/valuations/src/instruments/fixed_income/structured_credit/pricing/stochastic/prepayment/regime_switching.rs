//! Regime-switching prepayment model.

use super::traits::StochasticPrepayment;
use crate::instruments::fixed_income::structured_credit::utils::rates::cpr_to_smm;

/// Two-state Markov prepayment model with factor-shocked CPR.
#[derive(Debug, Clone)]
pub(crate) struct RegimeSwitchingPrepay {
    low_cpr: f64,
    high_cpr: f64,
    transition_up: f64,
    transition_down: f64,
    factor_loading: f64,
    cpr_volatility: f64,
}

impl RegimeSwitchingPrepay {
    /// Create a regime-switching prepayment model.
    pub(crate) fn new(
        low_cpr: f64,
        high_cpr: f64,
        transition_up: f64,
        transition_down: f64,
        factor_loading: f64,
        cpr_volatility: f64,
    ) -> Self {
        Self {
            low_cpr: low_cpr.clamp(0.0, 1.0),
            high_cpr: high_cpr.clamp(0.0, 1.0),
            transition_up: transition_up.clamp(0.0, 1.0),
            transition_down: transition_down.clamp(0.0, 1.0),
            factor_loading: factor_loading.clamp(-1.0, 1.0),
            cpr_volatility: cpr_volatility.clamp(0.0, 1.0),
        }
    }

    fn high_regime_probability(&self, seasoning: u32) -> f64 {
        let up = self.transition_up;
        let down = self.transition_down;
        let total = up + down;
        if total <= f64::EPSILON {
            return 0.0;
        }

        let stationary_high = up / total;
        let persistence = (1.0 - total).clamp(-1.0, 1.0);
        (stationary_high * (1.0 - persistence.powi(seasoning as i32))).clamp(0.0, 1.0)
    }

    fn base_cpr(&self, seasoning: u32) -> f64 {
        let high_prob = self.high_regime_probability(seasoning);
        self.low_cpr * (1.0 - high_prob) + self.high_cpr * high_prob
    }
}

impl StochasticPrepayment for RegimeSwitchingPrepay {
    fn conditional_smm(
        &self,
        seasoning: u32,
        factors: &[f64],
        _market_rate: f64,
        burnout: f64,
    ) -> f64 {
        let base_cpr = self.base_cpr(seasoning);
        if base_cpr <= f64::EPSILON {
            return 0.0;
        }

        let z = factors.first().copied().unwrap_or(0.0);
        let shock = (self.factor_loading * z * self.cpr_volatility).exp();
        cpr_to_smm((base_cpr * shock * burnout).clamp(0.0, 1.0))
    }

    fn expected_smm(&self, seasoning: u32) -> f64 {
        cpr_to_smm(self.base_cpr(seasoning))
    }

    fn factor_loading(&self) -> f64 {
        self.factor_loading
    }

    fn model_name(&self) -> &'static str {
        "Regime-Switching Prepayment"
    }
}
