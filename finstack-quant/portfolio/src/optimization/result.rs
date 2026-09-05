//! Problem, decision-space, constraint, and solver types for portfolio optimization.
//!
use super::problem::PortfolioOptimizationProblem;
use super::types::{SLACK_TOL, WEIGHT_TOL};
use crate::error::{Error, Result};
use crate::portfolio::Portfolio;
use crate::position::Position;
use crate::types::{Entity, PositionId};
use finstack_quant_core::config::ResultsMeta;
use finstack_quant_core::wire::SchemaVersion;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize, Serializer};

/// Status of an optimization run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "json-schema",
    schemars(with = "PortfolioOptimizationResultWire")
)]
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
    /// Reconstruction depends on the weighting scheme and is always written
    /// in `Position::quantity` units (lots, shares, face multiplier, or
    /// percentage points):
    /// - `ValueWeight`: convert the target PV share through `pv_per_unit`
    ///   (base PV per 1.0 of `scale_factor()`) and map scale back to quantity
    /// - `NotionalWeight`: convert the target notional share through
    ///   `|instrument.notional().amount()|` and map scale back to quantity
    /// - `UnitScaling`: treat the optimized weight as a quantity multiplier for
    ///   existing positions, or as the direct target quantity for new candidates
    pub implied_quantities: IndexMap<PositionId, f64>,

    /// Objective value at the solution.
    pub objective_value: f64,

    /// Evaluated portfolio‑level metrics at the solution (for key `MetricExpr`s).
    pub metric_values: IndexMap<String, f64>,

    /// Optimization status.
    pub status: OptimizationStatus,

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
    ///
    /// When `!self.status.is_feasible()` there is no solution and the list is
    /// empty — an empty list from a failed solve does **not** mean "already
    /// optimal, nothing to trade"; check [`OptimizationStatus::is_feasible`]
    /// before consuming it.
    #[must_use]
    pub fn to_trade_list(&self) -> Vec<TradeSpec> {
        let mut trades: Vec<TradeSpec> = self
            .optimal_weights
            .iter()
            .filter_map(|(pos_id, &target_weight)| {
                let current_weight = self.current_weights.get(pos_id).copied().unwrap_or(0.0);
                let delta_weight = target_weight - current_weight;

                // Skip positions with negligible change.
                // See `optimization::types::WEIGHT_TOL` for the rationale
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
    ///
    /// When `!self.status.is_feasible()` there is no solution and the list is
    /// empty; check [`OptimizationStatus::is_feasible`] before consuming it.
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
    ///
    /// When `!self.status.is_feasible()` no slacks exist and the list is
    /// empty — meaningless rather than "no constraint binds"; check
    /// [`OptimizationStatus::is_feasible`] before consuming it.
    #[must_use]
    pub fn binding_constraints(&self) -> Vec<(&str, f64)> {
        // See `optimization::types::SLACK_TOL` for the rationale behind
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
    /// Gross turnover implied by the optimized weights, or `NaN` when
    /// `!self.status.is_feasible()` — there is no solution, so "no turnover"
    /// would be indistinguishable from "already optimal". This mirrors the
    /// `objective_value = NaN` convention on failed solves.
    #[must_use]
    pub fn turnover(&self) -> f64 {
        if !self.status.is_feasible() {
            return f64::NAN;
        }
        self.weight_deltas.values().map(|d| d.abs()).sum()
    }
}

/// Serialize the canonical JSON wire format.
///
/// Emits all stored fields plus derived fields (`status_label`, `is_feasible`,
/// `turnover`, `trades`, `binding_constraints`, `label`). The `problem` field
/// is intentionally omitted — it contains `Arc<dyn Instrument>` values that do
/// not round-trip through serde.
/// Canonical serializable optimization-result contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortfolioOptimizationResultWire {
    /// Required numeric v1 marker.
    pub schema_version: SchemaVersion,
    /// Solver outcome.
    pub status: OptimizationStatus,
    /// Stable snake-case status label.
    pub status_label: String,
    /// Whether the solution can be consumed.
    pub is_feasible: bool,
    /// Objective value at the solution.
    pub objective_value: f64,
    /// Gross turnover.
    pub turnover: f64,
    /// Optimal weights keyed by position.
    pub optimal_weights: IndexMap<PositionId, f64>,
    /// Current weights keyed by position.
    pub current_weights: IndexMap<PositionId, f64>,
    /// Weight changes keyed by position.
    pub weight_deltas: IndexMap<PositionId, f64>,
    /// Implied target quantities keyed by position.
    pub implied_quantities: IndexMap<PositionId, f64>,
    /// Evaluated metrics.
    pub metric_values: IndexMap<String, f64>,
    /// Executable trade list.
    pub trades: Vec<TradeSpec>,
    /// Constraint slack values.
    pub constraint_slacks: IndexMap<String, f64>,
    /// Approximately binding constraint names.
    pub binding_constraints: Vec<String>,
    /// Optional user-supplied problem label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl From<&PortfolioOptimizationResult> for PortfolioOptimizationResultWire {
    fn from(result: &PortfolioOptimizationResult) -> Self {
        let status_label = match result.status {
            OptimizationStatus::Optimal => "optimal",
            OptimizationStatus::FeasibleButSuboptimal => "feasible_but_suboptimal",
            OptimizationStatus::Infeasible { .. } => "infeasible",
            OptimizationStatus::Unbounded => "unbounded",
            OptimizationStatus::Error { .. } => "error",
        };
        Self {
            schema_version: SchemaVersion::CURRENT,
            status: result.status.clone(),
            status_label: status_label.to_string(),
            is_feasible: result.status.is_feasible(),
            objective_value: result.objective_value,
            turnover: result.turnover(),
            optimal_weights: result.optimal_weights.clone(),
            current_weights: result.current_weights.clone(),
            weight_deltas: result.weight_deltas.clone(),
            implied_quantities: result.implied_quantities.clone(),
            metric_values: result.metric_values.clone(),
            trades: result.to_trade_list(),
            constraint_slacks: result.constraint_slacks.clone(),
            binding_constraints: result
                .binding_constraints()
                .into_iter()
                .map(|(name, _)| name.to_string())
                .collect(),
            label: result.problem.label.clone(),
        }
    }
}

impl Serialize for PortfolioOptimizationResult {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        PortfolioOptimizationResultWire::from(self).serialize(serializer)
    }
}
