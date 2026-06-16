//! Market impact model trait and shared types.
//!
//! Defines the [`MarketImpactModel`] trait and the data structures for
//! trade parameters, impact estimates, and execution trajectories.

use crate::error::Result;
use serde::{Deserialize, Serialize};

use super::types::LiquidityProfile;

/// Input parameters for a market impact calculation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradeParams {
    /// Total quantity to execute (positive = buy, negative = sell).
    pub quantity: f64,

    /// Execution horizon in trading days.
    pub horizon_days: f64,

    /// Daily return volatility of the instrument.
    pub daily_volatility: f64,

    /// Liquidity profile for the instrument.
    pub profile: LiquidityProfile,

    /// Risk aversion parameter (overrides config if set).
    pub risk_aversion: Option<f64>,

    /// Reference price used to convert the return-space volatility
    /// `daily_volatility` into a currency-space risk term (execution risk,
    /// variance, etc.).
    ///
    /// `None` means fall back to `profile.mid`, which matches the
    /// historical default. Set explicitly when the arrival price or
    /// decision-time price differs materially from the profile mid (e.g.
    /// when the profile was calibrated from a snapshot stale relative to
    /// the order).
    #[serde(default)]
    pub reference_price: Option<f64>,
}

impl TradeParams {
    /// Return the reference price used to convert return-space volatility
    /// into currency units, falling back to `profile.mid` when unset.
    pub fn effective_reference_price(&self) -> f64 {
        self.reference_price.unwrap_or(self.profile.mid)
    }
}

/// Estimated market impact from a trade.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImpactEstimate {
    /// Permanent price impact (information leakage, irreversible).
    pub permanent_impact: f64,

    /// Temporary price impact (order-flow pressure, mean-reverts).
    pub temporary_impact: f64,

    /// Total expected execution cost (permanent + temporary).
    pub total_cost: f64,

    /// Cost as basis points of notional value.
    pub cost_bps: f64,

    /// Execution risk (standard deviation of cost).
    pub execution_risk: f64,
}

/// Optimal execution schedule for a trade.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionTrajectory {
    /// Quantity to trade in each time bucket.
    pub quantities: Vec<f64>,

    /// Remaining position after each bucket.
    pub remaining: Vec<f64>,

    /// Expected cost of the optimal trajectory.
    pub expected_cost: f64,

    /// Variance of the cost under the optimal trajectory.
    pub cost_variance: f64,

    /// Time points (in trading days) for each bucket boundary.
    pub time_points: Vec<f64>,
}

/// Trait for market impact models that estimate the cost of executing a trade.
///
/// Implementations take the trade parameters and return the estimated
/// price impact (in currency units per share/contract). The trait is
/// object-safe to allow heterogeneous model selection per instrument.
pub trait MarketImpactModel: Send + Sync {
    /// Estimate the total execution cost of a trade.
    ///
    /// # Arguments
    ///
    /// * `params` - Trade parameters including size, urgency, and market data.
    ///
    /// # Returns
    ///
    /// Estimated total cost in the instrument's native currency.
    fn estimate_cost(&self, params: &TradeParams) -> Result<ImpactEstimate>;

    /// Compute the optimal execution trajectory.
    ///
    /// Returns the number of shares to trade in each time bucket to
    /// minimize expected cost + risk aversion * variance.
    ///
    /// # Arguments
    ///
    /// * `params` - Trade parameters.
    /// * `num_buckets` - Number of time intervals to divide execution into.
    ///
    /// # Returns
    ///
    /// Optimal trajectory as quantities per bucket.
    fn optimal_trajectory(
        &self,
        params: &TradeParams,
        num_buckets: usize,
    ) -> Result<ExecutionTrajectory>;

    /// Human-readable model name for reporting.
    fn model_name(&self) -> &str;
}
