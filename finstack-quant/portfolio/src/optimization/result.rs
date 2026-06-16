use super::problem::PortfolioOptimizationProblem;
use super::tolerances::{SLACK_TOL, WEIGHT_TOL};
use crate::error::{Error, Result};
use crate::portfolio::Portfolio;
use crate::position::Position;
use crate::types::{Entity, PositionId};
use finstack_quant_core::config::ResultsMeta;
use indexmap::IndexMap;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

/// Status of an optimization run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizationStatus {
    /// Found optimal solution.
    Optimal,

    /// Found feasible solution but solver stopped early.
    FeasibleButSuboptimal,

    /// No feasible solution exists.
    Infeasible {
        /// Constraints that appear to conflict (if determinable).
        conflicting_constraints: Vec<String>,
    },

    /// Objective is unbounded.
    Unbounded,

    /// Solver error.
    Error {
        /// Error message describing what went wrong.
        message: String,
    },
}

impl OptimizationStatus {
    /// Check if the status represents a usable solution.
    ///
    /// # Returns
    ///
    /// `true` when the result contains a feasible portfolio that downstream
    /// helpers may safely consume.
    #[must_use]
    pub fn is_feasible(&self) -> bool {
        matches!(self, Self::Optimal | Self::FeasibleButSuboptimal)
    }
}

/// Direction of a trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeDirection {
    /// Buy more of the instrument (increase exposure).
    Buy,
    /// Sell the instrument (decrease exposure).
    Sell,
    /// No change in exposure.
    Hold,
}

/// Whether a trade is for an existing position or a new candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradeType {
    /// Adjusting an existing portfolio position.
    Existing,
    /// Adding a new position from the candidate universe.
    NewPosition,
    /// Closing out an existing position entirely.
    CloseOut,
}

/// Trade specification for a single position.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeSpec {
    /// Position identifier in the optimized portfolio.
    pub position_id: PositionId,
    /// Underlying instrument identifier (or candidate id).
    pub instrument_id: String,
    /// Trade type (existing, new position, close‑out).
    pub trade_type: TradeType,
    /// Pre‑trade quantity.
    pub current_quantity: f64,
    /// Post‑trade quantity.
    pub target_quantity: f64,
    /// Quantity change (`target - current`).
    pub delta_quantity: f64,
    /// Buy / Sell / Hold classification.
    pub direction: TradeDirection,
    /// Pre‑trade weight.
    pub current_weight: f64,
    /// Post‑trade weight.
    pub target_weight: f64,
}

/// Solution of an optimization problem.
#[derive(Clone, Debug)]
pub struct PortfolioOptimizationResult {
    /// Echo of the original problem for traceability.
    pub problem: PortfolioOptimizationProblem,

    /// Pre‑trade weights (current portfolio state).
    pub current_weights: IndexMap<PositionId, f64>,

    /// Optimal weights per position (for positions in the universe).
    /// Non‑universe positions implicitly keep weight = current_weight.
    pub optimal_weights: IndexMap<PositionId, f64>,

    /// Weight changes (`optimal - current`).
    pub weight_deltas: IndexMap<PositionId, f64>,

    /// Implied target quantities for each position (units / face / notional).
    ///
    /// Reconstruction depends on the weighting scheme:
    /// - `ValueWeight`: convert target PV share back to quantity using `pv_per_unit`
    /// - `NotionalWeight`: apply the target notional share directly
    /// - `UnitScaling`: treat the optimized weight as a quantity multiplier for
    ///   existing positions, or as the direct target quantity for new candidates
    pub implied_quantities: IndexMap<PositionId, f64>,

    /// Objective value at the solution.
    pub objective_value: f64,

    /// Evaluated portfolio‑level metrics at the solution (for key `MetricExpr`s).
    pub metric_values: IndexMap<String, f64>,

    /// Optimization status.
    pub status: OptimizationStatus,

    /// Shadow prices / dual values for constraints (how much objective improves
    /// per unit relaxation). Key is constraint label or auto‑generated identifier.
    ///
    /// **Note:** The current LP backend (`good_lp` / `minilp`) does not expose
    /// dual information, so this map is always empty. It is retained for forward
    /// compatibility with solvers that do support duals.
    pub dual_values: IndexMap<String, f64>,

    /// Constraint slack values (positive = slack, zero ≈ binding).
    pub constraint_slacks: IndexMap<String, f64>,

    /// Calculation metadata (numeric mode, rounding context, timing).
    pub meta: ResultsMeta,
}

impl PortfolioOptimizationResult {
    /// Generate a new portfolio with quantities adjusted to target weights.
    ///
    /// This is a convenience helper that:
    /// - Clones the original portfolio
    /// - Updates position quantities according to `implied_quantities`
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInput`] if the optimization did not
    /// find a feasible solution.
    pub fn to_rebalanced_portfolio(&self) -> Result<Portfolio> {
        if !self.status.is_feasible() {
            return Err(Error::invalid_input(
                "cannot generate rebalanced portfolio from infeasible solution",
            ));
        }

        let mut portfolio = self.problem.portfolio.clone();

        for position in &mut portfolio.positions {
            if let Some(qty) = self.implied_quantities.get(&position.position_id) {
                position.quantity = *qty;
            }
        }
        for candidate in &self.problem.trade_universe.candidates {
            let Some(target_qty) = self.implied_quantities.get(&candidate.id).copied() else {
                continue;
            };
            let target_weight = self
                .optimal_weights
                .get(&candidate.id)
                .copied()
                .unwrap_or(0.0);
            if target_weight.abs() < WEIGHT_TOL || target_qty.abs() < WEIGHT_TOL {
                continue;
            }

            portfolio
                .entities
                .entry(candidate.entity_id.clone())
                .or_insert_with(|| Entity::new(candidate.entity_id.clone()));
            let mut position = Position::new(
                candidate.id.clone(),
                candidate.entity_id.clone(),
                candidate.instrument.id(),
                std::sync::Arc::clone(&candidate.instrument),
                target_qty,
                candidate.unit,
            )?;
            position.attributes = candidate.attributes.clone();
            portfolio.add_position(position)?;
        }

        portfolio.validate()?;
        Ok(portfolio)
    }

    /// Generate trade list (delta from current to target).
    ///
    /// Returns trades sorted by absolute quantity delta (largest first).
    /// Includes both adjustments to existing positions and new positions from
    /// candidates.
    ///
    /// # Returns
    ///
    /// Sorted trade list covering existing positions and candidate additions
    /// whose weight changes are materially non-zero.
    #[must_use]
    pub fn to_trade_list(&self) -> Vec<TradeSpec> {
        let mut trades: Vec<TradeSpec> = self
            .optimal_weights
            .iter()
            .filter_map(|(pos_id, &target_weight)| {
                let current_weight = self.current_weights.get(pos_id).copied().unwrap_or(0.0);
                let delta_weight = target_weight - current_weight;

                // Skip positions with negligible change.
                // See `optimization::tolerances::WEIGHT_TOL` for the rationale
                // behind the chosen scale.
                if !delta_weight.is_finite() || delta_weight.abs() < WEIGHT_TOL {
                    return None;
                }

                let existing_position = self.problem.portfolio.get_position(pos_id.as_str());
                let is_candidate = existing_position.is_none();

                let current_qty = existing_position.map(|p| p.quantity).unwrap_or(0.0);
                let target_qty = self.implied_quantities.get(pos_id).copied().unwrap_or(0.0);

                let trade_type = if is_candidate && target_weight.abs() > WEIGHT_TOL {
                    TradeType::NewPosition
                } else if !is_candidate && target_weight.abs() < WEIGHT_TOL {
                    TradeType::CloseOut
                } else {
                    TradeType::Existing
                };

                // Get instrument_id (from existing position or candidate)
                let instrument_id = existing_position
                    .map(|p| p.instrument_id.clone())
                    .or_else(|| {
                        self.problem
                            .trade_universe
                            .candidates
                            .iter()
                            .find(|c| c.id == *pos_id)
                            .map(|c| c.instrument.id().to_string())
                    })
                    .unwrap_or_default();

                let direction = if delta_weight > 0.0 {
                    TradeDirection::Buy
                } else if delta_weight < 0.0 {
                    TradeDirection::Sell
                } else {
                    TradeDirection::Hold
                };

                Some(TradeSpec {
                    position_id: pos_id.clone(),
                    instrument_id,
                    trade_type,
                    current_quantity: current_qty,
                    target_quantity: target_qty,
                    delta_quantity: target_qty - current_qty,
                    direction,
                    current_weight,
                    target_weight,
                })
            })
            .collect();

        trades.sort_by(|a, b| {
            b.delta_quantity
                .abs()
                .partial_cmp(&a.delta_quantity.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        trades
    }

    /// Get only trades for new positions (from candidates).
    ///
    /// # Returns
    ///
    /// Trade specifications whose `trade_type` is [`TradeType::NewPosition`].
    #[must_use]
    pub fn new_position_trades(&self) -> Vec<TradeSpec> {
        self.to_trade_list()
            .into_iter()
            .filter(|t| t.trade_type == TradeType::NewPosition)
            .collect()
    }

    /// Get binding constraints at the optimal solution (slack ≈ 0).
    ///
    /// # Returns
    ///
    /// Constraint names and slack values for approximately binding constraints.
    #[must_use]
    pub fn binding_constraints(&self) -> Vec<(&str, f64)> {
        // See `optimization::tolerances::SLACK_TOL` for the rationale behind
        // the chosen scale.
        self.constraint_slacks
            .iter()
            .filter(|(_, &slack)| slack.abs() < SLACK_TOL)
            .map(|(name, &slack)| (name.as_str(), slack))
            .collect()
    }

    /// Calculate gross turnover (sum of absolute weight changes).
    ///
    /// # Returns
    ///
    /// Gross turnover implied by the optimized weights.
    #[must_use]
    pub fn turnover(&self) -> f64 {
        self.weight_deltas.values().map(|d| d.abs()).sum()
    }
}

/// Serialize the canonical JSON wire format.
///
/// Emits all stored fields plus derived fields (`status_label`, `is_feasible`,
/// `turnover`, `trades`, `binding_constraints`, `label`). The `problem` field
/// is intentionally omitted — it contains `Arc<dyn Instrument>` values that do
/// not round-trip through serde.
/// Wire-format schema version for serialized `PortfolioOptimizationResult`.
pub const PORTFOLIO_OPTIMIZATION_RESULT_SCHEMA_VERSION: u32 = 1;

impl Serialize for PortfolioOptimizationResult {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let binding_constraints: Vec<String> = self
            .binding_constraints()
            .into_iter()
            .map(|(name, _)| name.to_string())
            .collect();
        let mut st = serializer.serialize_struct("PortfolioOptimizationResult", 16)?;
        st.serialize_field(
            "schema_version",
            &PORTFOLIO_OPTIMIZATION_RESULT_SCHEMA_VERSION,
        )?;
        st.serialize_field("status", &self.status)?;
        st.serialize_field("status_label", &format!("{:?}", self.status))?;
        st.serialize_field("is_feasible", &self.status.is_feasible())?;
        st.serialize_field("objective_value", &self.objective_value)?;
        st.serialize_field("turnover", &self.turnover())?;
        st.serialize_field("optimal_weights", &self.optimal_weights)?;
        st.serialize_field("current_weights", &self.current_weights)?;
        st.serialize_field("weight_deltas", &self.weight_deltas)?;
        st.serialize_field("implied_quantities", &self.implied_quantities)?;
        st.serialize_field("metric_values", &self.metric_values)?;
        st.serialize_field("trades", &self.to_trade_list())?;
        st.serialize_field("dual_values", &self.dual_values)?;
        st.serialize_field("constraint_slacks", &self.constraint_slacks)?;
        st.serialize_field("binding_constraints", &binding_constraints)?;
        st.serialize_field("label", &self.problem.label)?;
        st.end()
    }
}
