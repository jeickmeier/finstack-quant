//! Almgren-Chriss (2001) market impact model.
//!
//! Decomposes market impact into permanent and temporary components
//! and solves for the optimal execution trajectory that minimizes
//! expected cost + risk aversion * cost variance.
//!
//! # References
//!
//! - Almgren, R. & Chriss, N. (2001). "Optimal Execution of Portfolio
//!   Transactions." *Journal of Risk*, 3(2).
//!   `docs/REFERENCES.md#almgrenChriss2001OptimalExecution`

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

use super::impact::{ExecutionTrajectory, ImpactEstimate, MarketImpactModel, TradeParams};
use super::types::LiquidityProfile;

/// Almgren-Chriss (2001) market impact model.
///
/// Decomposes market impact into permanent and temporary components:
///
/// **Permanent impact**: proportional to total volume traded.
/// ```text
/// g(v) = gamma * v
/// ```
///
/// **Temporary impact**: order-flow pressure following a power law.
/// ```text
/// h(v) = eta * sign(v) * |v|^delta
/// ```
///
/// where `v` is the trading rate, `gamma` is the permanent impact coefficient,
/// `eta` is the temporary impact coefficient, and `delta` is the power-law
/// exponent (typically 0.5-0.6).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlmgrenChrissModel {
    /// Permanent impact coefficient (gamma).
    gamma: f64,

    /// Temporary impact coefficient (eta).
    eta: f64,

    /// Power-law exponent for temporary impact (delta).
    /// Typically 0.5-0.6 for equities.
    delta: f64,
}

impl AlmgrenChrissModel {
    /// Create a new Almgren-Chriss model.
    ///
    /// # Arguments
    ///
    /// * `gamma` - Permanent impact coefficient. Must be non-negative.
    /// * `eta` - Temporary impact coefficient. Must be positive.
    /// * `delta` - Power-law exponent. Must be in (0, 1].
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidInput` if parameters are out of valid range.
    pub fn new(gamma: f64, eta: f64, delta: f64) -> Result<Self> {
        if !gamma.is_finite() || gamma < 0.0 {
            return Err(Error::invalid_input(
                "gamma must be finite and non-negative",
            ));
        }
        if !eta.is_finite() || eta <= 0.0 {
            return Err(Error::invalid_input("eta must be finite and positive"));
        }
        if !delta.is_finite() || delta <= 0.0 || delta > 1.0 {
            return Err(Error::invalid_input("delta must be in (0, 1]"));
        }

        Ok(Self { gamma, eta, delta })
    }

    /// Estimate parameters from a `LiquidityProfile`.
    ///
    /// Uses the empirical calibration approach:
    /// - `gamma` estimated from a fraction of the spread per unit volume
    /// - `eta` estimated from spread and volume relationship
    /// - `delta` set to a default of 0.5
    ///
    /// # Arguments
    ///
    /// * `profile` - Liquidity profile for the instrument.
    /// * `daily_volatility` - Daily return volatility.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidInput` if daily_volatility is non-positive or
    /// if the profile has zero volume.
    pub fn from_profile(profile: &LiquidityProfile, daily_volatility: f64) -> Result<Self> {
        if !daily_volatility.is_finite() || daily_volatility <= 0.0 {
            return Err(Error::invalid_input(
                "daily_volatility must be finite and positive",
            ));
        }
        if profile.avg_daily_volume <= 0.0 {
            return Err(Error::invalid_input(
                "avg_daily_volume must be positive for calibration",
            ));
        }

        // Empirical calibration in price space:
        // gamma ~ spread / (2 * ADV) -- permanent price impact per share.
        let gamma = profile.spread() / (2.0 * profile.avg_daily_volume);

        // eta ~ sigma_price / sqrt(ADV), with sigma_price = daily_vol * mid.
        let eta = daily_volatility * profile.mid / profile.avg_daily_volume.sqrt();

        let delta = 0.5;

        Self::new(gamma, eta, delta)
    }
}

impl MarketImpactModel for AlmgrenChrissModel {
    fn estimate_cost(&self, params: &TradeParams) -> Result<ImpactEstimate> {
        if !params.quantity.is_finite() {
            return Err(Error::invalid_input("quantity must be finite"));
        }
        if !params.horizon_days.is_finite() || params.horizon_days <= 0.0 {
            return Err(Error::invalid_input(
                "horizon_days must be finite and positive",
            ));
        }
        if !params.daily_volatility.is_finite() || params.daily_volatility <= 0.0 {
            return Err(Error::invalid_input(
                "daily_volatility must be finite and positive",
            ));
        }

        let q = params.quantity;
        let t = params.horizon_days;

        // Trading rate assuming uniform execution
        let rate = q / t;

        // Permanent impact: gamma * Q (price shift)
        let perm_impact = self.gamma * q.abs();
        let perm_cost = perm_impact * q.abs() * 0.5;

        // Temporary impact: eta * |rate|^delta per period
        let temp_impact_per_period = self.eta * rate.abs().powf(self.delta);
        let temp_cost = temp_impact_per_period * q.abs();

        let total_cost = perm_cost + temp_cost;

        // Notional value (use the arrival / decision reference price when
        // the caller supplies one; `profile.mid` is the default).
        let reference_price = params.effective_reference_price();
        let notional = q.abs() * reference_price;
        let cost_bps = if notional > 0.0 {
            total_cost / notional * 10_000.0
        } else {
            0.0
        };

        // Uniform execution holds inventory linearly from Q to 0, so
        // integrated inventory variance contributes T / 3.
        let execution_risk = params.daily_volatility * (t / 3.0).sqrt() * q.abs() * reference_price;

        Ok(ImpactEstimate {
            permanent_impact: perm_cost,
            temporary_impact: temp_cost,
            total_cost,
            cost_bps,
            execution_risk,
        })
    }

    fn optimal_trajectory(
        &self,
        params: &TradeParams,
        num_buckets: usize,
    ) -> Result<ExecutionTrajectory> {
        if num_buckets == 0 {
            return Err(Error::invalid_input("num_buckets must be > 0"));
        }
        if !params.quantity.is_finite() {
            return Err(Error::invalid_input("quantity must be finite"));
        }
        if !params.horizon_days.is_finite() || params.horizon_days <= 0.0 {
            return Err(Error::invalid_input(
                "horizon_days must be finite and positive",
            ));
        }
        if !params.daily_volatility.is_finite() || params.daily_volatility <= 0.0 {
            return Err(Error::invalid_input(
                "daily_volatility must be finite and positive",
            ));
        }

        let q = params.quantity;
        let t = params.horizon_days;
        let dt = t / num_buckets as f64;
        let risk_aversion = params.risk_aversion.unwrap_or(1e-6);
        if !risk_aversion.is_finite() || risk_aversion < 0.0 {
            return Err(Error::invalid_input(
                "risk_aversion must be finite and non-negative",
            ));
        }
        let sigma = params.daily_volatility * params.effective_reference_price();

        // The closed-form `sinh` schedule used below is the optimal solution
        // for *linear* temporary impact (delta = 1) only. For general `delta`
        // the optimal trajectory requires solving a non-linear ODE; using the
        // linear schedule as an approximation can produce sub-optimal plans
        // (often by 10-20% for `delta` near 0.5). Reject the call so callers
        // either change the model's `delta` or drop down to `estimate_cost`,
        // which uses the true `delta` for cost reporting.
        if (self.delta - 1.0).abs() > 1e-12 {
            return Err(Error::invalid_input(format!(
                "Almgren-Chriss optimal_trajectory is only defined for delta = 1 \
                 (linear temporary impact); model has delta = {delta}. Either set \
                 the model's delta to 1.0 to opt into the closed-form schedule, or \
                 use estimate_cost for cost reporting under the true delta.",
                delta = self.delta
            )));
        }

        // kappa = sqrt(risk_aversion * sigma^2 / eta)
        // Optimal remaining: x_j = Q * sinh(kappa * (T - t_j)) / sinh(kappa * T)
        let kappa_sq = if self.eta > 0.0 {
            risk_aversion * sigma * sigma / self.eta
        } else {
            0.0
        };
        let kappa = kappa_sq.sqrt();

        let kappa_t = kappa * t;

        let mut remaining = Vec::with_capacity(num_buckets + 1);
        let mut time_points = Vec::with_capacity(num_buckets + 1);
        let mut quantities = Vec::with_capacity(num_buckets);

        // Generate the optimal remaining position at each time point
        remaining.push(q);
        time_points.push(0.0);

        if kappa_t.abs() < 1e-12 {
            // When kappa ~ 0, uniform execution is optimal
            let per_bucket = q / num_buckets as f64;
            for j in 1..=num_buckets {
                let t_j = j as f64 * dt;
                time_points.push(t_j);
                let rem = q - per_bucket * j as f64;
                remaining.push(rem);
                quantities.push(per_bucket);
            }
        } else {
            for j in 1..=num_buckets {
                let t_j = j as f64 * dt;
                time_points.push(t_j);
                let rem = if j == num_buckets {
                    0.0 // Ensure exact completion
                } else {
                    q * stable_sinh_ratio(kappa * (t - t_j), kappa_t)
                };
                remaining.push(rem);
            }
            for j in 0..num_buckets {
                quantities.push(remaining[j] - remaining[j + 1]);
            }
        }

        // Compute expected cost and cost variance of the trajectory
        let mut expected_cost = 0.0;
        let mut cost_variance = 0.0;

        for j in 0..num_buckets {
            let trade_rate = quantities[j] / dt;

            // Permanent impact cost contribution. Each trade `n_j` is filled
            // against the price already depressed by prior fills plus half of
            // its own impact, i.e. the average inventory across the bucket
            // `(x_j + x_{j+1}) / 2`. This midpoint rule telescopes to the
            // trajectory-independent `0.5 * gamma * Q^2`, matching the
            // closed-form permanent cost used by `estimate_cost`. Using the
            // pre-trade inventory `x_j` alone would over-count by
            // `0.5 * gamma * sum(n_j^2)` (an O(1/num_buckets) bias).
            let avg_inventory = (remaining[j].abs() + remaining[j + 1].abs()) / 2.0;
            expected_cost += self.gamma * quantities[j].abs() * avg_inventory;

            // Temporary impact cost contribution
            expected_cost += self.eta * trade_rate.abs().powf(self.delta) * quantities[j].abs();

            // Variance contribution: sigma^2 * remaining^2 * dt
            cost_variance += sigma * sigma * remaining[j + 1] * remaining[j + 1] * dt;
        }

        Ok(ExecutionTrajectory {
            quantities,
            remaining,
            expected_cost,
            cost_variance,
            time_points,
        })
    }

    fn model_name(&self) -> &str {
        "Almgren-Chriss"
    }
}

fn stable_sinh_ratio(numerator: f64, denominator: f64) -> f64 {
    if numerator == 0.0 {
        return 0.0;
    }
    if denominator.abs() <= 50.0 {
        return numerator.sinh() / denominator.sinh();
    }

    let num_term = if numerator.abs() > 50.0 {
        1.0
    } else {
        1.0 - (-2.0 * numerator).exp()
    };
    let den_term = if denominator.abs() > 50.0 {
        1.0
    } else {
        1.0 - (-2.0 * denominator).exp()
    };
    (numerator - denominator).exp() * num_term / den_term
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::liquidity::types::LiquidityProfile;

    fn test_profile() -> std::result::Result<LiquidityProfile, Box<dyn std::error::Error>> {
        Ok(LiquidityProfile::new(
            "TEST",
            100.0,
            99.5,
            100.5,
            1_000_000.0,
            500.0,
            0.001,
        )?)
    }

    fn test_params(quantity: f64) -> std::result::Result<TradeParams, Box<dyn std::error::Error>> {
        Ok(TradeParams {
            quantity,
            horizon_days: 5.0,
            daily_volatility: 0.02,
            profile: test_profile()?,
            risk_aversion: None,
            reference_price: None,
        })
    }

    #[test]
    fn construction_valid() {
        assert!(AlmgrenChrissModel::new(0.001, 0.01, 0.5).is_ok());
    }

    #[test]
    fn construction_rejects_negative_gamma() {
        assert!(AlmgrenChrissModel::new(-0.001, 0.01, 0.5).is_err());
    }

    #[test]
    fn construction_rejects_zero_eta() {
        assert!(AlmgrenChrissModel::new(0.001, 0.0, 0.5).is_err());
    }

    #[test]
    fn construction_rejects_delta_out_of_range() {
        assert!(AlmgrenChrissModel::new(0.001, 0.01, 0.0).is_err());
        assert!(AlmgrenChrissModel::new(0.001, 0.01, 1.5).is_err());
    }

    #[test]
    fn from_profile_calibrates() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let profile = test_profile()?;
        let model = AlmgrenChrissModel::from_profile(&profile, 0.02)?;
        assert!((model.gamma - 5e-7).abs() < 1e-12);
        assert!((model.eta - 0.002).abs() < 1e-12);
        Ok(())
    }

    #[test]
    fn estimate_cost_nonnegative() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let model = AlmgrenChrissModel::new(0.001, 0.01, 0.5)?;
        let params = test_params(10_000.0)?;
        let est = model.estimate_cost(&params)?;

        assert!(est.total_cost >= 0.0);
        assert!(est.permanent_impact >= 0.0);
        assert!(est.temporary_impact >= 0.0);
        assert!(est.cost_bps >= 0.0);
        assert!(est.execution_risk >= 0.0);
        let expected_execution_risk = params.daily_volatility * params.horizon_days.sqrt()
            / 3.0_f64.sqrt()
            * params.quantity.abs()
            * params.effective_reference_price();
        assert!((est.execution_risk - expected_execution_risk).abs() < 1e-9);
        Ok(())
    }

    #[test]
    fn estimate_cost_sell_side() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let model = AlmgrenChrissModel::new(0.001, 0.01, 0.5)?;
        let params = test_params(-10_000.0)?;
        let est = model.estimate_cost(&params)?;
        assert!(
            est.total_cost >= 0.0,
            "sell-side cost should be non-negative"
        );
        Ok(())
    }

    #[test]
    fn estimate_cost_scales_with_quantity() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let model = AlmgrenChrissModel::new(0.001, 0.01, 0.5)?;
        let small = model.estimate_cost(&test_params(1_000.0)?)?;
        let large = model.estimate_cost(&test_params(100_000.0)?)?;
        assert!(
            large.total_cost > small.total_cost,
            "larger trade should cost more"
        );
        Ok(())
    }

    #[test]
    fn trajectory_sums_to_quantity() -> std::result::Result<(), Box<dyn std::error::Error>> {
        // Closed-form trajectory is only defined for delta = 1 (linear impact);
        // see `optimal_trajectory` rejection contract.
        let model = AlmgrenChrissModel::new(0.001, 0.01, 1.0)?;
        let params = test_params(50_000.0)?;
        let traj = model.optimal_trajectory(&params, 10)?;

        assert_eq!(traj.quantities.len(), 10);
        assert_eq!(traj.remaining.len(), 11);
        assert_eq!(traj.time_points.len(), 11);

        let total_traded: f64 = traj.quantities.iter().sum();
        assert!(
            (total_traded - 50_000.0).abs() < 1e-6,
            "trajectory should trade entire quantity, got {total_traded}"
        );

        // Remaining should start at Q and end at 0
        assert!((traj.remaining[0] - 50_000.0).abs() < 1e-10);
        assert!(traj.remaining[10].abs() < 1e-10);
        Ok(())
    }

    #[test]
    fn trajectory_zero_risk_aversion_is_uniform(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let model = AlmgrenChrissModel::new(0.001, 0.01, 1.0)?;
        let mut params = test_params(10_000.0)?;
        params.risk_aversion = Some(0.0);
        let traj = model.optimal_trajectory(&params, 5)?;

        // With zero risk aversion, kappa = 0 => uniform execution
        let expected_per_bucket = 10_000.0 / 5.0;
        for q in &traj.quantities {
            assert!(
                (q - expected_per_bucket).abs() < 1e-6,
                "expected uniform {expected_per_bucket}, got {q}"
            );
        }
        Ok(())
    }

    /// For the zero-risk-aversion (uniform) trajectory with linear impact
    /// (`delta = 1`), the expected cost has a closed form:
    ///   permanent = 0.5 * gamma * Q^2   (trajectory-independent)
    ///   temporary = eta * Q^2 / T       (uniform rate Q/T over the horizon)
    /// This pins the midpoint discretization of the permanent component.
    #[test]
    fn trajectory_cost_matches_closed_form_uniform(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let gamma = 0.001;
        let eta = 0.01;
        let model = AlmgrenChrissModel::new(gamma, eta, 1.0)?;
        let mut params = test_params(10_000.0)?;
        params.risk_aversion = Some(0.0);
        let q = params.quantity;
        let t = params.horizon_days;

        let traj = model.optimal_trajectory(&params, 5)?;

        let expected_permanent = 0.5 * gamma * q * q;
        let expected_temporary = eta * q * q / t;
        let expected_total = expected_permanent + expected_temporary;
        assert!(
            (traj.expected_cost - expected_total).abs() < 1e-6,
            "expected closed-form cost {expected_total}, got {}",
            traj.expected_cost
        );
        Ok(())
    }

    #[test]
    fn trajectory_rejects_zero_buckets() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let model = AlmgrenChrissModel::new(0.001, 0.01, 1.0)?;
        let params = test_params(10_000.0)?;
        assert!(model.optimal_trajectory(&params, 0).is_err());
        Ok(())
    }

    #[test]
    fn trajectory_rejects_negative_risk_aversion(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let model = AlmgrenChrissModel::new(0.001, 0.01, 1.0)?;
        let mut params = test_params(10_000.0)?;
        params.risk_aversion = Some(-1.0);
        assert!(model.optimal_trajectory(&params, 5).is_err());
        Ok(())
    }

    #[test]
    fn trajectory_large_kappa_t_stays_finite() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let model = AlmgrenChrissModel::new(0.001, 1e-6, 1.0)?;
        let mut params = test_params(10_000.0)?;
        params.risk_aversion = Some(1e8);
        let traj = model.optimal_trajectory(&params, 5)?;
        assert!(traj.quantities.iter().all(|v| v.is_finite()));
        assert!(traj.remaining.iter().all(|v| v.is_finite()));
        assert!(traj.expected_cost.is_finite());
        assert!(traj.cost_variance.is_finite());
        Ok(())
    }

    #[test]
    fn trajectory_rejects_non_linear_delta() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        // delta != 1 => closed-form sinh schedule is not optimal; the function
        // must error out instead of returning a sub-optimal plan silently.
        let model = AlmgrenChrissModel::new(0.001, 0.01, 0.5)?;
        let params = test_params(10_000.0)?;
        assert!(model.optimal_trajectory(&params, 5).is_err());
        Ok(())
    }

    /// `optimal_trajectory` rejects `delta != 1` using a strict tolerance
    /// (`abs(delta - 1) > 1e-12`). The constructor caps `delta` at 1.0
    /// inclusive, so the realistic boundary case is `delta = 1.0 - eps` for
    /// a small `eps` — it must still be accepted by the trajectory.
    #[test]
    fn trajectory_accepts_delta_near_unity_within_tolerance(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        // 1.0 - 1e-13 is within 1e-12 of 1.0 and must be accepted.
        let model = AlmgrenChrissModel::new(0.001, 0.01, 1.0_f64 - 1e-13)?;
        let params = test_params(10_000.0)?;
        let traj = model.optimal_trajectory(&params, 4)?;
        let total: f64 = traj.quantities.iter().sum();
        assert!(
            (total - 10_000.0).abs() < 1e-6,
            "near-unity delta should still trade the full quantity, got {total}"
        );
        Ok(())
    }

    #[test]
    fn serde_round_trip_impact_estimate() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let est = ImpactEstimate {
            permanent_impact: 100.0,
            temporary_impact: 200.0,
            total_cost: 300.0,
            cost_bps: 15.0,
            execution_risk: 500.0,
        };
        let json = serde_json::to_string(&est)?;
        let est2: ImpactEstimate = serde_json::from_str(&json)?;
        assert_eq!(est, est2);
        Ok(())
    }
}
