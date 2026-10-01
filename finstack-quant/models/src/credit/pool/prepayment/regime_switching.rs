//! Regime-switching prepayment model.

use super::super::{clamped_cpr_to_smm, expected_shocked_smm};
use super::traits::{PrepaymentState, StochasticPrepayment};

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
        if total == 0.0 {
            return 0.0;
        }

        let stationary_high = up / total;
        if total <= 1.0 {
            // Keep rare transitions accurate even when 1 - total rounds to 1.
            let transition_mass = if seasoning == 0 {
                0.0
            } else {
                -(f64::from(seasoning) * (-total).ln_1p()).exp_m1()
            };
            return (stationary_high * transition_mass).clamp(0.0, 1.0);
        }
        let persistence = (1.0 - total).clamp(-1.0, 1.0);
        let power = persistence.abs().powf(f64::from(seasoning));
        let signed_power = if persistence < 0.0 && seasoning % 2 == 1 {
            -power
        } else {
            power
        };
        (stationary_high * (1.0 - signed_power)).clamp(0.0, 1.0)
    }

    fn state_smm(&self, high_regime: bool, factors: &[f64], burnout: f64) -> f64 {
        let cpr = if high_regime {
            self.high_cpr
        } else {
            self.low_cpr
        };
        if cpr == 0.0 || burnout == 0.0 {
            return 0.0;
        }
        let z = factors.first().copied().unwrap_or(0.0);
        let shock = (self.factor_loading * z * self.cpr_volatility).exp();
        clamped_cpr_to_smm(cpr * shock * burnout)
    }
}

impl StochasticPrepayment for RegimeSwitchingPrepay {
    fn initial_state(&self, seasoning: u32, uniform: f64) -> PrepaymentState {
        PrepaymentState {
            high_regime: uniform < self.high_regime_probability(seasoning),
        }
    }

    fn sample_smm(
        &self,
        _seasoning: u32,
        factors: &[f64],
        _market_rate: f64,
        burnout: f64,
        state: &mut PrepaymentState,
        uniform: f64,
    ) -> f64 {
        state.high_regime = if state.high_regime {
            uniform >= self.transition_down
        } else {
            uniform < self.transition_up
        };
        self.state_smm(state.high_regime, factors, burnout)
    }

    fn conditional_smm(
        &self,
        seasoning: u32,
        factors: &[f64],
        _market_rate: f64,
        burnout: f64,
    ) -> f64 {
        let high_prob = self.high_regime_probability(seasoning);
        (1.0 - high_prob) * self.state_smm(false, factors, burnout)
            + high_prob * self.state_smm(true, factors, burnout)
    }

    fn expected_smm(&self, seasoning: u32) -> f64 {
        let high_prob = self.high_regime_probability(seasoning);
        let volatility = self.factor_loading * self.cpr_volatility;
        (1.0 - high_prob) * expected_shocked_smm(self.low_cpr, volatility)
            + high_prob * expected_shocked_smm(self.high_cpr, volatility)
    }

    fn factor_loading(&self) -> f64 {
        self.factor_loading
    }

    fn model_name(&self) -> &'static str {
        "Regime-Switching Prepayment"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_smm_averages_monthly_rates_instead_of_annual_cpr() {
        let model = RegimeSwitchingPrepay::new(0.04, 0.80, 0.5, 0.5, 0.4, 0.0);
        let expected = 0.5 * clamped_cpr_to_smm(0.04) + 0.5 * clamped_cpr_to_smm(0.80);
        assert!((expected - 0.064455390488875).abs() < 1e-14);
        assert!((model.expected_smm(1) - expected).abs() < 1e-14);
        assert!((model.conditional_smm(1, &[0.0], 0.05, 1.0) - expected).abs() < 1e-14);
        assert_eq!(model.expected_smm(0), clamped_cpr_to_smm(0.04));
    }

    #[test]
    fn expected_smm_integrates_factor_shocks_within_each_regime() {
        use finstack_quant_core::math::norm_cdf;

        let model = RegimeSwitchingPrepay::new(0.04, 0.80, 0.3, 0.1, 0.8, 0.5);
        // Independent PDF integral, with z = cap_z - u^12 removing the
        // fractional-power cusp at CPR = 1. Uniform-z midpoint integration
        // converges too slowly here, even with hundreds of thousands of draws.
        // The canonical implementation instead integrates the SMM survival
        // function, so this verifies it through a different identity.
        let per_state_mean = |cpr: f64, panels: u32| {
            let cap_z = -cpr.ln() / 0.4;
            let upper = (cap_z + 10.0).powf(1.0 / 12.0);
            let step = upper / f64::from(panels);
            let integrand = |u: f64| {
                let gap = u.powi(12);
                let z = cap_z - gap;
                let smm = 1.0 - (-(-0.4 * gap).exp_m1()).powf(1.0 / 12.0);
                let density = (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
                smm * density * 12.0 * u.powi(11)
            };
            let mut sum = integrand(0.0) + integrand(upper);
            for i in 1..panels {
                let weight = if i % 2 == 0 { 2.0 } else { 4.0 };
                sum += weight * integrand(f64::from(i) * step);
            }
            // At and above cap_z the clipped annual CPR and monthly SMM
            // are exactly one. The omitted lower tail has mass below 1e-23.
            norm_cdf(-cap_z) + sum * step / 3.0
        };
        let high_probability = 0.75 * (1.0 - 0.6_f64.powi(12));
        for panels in [1_024, 2_048] {
            let integrated = (1.0 - high_probability) * per_state_mean(0.04, panels)
                + high_probability * per_state_mean(0.80, panels);
            assert!((integrated - 0.275051008022911).abs() < 1e-10);
            assert!((model.expected_smm(12) - integrated).abs() < 1e-10);
        }
    }

    #[test]
    fn sampled_path_retains_regime_between_months_even_without_factor_volatility() {
        let model = RegimeSwitchingPrepay::new(0.04, 0.80, 0.25, 0.10, 0.4, 0.0);
        let mut state = model.initial_state(0, 0.0);
        let low = clamped_cpr_to_smm(0.04);
        let high = clamped_cpr_to_smm(0.80);
        let sample = |month, state: &mut PrepaymentState, u| {
            model.sample_smm(month, &[0.0], 0.05, 1.0, state, u)
        };
        assert_eq!(sample(1, &mut state, 0.8), low);
        assert_eq!(sample(2, &mut state, 0.2), high);
        // The same draw stays high, whereas a new low-state path stays low.
        assert_eq!(sample(3, &mut state, 0.8), high);
        assert_eq!(sample(4, &mut state, 0.05), low);
        let mut fresh_path = model.initial_state(0, 0.0);
        assert_eq!(sample(1, &mut fresh_path, 0.8), low);
    }

    #[test]
    fn seasoned_initialization_and_first_transition_match_markov_probabilities() {
        let model = RegimeSwitchingPrepay::new(0.04, 0.80, 0.25, 0.10, 0.0, 0.0);
        let seasoning = 7;
        // Integrate the initial-state and transition uniforms on a Cartesian
        // midpoint grid; retain their independence rather than sharing a draw.
        let draws = 1_000;
        let mut total_smm = 0.0;
        for i in 0..draws {
            let initial = model.initial_state(seasoning, (f64::from(i) + 0.5) / f64::from(draws));
            for j in 0..draws {
                let mut state = initial.clone();
                total_smm += model.sample_smm(
                    seasoning + 1,
                    &[0.0],
                    0.05,
                    1.0,
                    &mut state,
                    (f64::from(j) + 0.5) / f64::from(draws),
                );
            }
        }
        let mean_smm = total_smm / f64::from(draws * draws);
        assert!((mean_smm - model.expected_smm(seasoning + 1)).abs() < 1e-4);
    }
}
