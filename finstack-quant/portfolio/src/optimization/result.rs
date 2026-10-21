//! Problem, decision-space, constraint, and solver types for portfolio optimization.
//!
use super::types::{SLACK_TOL, WEIGHT_TOL};
use super::universe::CandidatePosition;
use crate::error::{Error, Result};
use crate::position::Position;
use crate::types::{Entity, PositionId};
use crate::Portfolio;
use finstack_quant_core::config::ResultsMeta;
use finstack_quant_core::wire::SchemaVersion;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Status of an optimization run.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum OptimizationStatus {
    /// Found optimal solution.
    Optimal,

    /// No feasible solution exists. The LP backend does not expose an
    /// irreducible infeasible subset, so no conflicting constraints are named.
    Infeasible,

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
    /// `true` only for [`OptimizationStatus::Optimal`], the one status whose
    /// result carries a portfolio that downstream helpers may consume.
    #[must_use]
    pub fn is_feasible(&self) -> bool {
        matches!(self, Self::Optimal)
    }
}

/// Direction of a trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TradeDirection {
    /// Increase the signed instrument quantity, including covering a short.
    Buy,
    /// Decrease the signed instrument quantity, including increasing a short.
    Sell,
    /// No change in instrument quantity.
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
    /// Buy / Sell / Hold classification from the sign of `delta_quantity`.
    pub direction: TradeDirection,
    /// Pre‑trade weight.
    pub current_weight: f64,
    /// Post‑trade weight.
    pub target_weight: f64,
}

/// Solution of an optimization problem, and its canonical JSON wire contract.
///
/// Built by [`super::optimize`]: the trade list, turnover and binding
/// constraint names are computed from the solve when the result is built, so
/// the result needs no echo of the problem and round-trips through serde.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortfolioOptimizationResult {
    /// Configuration and numerical provenance from the solve.
    pub meta: ResultsMeta,
    /// Required numeric v1 marker.
    pub schema_version: SchemaVersion,
    /// Solver outcome; see [`OptimizationStatus::is_feasible`].
    pub status: OptimizationStatus,
    /// Objective value at the solution.
    ///
    /// `NaN` when the solve is not feasible; non-finite values travel as the
    /// string sentinels `"nan"`, `"inf"` and `"-inf"` so the result round-trips.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub objective_value: f64,
    /// Gross turnover (sum of absolute weight changes).
    ///
    /// `NaN` when the solve is not feasible — there is no solution, so "no
    /// turnover" would be indistinguishable from "already optimal"; encoded
    /// like `objective_value`.
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::NonFiniteF64Wire")
    )]
    pub turnover: f64,
    /// Optimal weights per position (for positions in the universe).
    /// Non‑universe positions implicitly keep weight = current_weight.
    pub optimal_weights: IndexMap<PositionId, f64>,
    /// Pre‑trade weights (current portfolio state).
    pub current_weights: IndexMap<PositionId, f64>,
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
    ///
    /// Held existing positions retain their exact current quantity under every
    /// scheme, including positions with zero or negligible present value.
    pub implied_quantities: IndexMap<PositionId, f64>,
    /// Executable trade list, largest absolute quantity change first.
    ///
    /// Covers existing positions and candidate additions whose weight change
    /// is materially non-zero. Empty for an infeasible solve, which is not the
    /// same as "nothing to trade"; check [`Self::is_feasible`] first.
    pub trades: Vec<TradeSpec>,
    /// Constraint slack values (positive = slack, zero ≈ binding); undefined
    /// averages use the `"nan"` sentinel.
    #[serde(with = "slack_values")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, finstack_quant_core::wire::NonFiniteF64Wire>")
    )]
    pub constraint_slacks: IndexMap<String, f64>,
    /// Names of the approximately binding constraints (slack ≈ 0), in slack
    /// order; empty for an infeasible solve.
    pub binding_constraints: Vec<String>,
    /// Optional user-supplied problem label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl PortfolioOptimizationResult {
    /// Whether the solution can be consumed (the status is
    /// [`OptimizationStatus::Optimal`]).
    ///
    /// # Returns
    ///
    /// `self.status.is_feasible()`; derived from `status`, never stored.
    #[must_use]
    pub fn is_feasible(&self) -> bool {
        self.status.is_feasible()
    }

    /// Executable trade list, largest absolute quantity change first.
    ///
    /// # Returns
    ///
    /// The stored `trades`; empty for an infeasible solve, which is not the
    /// same as "nothing to trade" — check [`Self::is_feasible`] first.
    #[must_use]
    pub fn to_trade_list(&self) -> &[TradeSpec] {
        &self.trades
    }

    /// Trades whose `trade_type` is [`TradeType::NewPosition`], in trade-list
    /// order.
    ///
    /// # Returns
    ///
    /// The matching trades; empty for an infeasible solve.
    #[must_use]
    pub fn new_position_trades(&self) -> Vec<TradeSpec> {
        self.trades
            .iter()
            .filter(|t| t.trade_type == TradeType::NewPosition)
            .cloned()
            .collect()
    }

    /// Approximately binding constraints and their slack values.
    ///
    /// Reads the stored `binding_constraints` names and looks each one up in
    /// `constraint_slacks`, so the accessor and the JSON wire report the same
    /// list.
    ///
    /// # Returns
    ///
    /// `(name, slack)` pairs in `binding_constraints` order; the slack is
    /// `NaN` for a name absent from `constraint_slacks` (only possible for a
    /// hand-edited result). Empty for an infeasible solve.
    #[must_use]
    pub fn binding_constraints(&self) -> Vec<(&str, f64)> {
        self.binding_constraints
            .iter()
            .map(|name| {
                let slack = self
                    .constraint_slacks
                    .get(name)
                    .copied()
                    .unwrap_or(f64::NAN);
                (name.as_str(), slack)
            })
            .collect()
    }
}

/// Build the trade list from current to target state, largest absolute
/// quantity change first.
///
/// Positions whose weight change is below `WEIGHT_TOL` (or non-finite) are
/// skipped. Instrument ids come from the held position, else from the
/// matching trade-universe candidate.
pub(crate) fn trade_list(
    portfolio: &Portfolio,
    candidates: &[CandidatePosition],
    current_weights: &IndexMap<PositionId, f64>,
    optimal_weights: &IndexMap<PositionId, f64>,
    implied_quantities: &IndexMap<PositionId, f64>,
) -> Vec<TradeSpec> {
    let mut trades: Vec<TradeSpec> = optimal_weights
        .iter()
        .filter_map(|(pos_id, &target_weight)| {
            let current_weight = current_weights.get(pos_id).copied().unwrap_or(0.0);
            let delta_weight = target_weight - current_weight;

            // Skip positions with negligible change.
            // See `optimization::types::WEIGHT_TOL` for the rationale
            // behind the chosen scale.
            if !delta_weight.is_finite() || delta_weight.abs() < WEIGHT_TOL {
                return None;
            }

            let existing_position = portfolio.get_position(pos_id.as_str());
            let is_candidate = existing_position.is_none();

            let current_qty = existing_position.map(|p| p.quantity).unwrap_or(0.0);
            let target_qty = implied_quantities.get(pos_id).copied().unwrap_or(0.0);

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
                    candidates
                        .iter()
                        .find(|c| c.id == *pos_id)
                        .map(|c| c.instrument.id().to_string())
                })
                .unwrap_or_default();

            let delta_quantity = target_qty - current_qty;
            let direction = if delta_quantity > 0.0 {
                TradeDirection::Buy
            } else if delta_quantity < 0.0 {
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
                delta_quantity,
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

/// Names of the constraints whose slack is approximately zero, in slack
/// order. See `optimization::types::SLACK_TOL` for the rationale behind the
/// chosen scale.
pub(crate) fn binding_constraint_names(slacks: &IndexMap<String, f64>) -> Vec<String> {
    slacks
        .iter()
        .filter(|(_, &slack)| slack.abs() < SLACK_TOL)
        .map(|(name, _)| name.clone())
        .collect()
}

/// Apply an optimization solution to a portfolio.
///
/// The single owner of the rebalancing rules, shared by the typed path and
/// [`super::rebalance_from_spec`]. Held positions take their
/// `implied_quantities`; a candidate is added (with its entity created when
/// missing) only when both its target weight and its target quantity are at
/// least `WEIGHT_TOL` in magnitude; the result is validated.
///
/// # Arguments
///
/// * `portfolio` - The portfolio that was optimized (the problem's
///   `portfolio`); consumed and returned rebalanced.
/// * `candidates` - The trade-universe candidates of the problem that
///   produced `result`; selected candidates become new positions.
/// * `result` - The solver outcome whose `status`, `implied_quantities` and
///   `optimal_weights` are applied.
///
/// # Returns
///
/// A validated portfolio with held quantities set to the implied quantities
/// and selected candidates added.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if the solution is not feasible, and
/// propagates position-construction and portfolio validation failures.
pub fn rebalance(
    mut portfolio: Portfolio,
    candidates: &[CandidatePosition],
    result: &PortfolioOptimizationResult,
) -> Result<Portfolio> {
    if !result.is_feasible() {
        return Err(Error::invalid_input(
            "cannot generate rebalanced portfolio from infeasible solution",
        ));
    }

    for position in &mut portfolio.positions {
        if let Some(qty) = result.implied_quantities.get(&position.position_id) {
            position.quantity = *qty;
        }
    }
    for candidate in candidates {
        let Some(target_qty) = result.implied_quantities.get(&candidate.id).copied() else {
            continue;
        };
        let target_weight = result
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

#[cfg(test)]
mod tests {
    use super::*;

    fn trade(position_id: &str, trade_type: TradeType) -> TradeSpec {
        TradeSpec {
            position_id: PositionId::new(position_id),
            instrument_id: position_id.to_string(),
            trade_type,
            current_quantity: 0.0,
            target_quantity: 1.0,
            delta_quantity: 1.0,
            direction: TradeDirection::Buy,
            current_weight: 0.0,
            target_weight: 0.5,
        }
    }

    fn result() -> PortfolioOptimizationResult {
        let mut constraint_slacks = IndexMap::new();
        constraint_slacks.insert("budget".to_string(), 0.0);
        constraint_slacks.insert("max_weight".to_string(), 0.25);
        PortfolioOptimizationResult {
            meta: ResultsMeta::default(),
            schema_version: SchemaVersion::CURRENT,
            status: OptimizationStatus::Optimal,
            objective_value: 1.0,
            turnover: 0.0,
            optimal_weights: IndexMap::new(),
            current_weights: IndexMap::new(),
            weight_deltas: IndexMap::new(),
            implied_quantities: IndexMap::new(),
            trades: vec![
                trade("OLD", TradeType::Existing),
                trade("NEW", TradeType::NewPosition),
            ],
            constraint_slacks,
            binding_constraints: vec!["budget".to_string()],
            label: None,
        }
    }

    #[test]
    fn new_position_trades_keeps_only_candidates() {
        let trades = result().new_position_trades();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].position_id.as_str(), "NEW");
    }

    #[test]
    fn binding_constraints_read_the_stored_names() {
        let mut solved = result();
        assert_eq!(solved.binding_constraints(), vec![("budget", 0.0)]);
        assert_eq!(
            binding_constraint_names(&solved.constraint_slacks),
            solved.binding_constraints
        );

        // The accessor reports the stored list, the same one the JSON wire
        // carries; a name without a slack reports NaN.
        solved.binding_constraints = vec!["max_weight".to_string(), "gone".to_string()];
        let binding = solved.binding_constraints();
        assert_eq!(binding[0], ("max_weight", 0.25));
        assert_eq!(binding[1].0, "gone");
        assert!(binding[1].1.is_nan());
    }

    #[test]
    fn is_feasible_derives_from_status() {
        let mut solved = result();
        assert!(solved.is_feasible());
        solved.status = OptimizationStatus::Infeasible;
        assert!(!solved.is_feasible());
        let json = serde_json::to_value(&solved).expect("serialize");
        assert_eq!(json["status"], serde_json::json!("infeasible"));
        assert!(json.get("is_feasible").is_none());
    }

    #[test]
    fn infeasible_result_round_trips_nan_objective_and_turnover() {
        // An infeasible solve reports NaN for both fields; serde_json used to
        // write them as `null` and then refuse to read its own output.
        let mut infeasible = result();
        infeasible.status = OptimizationStatus::Infeasible;
        infeasible.objective_value = f64::NAN;
        infeasible.turnover = f64::NAN;

        infeasible
            .constraint_slacks
            .insert("undefined_average".to_string(), f64::NAN);

        let json = serde_json::to_string(&infeasible).expect("serialize");
        assert!(json.contains(r#""objective_value":"nan""#), "{json}");
        assert!(json.contains(r#""turnover":"nan""#), "{json}");
        let decoded: PortfolioOptimizationResult =
            serde_json::from_str(&json).expect("deserialize");
        assert!(decoded.objective_value.is_nan());
        assert!(decoded.turnover.is_nan());
        assert!(decoded.constraint_slacks["undefined_average"].is_nan());
        assert_eq!(
            serde_json::to_value(&decoded.meta).expect("metadata"),
            serde_json::to_value(&infeasible.meta).expect("metadata")
        );

        // Finite values keep the plain JSON number form.
        let feasible = serde_json::to_value(result()).expect("serialize");
        assert_eq!(feasible["objective_value"], serde_json::json!(1.0));
        assert_eq!(feasible["turnover"], serde_json::json!(0.0));
    }
}

// Each map value uses the core non-finite scalar contract.
mod slack_values {
    use indexmap::IndexMap;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Value(#[serde(with = "finstack_quant_core::wire::non_finite_f64")] f64);

    /// Serialize ordered slack values with the core non-finite encoding.
    ///
    /// # Arguments
    ///
    /// * `values` - Ordered constraint names and slack values; NaN denotes an undefined average.
    /// * `serializer` - Serde output receiving finite numbers or core sentinel strings.
    pub fn serialize<S: serde::Serializer>(
        values: &IndexMap<String, f64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_map(values.iter().map(|(key, value)| (key, Value(*value))))
    }

    /// Decode finite or sentinel-encoded constraint slacks.
    ///
    /// # Arguments
    ///
    /// * `deserializer` - Serde input containing an ordered constraint-to-slack map.
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<IndexMap<String, f64>, D::Error> {
        Ok(IndexMap::<String, Value>::deserialize(deserializer)?
            .into_iter()
            .map(|(key, value)| (key, value.0))
            .collect())
    }
}
