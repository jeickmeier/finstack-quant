//! Prepared evaluation artifacts, setup containers, and prepared-run entry points.

use super::runner::*;
use crate::error::{Error, Result};
use crate::evaluator::cell::Cell;
use crate::evaluator::dag::{DependencyGraph, evaluate_order};
use crate::evaluator::results::{EvalStats, EvalWarning, StatementResult};
use crate::evaluator::{PeriodHistory, capital_structure_runtime};
use crate::types::{FinancialModelSpec, NodeId, NodeValueType};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::expr::Expr;
use indexmap::IndexMap;
use std::collections::HashSet;

/// One period's float values and typed cells, in column order.
pub(crate) type PeriodRow = (Vec<Option<f64>>, Vec<Option<Cell>>);

pub(super) type PeriodEvaluation = (
    IndexMap<String, crate::evaluator::ResultCell>,
    PeriodRow,
    Vec<EvalWarning>,
);

/// Per-path cache of Monte Carlo z-scores, keyed by node then period.
pub(crate) type McZCache = IndexMap<NodeId, IndexMap<PeriodId, f64>>;

/// The period one evaluation pass computes: the model, the period, whether
/// explicit actual values apply, and the topological node order.
#[derive(Clone, Copy)]
pub(crate) struct PeriodEval<'a> {
    /// Model being evaluated.
    pub(crate) model: &'a FinancialModelSpec,
    /// Period being evaluated, one of `model.periods`.
    pub(crate) period: &'a finstack_quant_core::dates::Period,
    /// Whether the period reads explicit actual values.
    pub(crate) is_actual: bool,
    /// Topologically sorted node order.
    pub(crate) eval_order: &'a [NodeId],
}

/// What one run fixes before any period is evaluated, shared by every
/// Monte Carlo path.
pub(super) struct RunSetup {
    /// Empty period history carrying the model's timeline.
    pub(super) history: PeriodHistory,
    /// Capital-structure state of the run; `None` without a capital
    /// structure or a market to generate it from.
    pub(super) capital: Option<CapitalSetup>,
}

/// The path-invariant capital-structure state of a run.
pub(super) struct CapitalSetup {
    /// The model's instruments, built once.
    pub(super) instruments: capital_structure_runtime::Instruments,
    /// Capital state holding each instrument's opening balance at the first
    /// period.
    pub(super) opening_state: crate::capital_structure::CapitalStructureState,
    /// Nodes that read `cs.*`, with their dependents.
    pub(super) affected_nodes: HashSet<NodeId>,
}

/// Cached structural analysis from [`Evaluator::prepare`], allowing repeated
/// evaluations of the same model structure without recompilation.
pub struct PreparedEvaluation {
    /// Topologically sorted node evaluation order.
    pub(crate) eval_order: Vec<NodeId>,
    /// Map from node id to its column index in the evaluation order.
    pub(crate) node_to_column: std::sync::Arc<IndexMap<NodeId, usize>>,
    /// Dependency graph retained for downstream consumers such as capital structure affected-node computation.
    pub(crate) dag: DependencyGraph,
    /// Compiled expressions snapshotted at prepare time so prepared
    /// evaluations are independent of the evaluator's mutable compiled cache
    /// (which other `evaluate` calls may overwrite).
    pub(crate) compiled_cache: std::sync::Arc<IndexMap<NodeId, Expr>>,
    /// Fingerprint of the timeline, node structure, forecasts and declared units
    /// this plan was compiled from, rejecting changes beyond input values.
    pub(crate) formula_fingerprint: u64,
    /// Canonical units inferred and validated when the plan was prepared.
    pub(crate) node_value_types: std::sync::Arc<IndexMap<NodeId, NodeValueType>>,
    /// The `value_type` validation recorded on each node of the validated
    /// model, so a run can hash the validated form of the caller's model.
    pub(crate) validated_value_types: IndexMap<NodeId, NodeValueType>,
}

impl PreparedEvaluation {
    /// The content identity of `model` in its validated form.
    ///
    /// Validation only adds inferred `value_type`s, so the caller's model with
    /// those filled in is the validated model, without a second analysis.
    pub(super) fn model_content_hash(&self, model: &FinancialModelSpec) -> Result<String> {
        let inferred = |id: &NodeId, node: &crate::types::NodeSpec| {
            node.value_type
                .is_none()
                .then(|| self.validated_value_types.get(id).copied())
                .flatten()
        };
        let hash = if model
            .nodes
            .iter()
            .any(|(id, node)| inferred(id, node).is_some())
        {
            let mut validated = model.clone();
            for (id, node) in &mut validated.nodes {
                if let Some(value_type) = inferred(id, node) {
                    node.value_type = Some(value_type);
                }
            }
            validated.canonical_hash()
        } else {
            model.canonical_hash()
        };
        hash.map_err(super::super::results::hash_error)
    }
}

/// Fingerprint the evaluation timeline and each node's calculation structure.
///
/// `PreparedEvaluation` snapshots *compiled* expressions, so a model whose
/// formulas changed after `prepare` would silently evaluate the stale code.
/// Comparing the node set cannot catch changes in formulas, units, forecast
/// parameters, observation availability or period dates, so these are all
/// fingerprinted while mutable explicit observation values are excluded.
///
/// `model.nodes` is an `IndexMap`, so iteration is insertion-ordered and the
/// fingerprint is deterministic. It is only ever compared against another
/// fingerprint from the same process, and never serialized.
pub(super) fn formula_fingerprint(model: &FinancialModelSpec) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for period in &model.periods {
        period.id.hash(&mut hasher);
        period.start.hash(&mut hasher);
        period.end.hash(&mut hasher);
        period.is_actual.hash(&mut hasher);
    }
    for (node_id, spec) in &model.nodes {
        node_id.as_str().hash(&mut hasher);
        spec.node_id.hash(&mut hasher);
        spec.node_type.hash(&mut hasher);
        spec.formula_text.hash(&mut hasher);
        spec.where_text.hash(&mut hasher);
        // The set of observation slots is structure; only the payload in an
        // existing slot may differ between prepare and evaluate.
        for period in spec.values.iter().flat_map(|values| values.keys()) {
            period.hash(&mut hasher);
        }
        spec.values.is_some().hash(&mut hasher);
        for (period, date) in &spec.availability_dates {
            period.hash(&mut hasher);
            date.hash(&mut hasher);
        }
        if let Some(forecast) = &spec.forecast {
            forecast.method().hash(&mut hasher);
            for (parameter, value) in &forecast.params() {
                parameter.hash(&mut hasher);
                value.hash(&mut hasher);
            }
        }
        match spec.value_type {
            None => 0_u8.hash(&mut hasher),
            Some(NodeValueType::Scalar) => 1_u8.hash(&mut hasher),
            Some(NodeValueType::Monetary { currency }) => {
                2_u8.hash(&mut hasher);
                currency.hash(&mut hasher);
            }
        }
    }
    model.periods.get_calendar().hash(&mut hasher);
    // The capital structure has no cheap structural hash; its canonical JSON
    // covers instruments, waterfall, reporting currency and FX policy.
    serde_json::to_string(&model.capital_structure)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

/// Principal outstanding before the first period: the debt balance, or zero
/// for a hedge, whose notional sizes coupons but is not borrowing principal.
pub(super) fn instrument_opening_balance(
    instrument: &dyn finstack_quant_valuations::instruments::Instrument,
    market_ctx: &finstack_quant_core::market_data::context::MarketContext,
    as_of: finstack_quant_core::dates::Date,
    period_start: finstack_quant_core::dates::Date,
) -> Result<finstack_quant_core::money::Money> {
    if crate::capital_structure::period_flows::is_debt_kind(instrument.key())? {
        capital_structure_runtime::resolve_opening_balance(
            instrument,
            market_ctx,
            as_of,
            period_start,
        )
    } else {
        let schedule = instrument.raw_cashflow_schedule(market_ctx, as_of)?;
        Ok(finstack_quant_core::money::Money::from((
            0_i64,
            schedule.get_notional().initial.currency(),
        )))
    }
}

/// Name the instrument and the remedy when its cashflows need market data the
/// evaluation was not given. Other errors pass through unchanged.
pub(super) fn missing_market_data(instrument_id: &str, err: Error) -> Error {
    if err.kind() == finstack_quant_core::ErrorKind::NotFound {
        Error::missing_data(format!(
            "capital-structure instrument '{instrument_id}' needs market data to generate its \
             cashflows ({err}); supply it through Evaluator::evaluate_with_market"
        ))
    } else {
        err
    }
}

impl Evaluator {
    /// Prepare the evaluator for repeated evaluations of the same model structure.
    ///
    /// Compiles formulas, builds the dependency graph, and caches the evaluation
    /// order so that subsequent calls to [`evaluate_prepared`](Self::evaluate_prepared)
    /// skip all structural analysis. Use this when running sensitivity analysis,
    /// goal seek, or any workflow that re-evaluates the same model with different
    /// input values. The returned plan snapshots compiled expressions; retain
    /// it only for models with the same node identities and formulas.
    ///
    /// # Errors
    ///
    /// Returns an error if a formula cannot be parsed or compiled, model
    /// dependencies cannot be resolved, or the dependency graph contains a
    /// cycle. A successful plan is structural only and performs no period
    /// evaluation.
    ///
    /// # Arguments
    ///
    /// * `model` - Validated model whose state and conventions drive the calculation.
    pub fn prepare(&mut self, model: &FinancialModelSpec) -> Result<PreparedEvaluation> {
        // Public Rust models are mutable and may be constructed without the
        // builder. Revalidate formulas and infer units before compiling, rather
        // than trusting a validation that preceded the caller's last mutation.
        let mut validated = model.clone();
        let analysis = validated.analyze_semantics()?;
        self.compiled_cache = std::sync::Arc::new(analysis.compiled);
        self.forecast_cache.clear();
        self.visibility_cutoff = None;
        let dag = analysis.graph;
        let eval_order = evaluate_order(&dag)?;
        let node_to_column = std::sync::Arc::new(
            eval_order
                .iter()
                .enumerate()
                .map(|(i, node_id)| (node_id.clone(), i))
                .collect(),
        );
        let node_value_types: IndexMap<NodeId, NodeValueType> = validated
            .nodes
            .iter()
            .map(|(id, node)| (id.clone(), node.value_type.unwrap_or(NodeValueType::Scalar)))
            .collect();
        let node_value_types = std::sync::Arc::new(node_value_types);
        self.node_value_types = std::sync::Arc::clone(&node_value_types);
        let validated_value_types = validated
            .nodes
            .iter()
            .filter_map(|(id, node)| Some((id.clone(), node.value_type?)))
            .collect();
        Ok(PreparedEvaluation {
            eval_order,
            node_to_column,
            dag,
            compiled_cache: std::sync::Arc::clone(&self.compiled_cache),
            formula_fingerprint: formula_fingerprint(model),
            node_value_types,
            validated_value_types,
        })
    }

    /// Evaluate a model reusing previously compiled formulas and evaluation order.
    ///
    /// The model must have the same structure (same nodes and formulas) as the one
    /// passed to [`prepare`](Self::prepare); only node *values* may differ. This
    /// avoids formula recompilation, DAG construction, and cycle detection on every
    /// call — typically saving 30-50 % of total evaluation time. Prepared
    /// evaluation has no as-of visibility cutoff and evaluates the capital
    /// structure exactly as [`evaluate`](Self::evaluate) does (an empty market
    /// as of the first period's start); use
    /// [`evaluate_with_market`](Self::evaluate_with_market) for instruments
    /// that need market data.
    ///
    /// # Errors
    ///
    /// Returns an evaluation error if `prepared` does not cover exactly the
    /// model's current node set; rebuild it with [`prepare`](Self::prepare)
    /// after any structural change. It also propagates ordinary evaluation
    /// errors for missing inputs, incompatible explicit scalar/currency types,
    /// failed forecasts, formulas, and final checks. Declared units must remain
    /// unchanged; rebuild the model and plan for a different currency or type.
    /// Callers must rebuild after formula changes as well: the plan deliberately
    /// reuses the compiled expressions captured at preparation time.
    ///
    /// # Arguments
    ///
    /// * `model` - Validated model whose state and conventions drive the calculation.
    /// * `prepared` - Prepared used by the algorithm, subject to the enclosing type invariants and documented units.
    pub fn evaluate_prepared(
        &mut self,
        model: &FinancialModelSpec,
        prepared: &PreparedEvaluation,
    ) -> Result<StatementResult> {
        self.evaluate_prepared_inner(model, prepared, None, None)
            .inspect_err(|err| {
                tracing::error!(
                    model_id = %model.id,
                    periods = model.periods.len(),
                    nodes = model.nodes.len(),
                    error = %err,
                    "prepared statement evaluation failed"
                );
            })
    }

    /// Evaluate the prepared statement graph at an explicit market and observation cut.
    ///
    /// See [`Self::evaluate_prepared`] for the invariants on observation slots
    /// and inferred units across repeated model runs.
    ///
    /// # Arguments
    /// * `model` - Statement model whose observation payloads may vary across runs.
    /// * `prepared` - Same period plan, observation slots, formulas and model structure as the snapshot.
    /// * `market` - Held market and pricing data for capital-structure calculations.
    /// * `as_of` - Valuation day used to price eligible cashflows.
    /// * `through` - Inclusive information cut, on or before the valuation date.
    ///
    /// # Errors
    /// Returns an error for an invalid cut, changed plan structure, invalid data
    /// availability, or a failed statement or capital-structure computation.
    pub fn evaluate_prepared_at(
        &mut self,
        model: &FinancialModelSpec,
        prepared: &PreparedEvaluation,
        market: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        through: finstack_quant_core::dates::Date,
    ) -> Result<StatementResult> {
        if through > as_of {
            return Err(Error::invalid_input(
                "Statement observation cut follows the valuation date",
            ));
        }
        model.validate_point_in_time()?;
        self.evaluate_prepared_inner(model, prepared, Some((market, as_of)), Some(through))
    }

    fn evaluate_prepared_inner(
        &mut self,
        model: &FinancialModelSpec,
        prepared: &PreparedEvaluation,
        market: Option<(
            &finstack_quant_core::market_data::context::MarketContext,
            finstack_quant_core::dates::Date,
        )>,
        visible_through: Option<finstack_quant_core::dates::Date>,
    ) -> Result<StatementResult> {
        let _span = tracing::info_span!(
            "statements.evaluate_prepared",
            model_id = model.id.as_str(),
            periods = model.periods.len(),
            nodes = model.nodes.len(),
        )
        .entered();
        let explicit_value_types = model.validate_explicit_values()?;
        self.forecast_cache.clear();
        self.visibility_cutoff = visible_through;
        // Use the compiled expressions captured at prepare time, not whatever
        // the evaluator's mutable cache currently holds (a later `evaluate` of
        // a different model sharing node ids would otherwise leak its formulas
        // into this prepared evaluation).
        self.compiled_cache = std::sync::Arc::clone(&prepared.compiled_cache);
        self.node_value_types = std::sync::Arc::clone(&prepared.node_value_types);

        // Structural guard: the prepared plan must cover exactly the model's
        // nodes. A node present in the model but absent from the plan would be
        // silently skipped here yet still labeled by `finalize_results`,
        // producing a well-formed result with whole series missing. Fail loudly
        // instead — the docs require the prepared plan and model to share
        // structure (only node values may differ).
        if prepared.node_to_column.len() != model.nodes.len() {
            return Err(Error::eval(format!(
                "evaluate_prepared: model has {} nodes but the prepared plan has {}; rebuild the \
                 plan with `prepare` for this model (only node values may differ between prepare \
                 and evaluate)",
                model.nodes.len(),
                prepared.node_to_column.len()
            )));
        }
        for node_id in model.nodes.keys() {
            if !prepared.node_to_column.contains_key(node_id) {
                return Err(Error::eval(format!(
                    "evaluate_prepared: model node '{}' is missing from the prepared plan; \
                     rebuild the plan with `prepare` for this model",
                    node_id.as_str()
                )));
            }
        }

        // The node set can match while the formulas differ. The plan holds
        // *compiled* expressions, so evaluating would silently use the code the
        // plan was built from and return a well-formed but wrong result.
        if formula_fingerprint(model) != prepared.formula_fingerprint {
            return Err(Error::eval(
                "evaluate_prepared: a formula, where clause, declared value type, period, observation slot, \
                 availability date, forecast or capital structure differs from the model the \
                 prepared plan was built for. The plan holds compiled expressions, so \
                 evaluating would silently use the old formula. Rebuild the plan with \
                 `prepare` (only node *values* may differ between prepare and evaluate).",
            ));
        }

        // A directly constructed model need not declare its units. Once the
        // declared structure matches, also preserve the units inferred by prepare.
        for (node_id, actual) in explicit_value_types {
            if prepared.node_value_types.get(&node_id) != Some(&actual) {
                return Err(Error::eval(format!(
                    "evaluate_prepared: input '{node_id}' has type {actual:?}, which differs \
                     from its prepared value type; rebuild the model and plan"
                )));
            }
        }

        let empty_market = finstack_quant_core::market_data::context::MarketContext::new();
        let capital_structure =
            market.or_else(|| contractual_capital_structure(model, &empty_market));
        let as_of = market.map(|(_, as_of)| as_of);
        let setup = self.run_setup(model, prepared, capital_structure)?;
        let (mut results, all_warnings) =
            self.evaluate_periods(model, prepared, &setup, capital_structure, None)?;

        self.finalize_results(
            &mut results,
            model,
            prepared,
            EvalStats {
                eval_time_ms: None,
                num_nodes: model.nodes.len(),
                num_periods: model.periods.len(),
                numeric_mode: crate::evaluator::NumericMode::Float64,
                parallel: false,
                warnings: all_warnings,
            },
            as_of,
            market
                .map(|(context, _)| Self::market_content_hash(context))
                .transpose()?
                .flatten(),
        )?;

        Ok(results)
    }
}
