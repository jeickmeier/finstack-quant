//! Correlation and volatility sensitivities for stochastic structured credit.
//!
//! Computes Greeks-like sensitivities for stochastic parameters:
//! - Correlation01: Sensitivity to asset correlation
//! - RecoveryCorrelation01: Sensitivity to recovery-default correlation
//! - PrepaymentVol01: Sensitivity to prepayment volatility

#![allow(dead_code)]

use super::calculator::StochasticMetricsCalculator;
use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::ScenarioTreeConfig;

/// Sensitivity configuration.
#[derive(Debug, Clone)]
pub(crate) struct SensitivityConfig {
    /// Bump size for correlation sensitivities (default: 0.01 = 1%)
    pub correlation_bump: f64,
    /// Bump size for volatility sensitivities (default: 0.01 = 1%)
    pub volatility_bump: f64,
    /// Notional for scaling
    pub notional: f64,
}

impl Default for SensitivityConfig {
    fn default() -> Self {
        Self {
            correlation_bump: 0.01,
            volatility_bump: 0.01,
            notional: 1_000_000.0,
        }
    }
}

impl SensitivityConfig {
    /// Create a new sensitivity configuration.
    pub(crate) fn new(notional: f64) -> Self {
        Self {
            notional,
            ..Default::default()
        }
    }

    /// Set correlation bump size.
    pub(crate) fn with_correlation_bump(mut self, bump: f64) -> Self {
        self.correlation_bump = bump.clamp(0.0001, 0.10);
        self
    }

    /// Set volatility bump size.
    pub(crate) fn with_volatility_bump(mut self, bump: f64) -> Self {
        self.volatility_bump = bump.clamp(0.0001, 0.10);
        self
    }
}

/// Correlation and volatility sensitivities.
#[derive(Debug, Clone)]
pub(crate) struct CorrelationSensitivities {
    // === Correlation sensitivities (per 1% bump) ===
    /// Sensitivity of expected loss to 1% asset correlation bump
    pub correlation_01_el: f64,

    /// Sensitivity of unexpected loss to 1% asset correlation bump.
    ///
    /// `None` when the underlying metrics carry no tail risk — UL is a
    /// dispersion metric and a recombining scenario tree cannot supply it.
    pub correlation_01_ul: Option<f64>,

    /// Sensitivity of 99% ES to 1% asset correlation bump.
    ///
    /// `None` when the underlying metrics carry no tail risk (recombining tree).
    pub correlation_01_es99: Option<f64>,

    // === Recovery correlation sensitivities ===
    /// Sensitivity to 1% recovery-default correlation bump
    pub recovery_correlation_01: f64,

    // === Prepayment sensitivities ===
    /// Sensitivity to 1% prepayment factor loading bump
    pub prepay_factor_01: f64,

    /// Sensitivity to 1% prepayment volatility bump
    pub prepay_vol_01: f64,

    // === Cross sensitivities ===
    /// Cross gamma: d²(EL)/d(corr)²
    pub correlation_gamma: f64,

    // === Diagnostics ===
    /// Base expected loss (for context)
    pub base_el: f64,

    /// Base unexpected loss (for context). `None` from recombining-tree metrics.
    pub base_ul: Option<f64>,

    /// Base 99% ES (for context). `None` from recombining-tree metrics.
    pub base_es99: Option<f64>,
}

impl CorrelationSensitivities {
    /// Create sensitivities with all values set to zero (tail-risk-derived
    /// fields `None`).
    pub(crate) fn zero() -> Self {
        Self {
            correlation_01_el: 0.0,
            correlation_01_ul: None,
            correlation_01_es99: None,
            recovery_correlation_01: 0.0,
            prepay_factor_01: 0.0,
            prepay_vol_01: 0.0,
            correlation_gamma: 0.0,
            base_el: 0.0,
            base_ul: None,
            base_es99: None,
        }
    }

    /// Compute sensitivities from a tree configuration.
    ///
    /// This performs multiple tree builds with bumped parameters
    /// to compute finite-difference sensitivities.
    pub(crate) fn compute(
        config: &ScenarioTreeConfig,
        sens_config: &SensitivityConfig,
    ) -> Result<Self, String> {
        let calc = StochasticMetricsCalculator::new(sens_config.notional);

        // Base case
        let base_metrics = calc.compute_from_config(config)?;

        // Correlation bump
        let bump = sens_config.correlation_bump;
        let corr_up_config = bump_asset_correlation(config, bump)?;
        let corr_up_metrics = calc.compute_from_config(&corr_up_config)?;

        // Correlation down (for gamma)
        let corr_down_config = bump_asset_correlation(config, -bump)?;
        let corr_down_metrics = calc.compute_from_config(&corr_down_config)?;

        // Recovery correlation bump
        let recovery_corr_up_config = bump_recovery_correlation(config, bump)?;
        let recovery_corr_up_metrics = calc.compute_from_config(&recovery_corr_up_config)?;

        // Prepayment volatility bump
        let prepay_vol_up_config = bump_prepay_volatility(config, sens_config.volatility_bump)?;
        let prepay_vol_up_metrics = calc.compute_from_config(&prepay_vol_up_config)?;

        // Compute sensitivities per 1% bump
        let scale = 0.01 / bump; // Normalize to per 1%

        let correlation_01_el =
            (corr_up_metrics.expected_loss - base_metrics.expected_loss) * scale;
        // UL / ES sensitivities need dispersion-preserving metrics; from a
        // recombining-tree config the base and bumped metrics carry no tail
        // risk, so these collapse to `None` (see `StochasticMetrics`).
        let correlation_01_ul = bumped_sensitivity(
            corr_up_metrics.unexpected_loss,
            base_metrics.unexpected_loss,
            scale,
        );
        let correlation_01_es99 = bumped_sensitivity(
            corr_up_metrics.expected_shortfall_99,
            base_metrics.expected_shortfall_99,
            scale,
        );

        // Gamma: second derivative
        let correlation_gamma = (corr_up_metrics.expected_loss - 2.0 * base_metrics.expected_loss
            + corr_down_metrics.expected_loss)
            / (bump * bump);

        // Recovery correlation sensitivity
        let recovery_correlation_01 =
            (recovery_corr_up_metrics.expected_loss - base_metrics.expected_loss) * scale;

        // Prepayment volatility sensitivity
        let vol_scale = 0.01 / sens_config.volatility_bump;
        let prepay_vol_01 = (prepay_vol_up_metrics.expected_prepayments
            - base_metrics.expected_prepayments)
            * vol_scale;

        Ok(Self {
            correlation_01_el,
            correlation_01_ul,
            correlation_01_es99,
            recovery_correlation_01,
            prepay_factor_01: 0.0, // Would require bumping factor loading
            prepay_vol_01,
            correlation_gamma,
            base_el: base_metrics.expected_loss,
            base_ul: base_metrics.unexpected_loss,
            base_es99: base_metrics.expected_shortfall_99,
        })
    }

    /// Quick estimate of correlation sensitivity without full re-pricing.
    ///
    /// Uses analytical approximation based on single-factor Gaussian copula:
    /// d(EL)/d(ρ) ≈ EL × (1 + k × √ρ) where k depends on portfolio granularity
    pub(crate) fn estimate_correlation_01(el: f64, ul: f64, current_corr: f64) -> f64 {
        // Simplified analytical estimate
        // For senior tranches: positive correlation sensitivity
        // For equity tranches: negative correlation sensitivity
        // This uses portfolio-level approximation

        if current_corr.abs() < 0.01 {
            return ul * 0.5; // Low correlation regime
        }

        // Approximate using UL as proxy for correlation exposure
        // Higher UL implies higher correlation sensitivity
        let implied_k = ul / el.max(1.0);
        el * implied_k * 0.01 / current_corr.sqrt().max(0.1)
    }
}

// === Helper functions for bumping configurations ===

/// Finite-difference sensitivity of an optional metric.
///
/// Returns `Some((bumped − base) · scale)` only when both the bumped and base
/// metric are available; `None` if either is missing — e.g. a tail-risk metric
/// from a recombining scenario tree.
fn bumped_sensitivity(bumped: Option<f64>, base: Option<f64>, scale: f64) -> Option<f64> {
    Some((bumped? - base?) * scale)
}

fn bump_asset_correlation(
    config: &ScenarioTreeConfig,
    bump: f64,
) -> Result<ScenarioTreeConfig, String> {
    let mut new_config = config.clone();
    new_config.correlation = config.correlation.bump_asset(bump);
    Ok(new_config)
}

fn bump_recovery_correlation(
    config: &ScenarioTreeConfig,
    bump: f64,
) -> Result<ScenarioTreeConfig, String> {
    use crate::instruments::common_impl::models::correlation::recovery::RecoverySpec;

    let mut new_config = config.clone();

    // Bump recovery correlation in recovery spec
    let new_recovery_spec = match &config.recovery_spec {
        RecoverySpec::MarketCorrelated {
            mean_recovery,
            recovery_volatility,
            factor_correlation,
        } => {
            let new_corr = (factor_correlation + bump).clamp(-0.99, 0.99);
            RecoverySpec::market_correlated(*mean_recovery, *recovery_volatility, new_corr)
                .map_err(|err| err.to_string())?
        }
        other => other.clone(),
    };

    new_config.recovery_spec = new_recovery_spec;
    Ok(new_config)
}

fn bump_prepay_volatility(
    config: &ScenarioTreeConfig,
    bump: f64,
) -> Result<ScenarioTreeConfig, String> {
    use crate::instruments::common_impl::models::correlation::factor_model::FactorSpec;

    let mut new_config = config.clone();

    // Bump prepayment volatility in factor spec
    let new_factor_spec = match &config.factor_spec {
        FactorSpec::SingleFactor {
            volatility,
            mean_reversion,
        } => {
            let new_vol = (volatility + bump).clamp(0.01, 2.0);
            FactorSpec::single_factor(new_vol, *mean_reversion)
        }
        FactorSpec::TwoFactor {
            prepay_vol,
            credit_vol,
            correlation,
        } => {
            let new_prepay_vol = (prepay_vol + bump).clamp(0.01, 2.0);
            FactorSpec::two_factor(new_prepay_vol, *credit_vol, *correlation)
        }
        other => other.clone(),
    };

    new_config.factor_spec = new_factor_spec;
    Ok(new_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::pricing::stochastic::tree::BranchingSpec;

    #[test]
    fn test_sensitivity_config_default() {
        let config = SensitivityConfig::default();
        assert!((config.correlation_bump - 0.01).abs() < 1e-10);
        assert!((config.notional - 1_000_000.0).abs() < 1e-10);
    }

    #[test]
    fn test_sensitivities_zero() {
        let sens = CorrelationSensitivities::zero();
        assert!((sens.correlation_01_el - 0.0).abs() < 1e-10);
        assert!((sens.recovery_correlation_01 - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_compute_sensitivities() {
        // Use small tree for speed
        let config = ScenarioTreeConfig::new(2, 0.167, BranchingSpec::fixed(2));
        let sens_config = SensitivityConfig::new(1_000_000.0);

        let result = CorrelationSensitivities::compute(&config, &sens_config);
        assert!(result.is_ok());

        let sens = result.expect("Should compute sensitivities");

        // Correlation01 should typically be positive for EL
        // (higher correlation → more extreme scenarios → higher average loss)
        // But this depends on the structure
        assert!(sens.correlation_01_el.is_finite());
        // UL sensitivity is a tail-risk metric; from a recombining-tree config
        // it is unavailable (`None`), not a silently-biased number.
        assert!(sens.correlation_01_ul.is_none());
        assert!(sens.correlation_01_es99.is_none());
        assert!(sens.base_el >= 0.0);
    }

    #[test]
    fn test_estimate_correlation_01() {
        let el = 50_000.0;
        let ul = 75_000.0;
        let corr = 0.20;

        let estimate = CorrelationSensitivities::estimate_correlation_01(el, ul, corr);

        // Should be finite and reasonable
        assert!(estimate.is_finite());
        assert!(estimate.abs() < el * 10.0); // Sanity check
    }

    #[test]
    fn test_bump_asset_correlation() {
        let config = ScenarioTreeConfig::new(2, 0.167, BranchingSpec::fixed(2));

        let bumped = bump_asset_correlation(&config, 0.05);
        assert!(bumped.is_ok());

        let bumped = bumped.expect("Should bump correlation");
        let orig_corr = config.correlation.asset_correlation();
        let new_corr = bumped.correlation.asset_correlation();

        assert!((new_corr - orig_corr - 0.05).abs() < 0.01);
    }
}
