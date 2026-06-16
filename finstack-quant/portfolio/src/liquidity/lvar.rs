//! Liquidity-adjusted Value at Risk (LVaR) calculator.
//!
//! Composes with existing VaR numbers (from `factor_model/` or external sources)
//! to produce liquidity-adjusted figures following Bangia et al. (1999).
//!
//! # Sign convention
//!
//! VaR and LVaR are expressed on the P&L axis: **losses are negative**. A valid
//! input `var` is therefore non-positive and all LVaR variants are likewise
//! non-positive. A "more conservative" LVaR is the more negative number.
//!
//! # References
//!
//! - Bangia, A., Diebold, F., Schuermann, T., Stroughair, J. (1999).
//!   "Modeling Liquidity Risk with Implications for Traditional Market
//!   Risk Measurement and Management." *Risk*, 12(1).
//!   `docs/REFERENCES.md#bangia1999LiquidityRisk`

use crate::error::{Error, Result};
use crate::types::PositionId;
use finstack_quant_core::math::special_functions::standard_normal_inv_cdf;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::types::{days_to_liquidate, LiquidityConfig, LiquidityProfile};

fn validate_lvar_confidence(confidence: f64) -> Result<()> {
    if !confidence.is_finite() || confidence <= 0.5 || confidence >= 1.0 {
        return Err(Error::invalid_input(
            "confidence must be finite and in the open interval (0.5, 1)",
        ));
    }
    Ok(())
}

/// Result of a liquidity-adjusted VaR calculation for a single position.
///
/// All VaR / LVaR fields follow the loss sign convention: **losses are negative**.
/// `exogenous_cost` and `endogenous_cost` are reported as non-negative magnitudes
/// of the liquidity add-on for easier cost accounting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LvarResult {
    /// Position identifier.
    pub position_id: PositionId,

    /// Standard VaR (input, not computed here). Non-positive number representing a loss.
    pub var: f64,

    /// Exogenous liquidity cost magnitude: half-spread times position value.
    /// This is the constant-cost add-on assuming spread is independent
    /// of position size. Always non-negative.
    pub exogenous_cost: f64,

    /// Endogenous liquidity cost magnitude: spread widening due to position size
    /// relative to ADV. Always non-negative.
    pub endogenous_cost: f64,

    /// Bangia et al. (1999) LVaR combining VaR with spread mean and
    /// spread volatility. Reported as a non-positive loss number:
    ///
    /// ```text
    /// LVaR = VaR - (0.5 * mean_relative_spread + z_alpha * 0.5 * rel_spread_vol) * PV
    /// ```
    pub lvar_bangia: f64,

    /// Time-to-liquidation adjusted LVaR (non-positive loss number):
    ///
    /// ```text
    /// LVaR_horizon = VaR * sqrt(liquidation_days / holding_period)
    /// ```
    pub lvar_horizon: f64,

    /// Days required to liquidate at the configured participation rate.
    pub days_to_liquidate: f64,

    /// Composite LVaR: `min(lvar_bangia, lvar_horizon)` — the most-negative
    /// and therefore most conservative of the two adjustments.
    pub lvar_composite: f64,
}

/// Aggregated LVaR results across a portfolio.
///
/// VaR / LVaR totals follow the loss sign convention (sums of negative numbers).
/// `total_exogenous_cost` and `total_endogenous_cost` are non-negative magnitudes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioLvarReport {
    /// Per-position LVaR results.
    pub position_results: Vec<LvarResult>,

    /// Sum of standard VaR across positions (non-positive).
    pub total_var: f64,

    /// Sum of composite LVaR across positions (most-conservative aggregate, non-positive).
    pub total_lvar_composite: f64,

    /// Total exogenous liquidity cost magnitude (non-negative).
    pub total_exogenous_cost: f64,

    /// Total endogenous liquidity cost magnitude (non-negative).
    pub total_endogenous_cost: f64,

    /// Liquidity cost as percentage of total VaR magnitude.
    ///
    /// ```text
    /// (|total_lvar_composite| - |total_var|) / |total_var| * 100
    /// ```
    pub liquidity_cost_pct: f64,

    /// Positions for which no `LiquidityProfile` was provided.
    pub missing_profiles: Vec<PositionId>,

    /// Positions whose LVaR computation failed at runtime, paired with the
    /// error message. Populated by [`LvarCalculator::compute_portfolio`] so
    /// callers can detect silent drops instead of relying on count checks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_positions: Vec<(PositionId, String)>,
}

/// Scalar Bangia LVaR outputs for an isolated position where relative spread
/// statistics are already known. Used by bindings that don't carry a full
/// `LiquidityProfile`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LvarBangiaScalar {
    /// Input VaR (non-positive loss number), echoed back for convenience.
    pub var: f64,
    /// Non-negative magnitude of the Bangia spread-cost add-on.
    pub spread_cost: f64,
    /// Bangia-adjusted LVaR (non-positive loss number, `lvar <= var <= 0`).
    pub lvar: f64,
    /// Ratio `lvar / var`. `NaN` when `var == 0`.
    pub lvar_ratio: f64,
}

/// Bangia, Diebold, Schuermann & Stroughair (1999) LVaR from scalar spread
/// statistics, for an isolated position where the caller already has
/// `spread_mean` and `spread_vol` in relative (fraction-of-mid) terms.
///
/// Formula:
/// ```text
/// spread_cost = (0.5 * spread_mean + 0.5 * z_alpha * spread_vol) * |position_value|
/// lvar        = var - spread_cost
/// ```
///
/// # Errors
///
/// Returns `Error::InvalidInput` if `var` is positive or non-finite, if
/// `spread_mean` or `spread_vol` are negative or non-finite, if `confidence`
/// is outside `(0, 1)`, or if `position_value` is non-finite.
///
/// # References
///
/// - Bangia et al. (1999). `docs/REFERENCES.md#bangia1999LiquidityRisk`
pub fn lvar_bangia_scalar(
    var: f64,
    spread_mean: f64,
    spread_vol: f64,
    confidence: f64,
    position_value: f64,
) -> Result<LvarBangiaScalar> {
    if !var.is_finite() || var > 0.0 {
        return Err(Error::invalid_input(format!(
            "var must be non-positive and finite (loss sign convention), got {var}"
        )));
    }
    if !spread_mean.is_finite() || spread_mean < 0.0 {
        return Err(Error::invalid_input(
            "spread_mean must be non-negative and finite",
        ));
    }
    if !spread_vol.is_finite() || spread_vol < 0.0 {
        return Err(Error::invalid_input(
            "spread_vol must be non-negative and finite",
        ));
    }
    validate_lvar_confidence(confidence)?;
    if !position_value.is_finite() {
        return Err(Error::invalid_input("position_value must be finite"));
    }

    let pv = position_value.abs();
    let z_alpha = standard_normal_inv_cdf(confidence);
    let spread_cost = (0.5 * spread_mean + 0.5 * z_alpha * spread_vol) * pv;
    let lvar = var - spread_cost;
    let lvar_ratio = if var.abs() > 1e-15 {
        lvar / var
    } else {
        f64::NAN
    };

    Ok(LvarBangiaScalar {
        var,
        spread_cost,
        lvar,
        lvar_ratio,
    })
}

/// Calculator for liquidity-adjusted VaR.
///
/// This calculator does not compute VaR itself -- it takes VaR as an input
/// and adjusts it for liquidity costs and time-to-liquidation effects.
/// VaR should come from the existing `factor_model::ParametricDecomposer`,
/// `factor_model::SimulationDecomposer`, or an external source.
///
/// # References
///
/// - Bangia et al. (1999). `docs/REFERENCES.md#bangia1999LiquidityRisk`
pub struct LvarCalculator {
    config: LiquidityConfig,
    z_alpha: f64,
}

impl LvarCalculator {
    /// Create a new calculator with the given configuration.
    pub fn new(config: LiquidityConfig) -> Self {
        let z_alpha = standard_normal_inv_cdf(config.confidence_level);
        Self { config, z_alpha }
    }

    /// Compute LVaR for a single position.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Identifier for the position.
    /// * `var` - Standard VaR for the position following the loss sign convention
    ///   (non-positive number; `-10_000.0` means a $10,000 loss at the chosen
    ///   confidence level). `0.0` is accepted (zero-risk cash position).
    /// * `position_value` - Market value of the position (sign ignored; only the
    ///   magnitude is used).
    /// * `profile` - Liquidity profile for the instrument.
    ///
    /// # Returns
    ///
    /// [`LvarResult`] with all LVaR variants computed. VaR / LVaR fields are
    /// non-positive; exogenous / endogenous costs are non-negative magnitudes.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidInput` if `var` is positive or non-finite,
    /// or if `position_value` is non-finite.
    pub fn compute(
        &self,
        position_id: &PositionId,
        var: f64,
        position_value: f64,
        profile: &LiquidityProfile,
    ) -> Result<LvarResult> {
        if !var.is_finite() || var > 0.0 {
            return Err(Error::invalid_input(format!(
                "VaR must be non-positive and finite (loss sign convention), got {var}"
            )));
        }
        if !position_value.is_finite() {
            return Err(Error::invalid_input(format!(
                "position_value must be finite, got {position_value}"
            )));
        }
        validate_lvar_confidence(self.config.confidence_level)?;

        let pv = position_value.abs();

        // Exogenous cost magnitude: half-spread * position value.
        let exogenous_cost = profile.half_spread() / profile.mid * pv;

        // Endogenous cost magnitude: spread widening due to position size relative to ADV.
        let position_shares = if profile.mid > 0.0 {
            pv / profile.mid
        } else {
            0.0
        };
        // Endogenous cost: square-root size-dependent spread widening. The
        // coefficient is a calibration parameter (default 0.1, but no
        // canonical published source — see `LiquidityConfig::endogenous_spread_coef`).
        // Set to 0.0 in config to disable.
        let endogenous_cost = if profile.avg_daily_volume > 0.0
            && self.config.endogenous_spread_coef > 0.0
        {
            let ratio = position_shares / profile.avg_daily_volume;
            let impact = self.config.endogenous_spread_coef * profile.spread() * ratio.powf(0.5);
            impact / profile.mid * pv
        } else {
            0.0
        };

        // Bangia et al. LVaR (loss sign convention):
        //   LVaR = VaR - (0.5 * mean_relative_spread + z_alpha * 0.5 * rel_spread_vol) * PV
        // The add-on magnitude is subtracted because losses are negative.
        let half_relative_spread = 0.5 * profile.relative_spread();
        let spread_vol_term = 0.5 * self.z_alpha * profile.relative_spread_volatility();
        let lvar_bangia = var - (half_relative_spread + spread_vol_term) * pv;

        // Days to liquidate.
        let dtl = days_to_liquidate(
            position_shares,
            profile.avg_daily_volume,
            self.config.participation_rate,
        );

        // Horizon-adjusted LVaR: scales the (negative) VaR by sqrt(dtl / H).
        let horizon_scale = if self.config.holding_period > 0.0 && dtl.is_finite() {
            (dtl / self.config.holding_period).sqrt()
        } else if dtl.is_infinite() {
            dtl
        } else {
            1.0
        };
        let lvar_horizon = if var == 0.0 && dtl.is_infinite() {
            0.0
        } else {
            var * horizon_scale
        };

        // Composite: most conservative = most negative under loss sign convention.
        let lvar_composite = if lvar_bangia.is_finite() && lvar_horizon.is_finite() {
            lvar_bangia.min(lvar_horizon)
        } else if lvar_horizon.is_infinite() {
            // -inf if dtl is infinite, which is the most conservative value.
            f64::NEG_INFINITY
        } else {
            lvar_bangia
        };

        Ok(LvarResult {
            position_id: position_id.clone(),
            var,
            exogenous_cost,
            endogenous_cost,
            lvar_bangia,
            lvar_horizon,
            days_to_liquidate: dtl,
            lvar_composite,
        })
    }

    /// Compute LVaR for all positions in a portfolio.
    ///
    /// Runs serially; per-position work is dominated by a small closed-form
    /// computation, and the aggregated totals are accumulated into a running
    /// sum, which is trivially serial.
    ///
    /// # Arguments
    ///
    /// * `position_vars` - Slice of (position_id, var, position_value) tuples.
    /// * `profiles` - Map from instrument_id to liquidity profile.
    ///
    /// # Returns
    ///
    /// [`PortfolioLvarReport`] with per-position results and aggregates.
    /// Positions without a matching profile are recorded in `missing_profiles`.
    pub fn compute_portfolio(
        &self,
        position_vars: &[(PositionId, String, f64, f64)],
        profiles: &HashMap<String, LiquidityProfile>,
    ) -> PortfolioLvarReport {
        let mut position_results = Vec::new();
        let mut missing_profiles = Vec::new();
        let mut failed_positions: Vec<(PositionId, String)> = Vec::new();

        for (pos_id, instrument_id, var, pv) in position_vars {
            match profiles.get(instrument_id.as_str()) {
                Some(profile) => match self.compute(pos_id, *var, *pv, profile) {
                    Ok(result) => position_results.push(result),
                    Err(err) => {
                        let message = err.to_string();
                        tracing::warn!(
                            position_id = %pos_id,
                            instrument_id = %instrument_id,
                            error = %message,
                            "LVaR computation failed for position"
                        );
                        failed_positions.push((pos_id.clone(), message));
                    }
                },
                None => {
                    missing_profiles.push(pos_id.clone());
                }
            }
        }

        let total_var: f64 = position_results.iter().map(|r| r.var).sum();
        let total_lvar_composite: f64 = position_results.iter().map(|r| r.lvar_composite).sum();
        let total_exogenous_cost: f64 = position_results.iter().map(|r| r.exogenous_cost).sum();
        let total_endogenous_cost: f64 = position_results.iter().map(|r| r.endogenous_cost).sum();

        // Under the loss sign convention totals are non-positive; report the
        // liquidity add-on percentage using magnitudes for interpretability.
        let liquidity_cost_pct = if total_var.abs() > 0.0 {
            (total_lvar_composite.abs() - total_var.abs()) / total_var.abs() * 100.0
        } else {
            0.0
        };

        PortfolioLvarReport {
            position_results,
            total_var,
            total_lvar_composite,
            total_exogenous_cost,
            total_endogenous_cost,
            liquidity_cost_pct,
            missing_profiles,
            failed_positions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidity::types::{LiquidityConfig, SpreadVolatilityKind};

    fn test_profile() -> std::result::Result<LiquidityProfile, Box<dyn std::error::Error>> {
        Ok(LiquidityProfile::new(
            "TEST",
            100.0,
            99.5,
            100.5,
            1_000_000.0,
            500.0,
            0.002,
        )?)
    }

    fn default_calculator() -> LvarCalculator {
        LvarCalculator::new(LiquidityConfig::default())
    }

    #[test]
    fn lvar_basic_computation() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let calc = default_calculator();
        let profile = test_profile()?;
        let pos_id = PositionId::new("POS1");

        // VaR is a loss = negative under the loss sign convention.
        let r = calc.compute(&pos_id, -10_000.0, 1_000_000.0, &profile)?;

        assert_eq!(r.var, -10_000.0);
        assert!(
            r.exogenous_cost > 0.0,
            "exogenous cost should be a positive magnitude"
        );
        assert!(
            r.lvar_bangia < r.var,
            "LVaR Bangia should be more negative than VaR"
        );
        assert!(
            r.lvar_composite <= r.var,
            "composite LVaR should be <= VaR (more conservative loss)"
        );
        Ok(())
    }

    #[test]
    fn lvar_zero_spread_zero_exogenous() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let calc = default_calculator();
        // Zero-spread instrument (bid == ask == mid).
        let profile = LiquidityProfile {
            instrument_id: "ZERO_SPREAD".into(),
            mid: 100.0,
            bid: 100.0,
            ask: 100.0,
            avg_daily_volume: 1_000_000.0,
            avg_trade_size: 500.0,
            spread_volatility: 0.0,
            spread_volatility_kind: SpreadVolatilityKind::default(),
            observation_days: 20,
        };
        let pos_id = PositionId::new("POS1");

        let r = calc.compute(&pos_id, -10_000.0, 1_000_000.0, &profile)?;
        assert!(
            (r.exogenous_cost).abs() < 1e-10,
            "zero spread => zero exogenous cost"
        );
        Ok(())
    }

    #[test]
    fn lvar_horizon_equals_one_when_dtl_equals_holding(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        // If days_to_liquidate == holding_period, horizon adjustment = sqrt(1) = 1
        // position_shares = PV / mid. ADV * participation_rate = daily_capacity.
        // dtl = position_shares / daily_capacity = 1.0
        // => position_shares = ADV * participation_rate * 1.0 = 100,000
        // => PV = 100,000 * mid = 10,000,000
        let config = LiquidityConfig {
            participation_rate: 0.10,
            holding_period: 1.0,
            ..LiquidityConfig::default()
        };
        let calc = LvarCalculator::new(config);
        let profile = LiquidityProfile::new("TEST", 100.0, 99.5, 100.5, 1_000_000.0, 500.0, 0.0)?;

        // position_shares = 10_000_000 / 100 = 100_000
        // daily_capacity = 0.10 * 1_000_000 = 100_000
        // dtl = 1.0
        let pos_id = PositionId::new("POS1");
        let r = calc.compute(&pos_id, -10_000.0, 10_000_000.0, &profile)?;
        assert!(
            (r.days_to_liquidate - 1.0).abs() < 1e-10,
            "expected dtl=1.0, got {}",
            r.days_to_liquidate
        );
        assert!(
            (r.lvar_horizon - (-10_000.0)).abs() < 1e-6,
            "horizon LVaR should equal VaR when dtl=holding_period"
        );
        Ok(())
    }

    #[test]
    fn lvar_rejects_positive_var() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let calc = default_calculator();
        let profile = test_profile()?;
        let pos_id = PositionId::new("POS1");
        // A positive VaR violates the loss sign convention.
        assert!(calc.compute(&pos_id, 1.0, 1_000_000.0, &profile).is_err());
        Ok(())
    }

    #[test]
    fn lvar_rejects_nan_var() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let calc = default_calculator();
        let profile = test_profile()?;
        let pos_id = PositionId::new("POS1");
        assert!(calc
            .compute(&pos_id, f64::NAN, 1_000_000.0, &profile)
            .is_err());
        Ok(())
    }

    #[test]
    fn lvar_rejects_non_finite_position_value(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let calc = default_calculator();
        let profile = test_profile()?;
        let pos_id = PositionId::new("POS1");
        assert!(calc
            .compute(&pos_id, -1000.0, f64::INFINITY, &profile)
            .is_err());
        Ok(())
    }

    #[test]
    fn mo12_scalar_rejects_sub_median_confidence() {
        let err = lvar_bangia_scalar(-1_000.0, 0.01, 0.001, 0.49, 100_000.0)
            .expect_err("MO-12: confidence below 0.5 must be rejected");

        assert!(
            err.to_string().contains("confidence"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn mo12_calculator_rejects_unit_confidence(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let calc = LvarCalculator::new(LiquidityConfig {
            confidence_level: 1.0,
            ..LiquidityConfig::default()
        });
        let profile = test_profile()?;
        let pos_id = PositionId::new("POS1");

        let err = calc
            .compute(&pos_id, -1_000.0, 100_000.0, &profile)
            .expect_err("MO-12: confidence >= 1 must be rejected");
        assert!(
            err.to_string().contains("confidence"),
            "unexpected error: {err}"
        );
        Ok(())
    }

    #[test]
    fn mo12_zero_var_with_infinite_dtl_has_finite_horizon_lvar(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let calc = default_calculator();
        let profile = LiquidityProfile::new("TEST", 100.0, 100.0, 100.0, 0.0, 0.0, 0.0)?;
        let pos_id = PositionId::new("POS1");

        let result = calc.compute(&pos_id, 0.0, 100_000.0, &profile)?;

        assert_eq!(result.days_to_liquidate, f64::INFINITY);
        assert_eq!(result.lvar_horizon, 0.0);
        assert!(result.lvar_composite.is_finite());
        Ok(())
    }

    #[test]
    fn portfolio_lvar_missing_profiles() {
        let calc = default_calculator();
        let profiles: HashMap<String, LiquidityProfile> = HashMap::new();
        let position_vars = vec![(
            PositionId::new("POS1"),
            "UNKNOWN".to_string(),
            -10_000.0,
            1_000_000.0,
        )];

        let report = calc.compute_portfolio(&position_vars, &profiles);
        assert!(report.position_results.is_empty());
        assert_eq!(report.missing_profiles.len(), 1);
        assert!(report.failed_positions.is_empty());
    }

    #[test]
    fn portfolio_lvar_surfaces_computation_errors(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let calc = default_calculator();
        let profile = test_profile()?;
        let mut profiles = HashMap::new();
        profiles.insert("TEST".to_string(), profile);

        // Positive VaR violates the loss sign convention and must surface
        // as a failed position rather than being silently dropped.
        let position_vars = vec![(
            PositionId::new("POS1"),
            "TEST".to_string(),
            1_000.0,
            500_000.0,
        )];
        let report = calc.compute_portfolio(&position_vars, &profiles);
        assert!(report.position_results.is_empty());
        assert_eq!(report.failed_positions.len(), 1);
        assert_eq!(report.failed_positions[0].0, PositionId::new("POS1"));
        Ok(())
    }

    #[test]
    fn portfolio_lvar_aggregation() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let calc = default_calculator();
        let profile = test_profile()?;
        let mut profiles = HashMap::new();
        profiles.insert("TEST".to_string(), profile);

        let position_vars = vec![
            (
                PositionId::new("POS1"),
                "TEST".to_string(),
                -5_000.0,
                500_000.0,
            ),
            (
                PositionId::new("POS2"),
                "TEST".to_string(),
                -8_000.0,
                800_000.0,
            ),
        ];

        let report = calc.compute_portfolio(&position_vars, &profiles);
        assert_eq!(report.position_results.len(), 2);
        assert!((report.total_var - (-13_000.0)).abs() < 1e-10);
        // LVaR is more negative (more conservative loss) than VaR.
        assert!(report.total_lvar_composite <= report.total_var);
        assert!(report.liquidity_cost_pct >= 0.0);
        assert!(report.failed_positions.is_empty());
        Ok(())
    }

    #[test]
    fn serde_round_trip_lvar_result() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let r = LvarResult {
            position_id: PositionId::new("POS1"),
            var: -10_000.0,
            exogenous_cost: 5_000.0,
            endogenous_cost: 100.0,
            lvar_bangia: -17_000.0,
            lvar_horizon: -12_000.0,
            days_to_liquidate: 2.5,
            lvar_composite: -17_000.0,
        };

        let json = serde_json::to_string(&r)?;
        let r2: LvarResult = serde_json::from_str(&json)?;
        assert_eq!(r, r2);
        Ok(())
    }
}
