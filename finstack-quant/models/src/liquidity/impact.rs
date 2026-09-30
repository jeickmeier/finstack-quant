//! Shared market impact types.
//!
//! Defines the data structures for trade parameters, impact estimates, and
//! execution trajectories consumed by [`super::AlmgrenChrissModel`] and
//! [`super::KyleLambdaModel`].

use serde::{Deserialize, Serialize};

use super::types::LiquidityProfile;

/// Input parameters for a market impact calculation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

    /// Risk aversion parameter for trajectory optimization; `None` falls
    /// back to the model's internal default (currently `1e-6` in the
    /// Almgren-Chriss trajectory solver).
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

/// Estimated market-impact execution *costs* from a trade.
///
/// All monetary fields are costs in currency units (impact integrated over
/// the executed quantity, e.g. `½·γ·Q²` for a linear permanent impact), not
/// per-share price displacements. The field names keep their historical
/// `*_impact` spelling for wire stability.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct ImpactEstimate {
    /// Permanent-impact component of the expected execution cost, in
    /// currency units (information leakage, irreversible). Despite the
    /// name, this is a cost, not a price displacement.
    pub permanent_impact: f64,

    /// Temporary-impact component of the expected execution cost, in
    /// currency units (order-flow pressure, mean-reverts). Despite the
    /// name, this is a cost, not a price displacement.
    pub temporary_impact: f64,

    /// Total expected execution cost (permanent + temporary), in currency
    /// units.
    pub total_cost: f64,

    /// Cost as basis points of notional value.
    pub cost_bp: f64,

    /// Execution risk (standard deviation of cost).
    pub execution_risk: f64,
}

/// Optimal execution schedule for a trade.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
