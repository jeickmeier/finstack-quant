//! Main evaluator implementation.

use crate::error::{Error, Result};
use crate::evaluator::cell::{Cell, UnavailableReason};
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::dag::{DependencyGraph, evaluate_order};
use crate::evaluator::forecast_eval;
use crate::evaluator::formula::evaluate_value;
use crate::evaluator::monte_carlo::{
    MonteCarloAccumulator, MonteCarloConfig, MonteCarloResults, PathResult,
};
use crate::evaluator::precedence::{NodeValueSource, resolve_node_value};
use crate::evaluator::results::{CellSource, EvalStats, EvalWarning, StatementResult};
use crate::evaluator::value::{self, At};
use crate::evaluator::{
    PeriodHistory, capital_structure_runtime, capital_structure_runtime::dependent_closure,
};
use crate::forecast::stream::PathId;
use crate::types::{AmountOrScalar, FinancialModelSpec, NodeId, NodeValueType};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::expr::Expr;
use indexmap::IndexMap;
use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// One period's float values and typed cells, in column order.
pub(crate) type PeriodRow = (Vec<Option<f64>>, Vec<Option<Cell>>);

type PeriodEvaluation = (
    IndexMap<String, crate::evaluator::ResultCell>,
    PeriodRow,
    Vec<EvalWarning>,
);

/// Per-path cache of Monte Carlo z-scores, keyed by node then period.
pub(super) type McZCache = IndexMap<NodeId, IndexMap<PeriodId, f64>>;

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

/// Cached structural analysis from [`Evaluator::prepare`], allowing repeated
/// evaluations of the same model structure without recompilation.
pub struct PreparedEvaluation {
    /// Topologically sorted node evaluation order.
    pub(super) eval_order: Vec<NodeId>,
    /// Map from node id to its column index in the evaluation order.
    pub(super) node_to_column: std::sync::Arc<IndexMap<NodeId, usize>>,
    /// Dependency graph retained for downstream consumers such as capital structure affected-node computation.
    dag: DependencyGraph,
    /// Compiled expressions snapshotted at prepare time so prepared
    /// evaluations are independent of the evaluator's mutable compiled cache
    /// (which other `evaluate` calls may overwrite).
    pub(super) compiled_cache: std::sync::Arc<IndexMap<NodeId, Expr>>,
    /// Fingerprint of the timeline, node structure, forecasts and declared units
    /// this plan was compiled from, rejecting changes beyond input values.
    formula_fingerprint: u64,
    /// Canonical units inferred and validated when the plan was prepared.
    pub(super) node_value_types: IndexMap<NodeId, NodeValueType>,
    /// The `value_type` validation recorded on each node of the validated
    /// model, so a run can hash the validated form of the caller's model.
    validated_value_types: IndexMap<NodeId, NodeValueType>,
}

impl PreparedEvaluation {
    /// The content identity of `model` in its validated form.
    ///
    /// Validation only adds inferred `value_type`s, so the caller's model with
    /// those filled in is the validated model, without a second analysis.
    fn model_content_hash(&self, model: &FinancialModelSpec) -> Result<String> {
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
        hash.map_err(super::results::hash_error)
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
fn formula_fingerprint(model: &FinancialModelSpec) -> u64 {
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
    model.periods.calendar.hash(&mut hasher);
    // The capital structure has no cheap structural hash; its canonical JSON
    // covers instruments, waterfall, reporting currency and FX policy.
    serde_json::to_string(&model.capital_structure)
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

/// Principal outstanding before the first period: the debt balance, or zero
/// for a hedge, whose notional sizes coupons but is not borrowing principal.
fn instrument_opening_balance(
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
fn missing_market_data(instrument_id: &str, err: Error) -> Error {
    if err.kind() == finstack_quant_core::ErrorKind::NotFound {
        Error::missing_data(format!(
            "capital-structure instrument '{instrument_id}' needs market data to generate its \
             cashflows ({err}); supply it through Evaluator::evaluate_with_market"
        ))
    } else {
        err
    }
}

/// Evaluator for financial models.
///
/// The evaluator compiles formulas, resolves dependencies, and evaluates
/// nodes period-by-period according to precedence rules.
///
/// # Example
///
/// ```rust
/// # use finstack_quant_statements::builder::ModelBuilder;
/// # use finstack_quant_statements::evaluator::Evaluator;
/// # use finstack_quant_core::dates::PeriodId;
/// let model = {
///     let mut model_builder = ModelBuilder::new("demo");
///     model_builder.periods("2025Q1..Q2", None)?;
///     model_builder.value("revenue", &[
///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), 100_000.0.into()),
///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), 105_000.0.into()),
///     ]);
///     model_builder.compute("gross_profit", "revenue * 0.6")?;
///     model_builder.build()?
/// };
///
/// let mut evaluator = Evaluator::new();
/// let results = evaluator.evaluate(&model)?;
/// assert_eq!(results.get("revenue", &PeriodId::quarter(2025, 1).expect("valid period fixture")), Some(100_000.0));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct Evaluator {
    /// Cached compiled expressions (Arc-shared across Monte Carlo path clones)
    compiled_cache: std::sync::Arc<IndexMap<NodeId, Expr>>,

    /// Resolved units of the prepared model's nodes (Arc-shared like the
    /// compiled expressions); they type each computed cell.
    node_value_types: std::sync::Arc<IndexMap<NodeId, NodeValueType>>,

    /// Cached forecast results: node_id → (period_id → value)
    forecast_cache: IndexMap<NodeId, forecast_eval::ForecastRun>,

    /// Optional check suite run after evaluation to produce inline validation
    check_suite: Option<std::sync::Arc<crate::checks::CheckSuite>>,

    /// As-of visibility cutoff for the current evaluation run. Actual periods
    /// starting after this date have hidden explicit values and are treated as
    /// forecast periods by forecast evaluation.
    visibility_cutoff: Option<finstack_quant_core::dates::Date>,
}

impl Evaluator {
    /// Create a new evaluator.
    #[must_use = "creating an evaluator has no effect without calling evaluate()"]
    pub fn new() -> Self {
        Self {
            compiled_cache: std::sync::Arc::new(IndexMap::new()),
            node_value_types: std::sync::Arc::new(IndexMap::new()),
            forecast_cache: IndexMap::new(),
            check_suite: None,
            visibility_cutoff: None,
        }
    }

    /// Attach a [`CheckSuite`](crate::checks::CheckSuite) to run after each evaluation.
    ///
    /// When set, `evaluate()` and related methods will automatically run the
    /// suite against the model and results, attaching the report to
    /// [`StatementResult::check_report`].
    #[must_use]
    pub fn with_checks(mut self, suite: crate::checks::CheckSuite) -> Self {
        self.check_suite = Some(std::sync::Arc::new(suite));
        self
    }

    /// Evaluate a financial model over all periods with required market context.
    ///
    /// Use this method when capital-structure instruments need market data
    /// (floating coupons, swaps, caps, swaptions) or when explicit values must
    /// be cut off at an as-of date. [`Evaluator::evaluate`] also evaluates the
    /// capital structure, but from an empty market with every value visible.
    ///
    /// # Observation cutoff
    ///
    /// `cutoff` selects which explicit observations the evaluation may read.
    /// It is separate from `as_of`, which dates pricing only.
    ///
    /// - [`ObservationCutoff::FullSnapshot`] reads every observation in the
    ///   model.
    /// - [`ObservationCutoff::Through`] reads an observation only when its
    ///   availability date is on or before the cutoff date, inclusive through
    ///   the end of that day. The model must name its source snapshot and
    ///   give an availability date to every observation in an actual period.
    ///   An undated explicit value in a forecast period is a forecast
    ///   assumption and is always visible.
    ///
    /// A hidden value keeps its place in the model timeline, but the node
    /// resolves through forecast or formula fallbacks under normal precedence.
    ///
    /// `actuals_until` still classifies periods as actual or forecast.
    /// Availability is observation-specific and only decides whether an
    /// explicit value was known at this evaluation's point in time.
    ///
    /// # Arguments
    ///
    /// * `model` - The financial model specification
    /// * `market_ctx` - Market context for pricing instruments
    /// * `as_of` - Valuation date for pricing the capital structure
    /// * `cutoff` - Explicit observations the evaluation may read: the full
    ///   snapshot, or those published on or before a date
    ///
    /// # Returns
    ///
    /// Returns `StatementResult` containing the evaluated values for all nodes and periods.
    /// The result includes capital-structure cashflows when the model defines
    /// them, together with any attached check-suite report and evaluation
    /// warnings.
    ///
    /// # Errors
    ///
    /// Returns an error if model formulas cannot be parsed or compiled, its
    /// dependency graph is invalid or cyclic, a required value or forecast
    /// cannot be resolved, capital-structure instruments cannot be built or
    /// priced from `market_ctx`, or final checks fail. Under a cutoff date it
    /// also returns an error if the model names no source snapshot, an
    /// observation in an actual period has no availability date, or a hidden
    /// observation has no forecast or formula to fall back on.
    pub fn evaluate_with_market(
        &mut self,
        model: &FinancialModelSpec,
        market_ctx: &finstack_quant_core::market_data::context::MarketContext,
        as_of: finstack_quant_core::dates::Date,
        cutoff: ObservationCutoff,
    ) -> Result<StatementResult> {
        let visibility_cutoff = match cutoff {
            ObservationCutoff::FullSnapshot => None,
            ObservationCutoff::Through(date) => {
                model.validate_point_in_time()?;
                Some(date)
            }
        };
        self.evaluate_inner(
            model,
            Some((market_ctx, as_of)),
            visibility_cutoff,
            Some(as_of),
        )
        .inspect_err(|err| {
            tracing::error!(
                model_id = %model.id,
                periods = model.periods.len(),
                nodes = model.nodes.len(),
                has_market = true,
                %as_of,
                error = %err,
                "statement evaluation (with market) failed"
            );
        })
    }

    /// Internal evaluation implementation shared by [`Evaluator::evaluate`] and
    /// [`Evaluator::evaluate_with_market`].
    ///
    /// `capital_structure` is the market and cashflow date the model's
    /// instruments are generated from (`None` skips the capital structure).
    /// `visibility_cutoff` independently hides explicit actuals that were not
    /// yet available.
    fn evaluate_inner(
        &mut self,
        model: &FinancialModelSpec,
        capital_structure: Option<(
            &finstack_quant_core::market_data::context::MarketContext,
            finstack_quant_core::dates::Date,
        )>,
        visibility_cutoff: Option<finstack_quant_core::dates::Date>,
        valuation_date: Option<finstack_quant_core::dates::Date>,
    ) -> Result<StatementResult> {
        let _span = tracing::info_span!(
            "statements.evaluate",
            model_id = model.id.as_str(),
            periods = model.periods.len(),
            nodes = model.nodes.len(),
            has_market = capital_structure.is_some(),
            has_as_of = visibility_cutoff.is_some()
        )
        .entered();
        #[cfg(not(target_arch = "wasm32"))]
        let start = Instant::now();

        let prepared = self.prepare(model)?;
        self.visibility_cutoff = visibility_cutoff;
        let (mut results, all_warnings) =
            self.evaluate_periods(model, &prepared, capital_structure, None)?;

        self.finalize_results(
            &mut results,
            model,
            &prepared,
            EvalStats {
                #[cfg(not(target_arch = "wasm32"))]
                eval_time_ms: Some(start.elapsed().as_millis() as u64),
                #[cfg(target_arch = "wasm32")]
                eval_time_ms: None,
                num_nodes: model.nodes.len(),
                num_periods: model.periods.len(),
                numeric_mode: crate::evaluator::NumericMode::Float64,
                parallel: false,
                warnings: all_warnings,
            },
            valuation_date,
        )?;

        Ok(results)
    }

    /// Run every period in timeline order on a prepared plan.
    ///
    /// `capital_structure` is the market and cashflow date the model's
    /// instruments are generated from; `None` skips the capital structure.
    /// `mc_path` is the Monte Carlo path (`None` for a single
    /// deterministic run); each path runs this same loop, capital-structure
    /// waterfall included. Shared by [`evaluate_inner`](Self::evaluate_inner),
    /// [`evaluate_prepared_inner`](Self::evaluate_prepared_inner) and
    /// [`evaluate_monte_carlo`](Self::evaluate_monte_carlo).
    fn evaluate_periods(
        &mut self,
        model: &FinancialModelSpec,
        prepared: &PreparedEvaluation,
        capital_structure: Option<(
            &finstack_quant_core::market_data::context::MarketContext,
            finstack_quant_core::dates::Date,
        )>,
        mc_path: Option<PathId>,
    ) -> Result<(StatementResult, Vec<EvalWarning>)> {
        let mut mc_z_cache = McZCache::new();
        let market_ctx = capital_structure.map(|(market, _)| market);
        let as_of = capital_structure.map(|(_, as_of)| as_of);

        // Only nodes whose formula or where clause reaches `cs.*` (and their
        // dependents) need the capital-structure pass.
        let mut cs_seed_nodes = HashSet::new();
        for (node_id, spec) in model.nodes.iter().filter(|_| capital_structure.is_some()) {
            let formula_references_cs = spec
                .formula_text
                .as_deref()
                .map(crate::utils::formula::contains_capital_structure_reference)
                .transpose()?
                .unwrap_or(false);
            let predicate_references_cs = spec
                .where_text
                .as_deref()
                .map(crate::utils::formula::contains_capital_structure_reference)
                .transpose()?
                .unwrap_or(false);
            if formula_references_cs || predicate_references_cs {
                cs_seed_nodes.insert(node_id.clone());
            }
        }

        let cs_affected_nodes = dependent_closure(&prepared.dag, &cs_seed_nodes);

        let mut cs_state = if let (Some(_market_ctx), Some(_as_of)) = (market_ctx, as_of) {
            Some(crate::capital_structure::CapitalStructureState::new())
        } else {
            None
        };

        // Pre-compute instruments if market context is available
        let instruments = if let (Some(_market_ctx), Some(_as_of)) = (market_ctx, as_of) {
            self.build_instruments(model)?
        } else {
            None
        };

        if let (Some(state), Some(insts), Some(market_ctx), Some(as_of_date), Some(first_period)) = (
            cs_state.as_mut(),
            instruments.as_ref(),
            market_ctx,
            as_of,
            model.periods.first(),
        ) {
            for (instrument_id, instrument) in insts {
                let opening_balance = instrument_opening_balance(
                    instrument.as_ref(),
                    market_ctx,
                    as_of_date,
                    first_period.start,
                )
                .map_err(|err| missing_market_data(instrument_id, err))?;

                state
                    .opening_balances
                    .insert(instrument_id.clone(), opening_balance);
            }
        }

        let mut historical = std::sync::Arc::new(PeriodHistory::with_periods(
            std::sync::Arc::clone(&prepared.node_to_column),
            &model.periods,
        ));
        let mut historical_cs: std::sync::Arc<
            IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>,
        > = std::sync::Arc::new(IndexMap::new());
        let mut all_warnings = Vec::new();
        let mut results = StatementResult::new();

        // Accumulator for capital structure cashflows across all periods
        let mut cs_cashflows_accum = crate::capital_structure::CapitalStructureCashflows::new();
        let mut has_cs = false;

        for period in &model.periods {
            let (period_results, period_row, period_warnings) =
                if let (Some(market_ctx), Some(as_of), Some(ref mut state), Some(insts)) =
                    (market_ctx, as_of, cs_state.as_mut(), instruments.as_ref())
                {
                    let (vals, row, warns, period_cs) = self.evaluate_period_dynamic(
                        PeriodEval {
                            model,
                            period,
                            is_actual: period.is_actual,
                            eval_order: &prepared.eval_order,
                        },
                        &historical,
                        &historical_cs,
                        capital_structure_runtime::CapitalStructureRun {
                            market_ctx,
                            as_of,
                            instruments: insts,
                            state,
                            affected_nodes: &cs_affected_nodes,
                        },
                        mc_path.map(|path| (path, &mut mc_z_cache)),
                    )?;

                    cs_cashflows_accum.set_period(period_cs)?;
                    has_cs = true;

                    (vals, row, warns)
                } else {
                    self.evaluate_period(
                        PeriodEval {
                            model,
                            period,
                            is_actual: period.is_actual,
                            eval_order: &prepared.eval_order,
                        },
                        &historical,
                        &historical_cs,
                        mc_path.map(|path| (path, &mut mc_z_cache)),
                    )?
                };

            all_warnings.extend(period_warnings);

            for (node_id, cell) in period_results {
                results.set_cell(&node_id, period.id, cell);
            }

            // Add to historical context for next period (move, not clone)
            std::sync::Arc::make_mut(&mut historical).push_typed_row(
                period.id,
                period_row.0,
                period_row.1,
            );
            if has_cs {
                let period_snapshot = cs_cashflows_accum.period_snapshot(period.id);
                std::sync::Arc::make_mut(&mut historical_cs).insert(period.id, period_snapshot);
            }

            // Advance CS state for next period
            if let Some(ref mut state) = cs_state {
                state.advance_period();
            }
        }

        // Expose accumulated capital structure cashflows on the result
        if has_cs {
            results.cs_cashflows = Some(cs_cashflows_accum);
        }

        Ok((results, all_warnings))
    }

    /// Evaluate the model without a market under an observation cutoff.
    ///
    /// The contractual capital structure is read as in [`Self::evaluate`];
    /// a cutoff date hides observations published after it.
    pub(crate) fn evaluate_under_cutoff(
        &mut self,
        model: &FinancialModelSpec,
        cutoff: &ObservationCutoff,
    ) -> Result<StatementResult> {
        let visibility_cutoff = match cutoff {
            ObservationCutoff::FullSnapshot => None,
            ObservationCutoff::Through(date) => {
                model.validate_point_in_time()?;
                Some(*date)
            }
        };
        let empty_market = finstack_quant_core::market_data::context::MarketContext::new();
        self.evaluate_inner(
            model,
            contractual_capital_structure(model, &empty_market),
            visibility_cutoff,
            None,
        )
    }

    /// Evaluate a financial model over all periods without market context.
    ///
    /// Every explicit value is visible: there is no as-of cutoff.
    ///
    /// A model with a capital structure is evaluated with it. Its instruments
    /// generate their contractual cashflows from an empty market as of the
    /// first period's start, so fixed-rate debt needs no market data and the
    /// result carries `cs_cashflows` exactly as
    /// [`Evaluator::evaluate_with_market`] would give with an empty market on
    /// that date. Instruments whose cashflows depend on market data (floating
    /// coupons, swaps, caps, swaptions) need
    /// [`Evaluator::evaluate_with_market`].
    ///
    /// # Arguments
    ///
    /// * `model` - The financial model specification
    /// # Returns
    ///
    /// Returns `StatementResult` containing the evaluated values for all nodes
    /// and periods, with capital-structure cashflows when the model defines a
    /// capital structure.
    ///
    /// # Errors
    ///
    /// Returns an error if formulas cannot be parsed or compiled, the model's
    /// dependency graph is invalid or cyclic, a required input/forecast cannot
    /// be resolved, a capital-structure instrument needs market data to
    /// generate its cashflows (a missing-data error naming the instrument) or
    /// otherwise fails, or final model checks fail.
    pub fn evaluate(&mut self, model: &FinancialModelSpec) -> Result<StatementResult> {
        let empty_market = finstack_quant_core::market_data::context::MarketContext::new();
        self.evaluate_inner(
            model,
            contractual_capital_structure(model, &empty_market),
            None,
            None,
        )
        .inspect_err(|err| {
            tracing::error!(
                model_id = %model.id,
                periods = model.periods.len(),
                nodes = model.nodes.len(),
                error = %err,
                "statement evaluation failed"
            );
        })
    }

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
        self.node_value_types = std::sync::Arc::new(node_value_types.clone());
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
        self.evaluate_prepared_inner(model, prepared)
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

    fn evaluate_prepared_inner(
        &mut self,
        model: &FinancialModelSpec,
        prepared: &PreparedEvaluation,
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
        self.visibility_cutoff = None;
        // Use the compiled expressions captured at prepare time, not whatever
        // the evaluator's mutable cache currently holds (a later `evaluate` of
        // a different model sharing node ids would otherwise leak its formulas
        // into this prepared evaluation).
        self.compiled_cache = std::sync::Arc::clone(&prepared.compiled_cache);
        self.node_value_types = std::sync::Arc::new(prepared.node_value_types.clone());

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
        let (mut results, all_warnings) = self.evaluate_periods(
            model,
            prepared,
            contractual_capital_structure(model, &empty_market),
            None,
        )?;

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
            None,
        )?;

        Ok(results)
    }

    /// Evaluate a financial model in Monte Carlo mode.
    ///
    /// The model is replayed `config.n_paths` times on the ordinary period
    /// engine, each path with fresh state and its own deterministic random
    /// streams, and the requested metrics are aggregated into R-7 sample
    /// quantiles per forecast period. A model with a capital structure has
    /// its instruments and waterfall evaluated on every path, so each `cs.*`
    /// reference sees that path's own debt service and balances. The same
    /// seed replays the same paths, serially or in parallel.
    ///
    /// # Arguments
    ///
    /// * `model` - The model to simulate. It needs at least one forecast
    ///   period.
    /// * `config` - Path count, exact seed, the metrics to report and the
    ///   probability levels.
    /// * `context` - The market, valuation date and observation cutoff the
    ///   paths evaluate in, as for
    ///   [`evaluate_with_market`](Self::evaluate_with_market). `None`
    ///   evaluates as [`evaluate`](Self::evaluate) does: the full snapshot,
    ///   with the capital structure projected on an empty market as of the
    ///   first period's start.
    ///
    /// # Returns
    ///
    /// Quantiles for a complete run. When a requested sample has no value on
    /// some path the run is incomplete: no statistics are published and
    /// `incomplete` names the first such sample.
    ///
    /// # Errors
    ///
    /// Returns an evaluation error when the configuration is invalid (a path
    /// count of zero or beyond the path-identifier domain, no or unknown or
    /// repeated metrics, repeated or out-of-range percentiles, a run larger
    /// than the sample limit), the model has no forecast period, the model
    /// cannot be prepared, or a path fails; of several failing paths the
    /// lowest-numbered one is reported.
    pub fn evaluate_monte_carlo(
        &mut self,
        model: &FinancialModelSpec,
        config: &MonteCarloConfig,
        context: Option<crate::evaluator::MonteCarloContext<'_>>,
    ) -> Result<MonteCarloResults> {
        self.evaluate_monte_carlo_inner(model, config, context)
            .inspect_err(|err| {
                tracing::error!(
                    model_id = %model.id,
                    paths = config.n_paths,
                    periods = model.periods.len(),
                    nodes = model.nodes.len(),
                    seed = config.seed,
                    error = %err,
                    "Monte Carlo evaluation failed"
                );
            })
    }

    fn evaluate_monte_carlo_inner(
        &mut self,
        model: &FinancialModelSpec,
        config: &MonteCarloConfig,
        context: Option<crate::evaluator::MonteCarloContext<'_>>,
    ) -> Result<MonteCarloResults> {
        let _span = tracing::info_span!(
            "statements.evaluate_monte_carlo",
            model_id = model.id.as_str(),
            paths = config.n_paths,
            periods = model.periods.len(),
            nodes = model.nodes.len()
        )
        .entered();

        let prepared = self.prepare(model)?;
        // Size and requests are checked before any path runs.
        let accumulator_seed =
            MonteCarloAccumulator::new(model, config, &prepared.node_value_types)?;
        let empty_market = finstack_quant_core::market_data::context::MarketContext::new();
        let capital_structure = match context {
            Some(context) => {
                self.visibility_cutoff = match context.cutoff {
                    ObservationCutoff::FullSnapshot => None,
                    ObservationCutoff::Through(date) => {
                        model.validate_point_in_time()?;
                        Some(date)
                    }
                };
                model
                    .capital_structure
                    .as_ref()
                    .map(|_| (context.market, context.as_of))
            }
            None => contractual_capital_structure(model, &empty_market),
        };

        // Run paths — parallel via rayon on native targets, serial on wasm32
        // (rayon's thread pool is unavailable in single-threaded wasm).
        // Both paths are deterministic for a given seed, and both report the
        // lowest-index failing path when several paths fail.
        let run_single_path = |path_idx: usize| -> Result<PathResult> {
            let mut path_eval = self.clone();
            path_eval.forecast_cache.clear();

            let path = PathId {
                run_seed: config.seed,
                path: path_idx as u64,
            };
            let (results, warnings) =
                path_eval.evaluate_periods(model, &prepared, capital_structure, Some(path))?;
            Ok((results.path_samples(accumulator_seed.metrics()), warnings))
        };

        // Native: each rayon split folds its paths in increasing index order and
        // stops at its first failure, carrying `(path_idx, error)`; the reduce
        // keeps the smaller index. The reported error is therefore the
        // lowest-index failing path's on every thread count, the same error
        // the serial wasm32 loop returns. A merge failure (not tied to a path)
        // carries `usize::MAX`, so any path error takes precedence.
        #[cfg(not(target_arch = "wasm32"))]
        let accumulator = {
            use rayon::prelude::*;
            type Partial = std::result::Result<MonteCarloAccumulator, (usize, Error)>;
            (0..config.n_paths)
                .into_par_iter()
                .fold(
                    || -> Partial { Ok(accumulator_seed.empty_like()) },
                    |state, path_idx| {
                        let mut acc = state?;
                        run_single_path(path_idx)
                            .and_then(|(path_results, warnings)| {
                                acc.push_path(path_idx, path_results, warnings)
                            })
                            .map_err(|error| (path_idx, error))?;
                        Ok(acc)
                    },
                )
                .reduce(
                    || Ok(accumulator_seed.empty_like()),
                    |left, right| match (left, right) {
                        (Ok(left), Ok(right)) => {
                            left.merge(right).map_err(|error| (usize::MAX, error))
                        }
                        (Err(left), Err(right)) => {
                            Err(if left.0 <= right.0 { left } else { right })
                        }
                        (Err(error), Ok(_)) | (Ok(_), Err(error)) => Err(error),
                    },
                )
                .map_err(|(_, error)| error)?
        };

        #[cfg(target_arch = "wasm32")]
        let accumulator = {
            let mut acc = accumulator_seed.empty_like();
            for path_idx in 0..config.n_paths {
                let (path_results, warnings) = run_single_path(path_idx)?;
                acc.push_path(path_idx, path_results, warnings)?;
            }
            acc
        };

        accumulator.finish()
    }

    /// Populate value types, set metadata, and run the optional check suite.
    fn finalize_results(
        &self,
        results: &mut StatementResult,
        model: &FinancialModelSpec,
        prepared: &PreparedEvaluation,
        meta: EvalStats,
        valuation_date: Option<finstack_quant_core::dates::Date>,
    ) -> Result<()> {
        results.meta = meta;
        results.periods = Some(model.periods.clone());
        results.populate_value_types(&prepared.node_value_types);
        results.provenance = Some(crate::evaluator::RunProvenance::with_model_hash(
            model,
            prepared.model_content_hash(model)?,
            self.visibility_cutoff
                .map_or(ObservationCutoff::FullSnapshot, ObservationCutoff::Through),
            valuation_date,
        )?);
        if let Some(suite) = &self.check_suite {
            results.check_report = Some(suite.run(model, results)?);
        }
        Ok(())
    }

    /// Evaluate nodes in topological order within a context.
    ///
    /// This is the shared inner loop used by all period evaluation methods.
    /// It handles where-clause masking, precedence resolution, forecast/formula
    /// dispatch, and storing results in the context.
    pub(crate) fn evaluate_nodes_in_order(
        &mut self,
        eval: PeriodEval<'_>,
        context: &mut EvaluationContext,
        mc_path: Option<PathId>,
        node_filter: Option<&HashSet<NodeId>>,
        mut mc_z_cache: Option<&mut McZCache>,
    ) -> Result<()> {
        let PeriodEval {
            model,
            period: eval_period,
            is_actual,
            eval_order,
        } = eval;
        let period_id = &eval_period.id;
        let visibility_cutoff = self.visibility_cutoff;
        let period = model
            .periods
            .iter()
            .find(|period| period.id == *period_id)
            .ok_or_else(|| Error::eval(format!("Period '{period_id}' not found in model")))?;
        for node_id in eval_order {
            if let Some(filter) = node_filter
                && !filter.contains(node_id.as_str())
            {
                continue;
            }

            let node_spec = model
                .get_node(node_id.as_str())
                .ok_or_else(|| Error::eval(format!("Node '{}' not found in model", node_id)))?;

            // The mask is evaluated before any value source. A false mask is a
            // masked cell with no numeric value; a mask with no value leaves
            // the cell unavailable; an invalid mask is an error.
            if node_spec.where_text.is_some() {
                let where_key = NodeId::new(format!("__where__{}", node_id));
                if let Some(where_expr) = self.compiled_cache.get(&where_key) {
                    let at = At {
                        node_id: Some(node_id.as_str()),
                        period: *period_id,
                    };
                    let mask = evaluate_value(where_expr, context, Some(node_id.as_str()))?;
                    let masked = match value::control(mask, "where clause", at)? {
                        Some(true) => None,
                        Some(false) => Some(Cell::Masked),
                        None => Some(Cell::Unavailable(UnavailableReason::MissingMask)),
                    };
                    if let Some(cell) = masked {
                        context.set_cell(node_id.as_str(), cell)?;
                        continue;
                    }
                }
            }
            let hidden = visibility_cutoff.is_some_and(|cutoff| {
                node_spec
                    .explicit_value_available_on(period)
                    .is_some_and(|available_on| available_on > cutoff)
            });
            // A forecast is eligible only in a forecast period: an actual
            // hidden by the cutoff is not forecast.
            let source = resolve_node_value(node_spec, period_id, is_actual, hidden)?;
            let cell_source = match source {
                NodeValueSource::Value(_) => Some(CellSource::Value),
                NodeValueSource::Forecast => Some(CellSource::Forecast),
                NodeValueSource::Formula => Some(CellSource::Formula),
                NodeValueSource::Missing | NodeValueSource::Hidden => None,
            };
            let value_type = self.node_value_types.get(node_id).copied();
            let cell = {
                let mut mc_z_wrapper: Option<&mut McZCache> = mc_z_cache.as_deref_mut();
                match source {
                    NodeValueSource::Value(observation) => Ok(match observation {
                        AmountOrScalar::Amount(money) => Cell::Money(money),
                        AmountOrScalar::Scalar(value) => Cell::Scalar(value),
                    }),
                    NodeValueSource::Missing => Ok(Cell::Missing),
                    NodeValueSource::Hidden => {
                        Ok(Cell::Unavailable(UnavailableReason::HiddenByCutoff))
                    }
                    NodeValueSource::Forecast => forecast_eval::evaluate_forecast(
                        node_spec,
                        eval,
                        context,
                        &mut self.forecast_cache,
                        mc_path,
                        &mut mc_z_wrapper,
                    )
                    .and_then(|forecast| match forecast {
                        forecast_eval::ForecastCell::Unavailable(reason) => {
                            Ok(Cell::Unavailable(reason))
                        }
                        forecast_eval::ForecastCell::Value {
                            value,
                            clipped_from,
                        } => {
                            if let Some(raw) = clipped_from {
                                context.warnings.push(EvalWarning::ForecastClipped {
                                    node_id: node_id.as_str().to_string(),
                                    period: *period_id,
                                    raw,
                                    published: value,
                                });
                            }
                            computed_cell(value, value_type, node_id, period_id)
                        }
                    }),
                    NodeValueSource::Formula => {
                        let expr = self.compiled_cache.get(node_id).ok_or_else(|| {
                            Error::eval(format!("No compiled formula for node '{}'", node_id))
                        })?;
                        let at = At {
                            node_id: Some(node_id.as_str()),
                            period: *period_id,
                        };
                        evaluate_value(expr, context, Some(node_id.as_str()))
                            .and_then(|result| value::into_cell(result, value_type, at))
                    }
                }
            }
            .map_err(|e| {
                tracing::error!(
                    node_id = node_id.as_str(),
                    period = %period_id,
                    error = %e,
                    "node evaluation failed"
                );
                e
            })?;

            if matches!(cell, Cell::Unavailable(UnavailableReason::MissingInput)) {
                context.warnings.push(EvalWarning::NonFiniteValue {
                    node_id: node_id.as_str().to_string(),
                    period: *period_id,
                    value: f64::NAN,
                });
            }
            context.set_cell(node_id.as_str(), cell)?;
            if let Some(cell_source) = cell_source {
                context.set_source(node_id.as_str(), cell_source);
            }
        }

        Ok(())
    }

    /// Evaluate a single period.
    ///
    /// `mc` carries the Monte Carlo path identity and per-path z-score cache;
    /// `None` is the single-run path.
    fn evaluate_period(
        &mut self,
        eval: PeriodEval<'_>,
        historical: &std::sync::Arc<PeriodHistory>,
        historical_cs: &std::sync::Arc<
            IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>,
        >,
        mc: Option<(PathId, &mut McZCache)>,
    ) -> Result<PeriodEvaluation> {
        let mut context = EvaluationContext::new_with_history(
            eval.period.id,
            std::sync::Arc::clone(historical),
            std::sync::Arc::clone(historical_cs),
        );

        let (mc_path, mc_z_cache) = match mc {
            Some((path, cache)) => (Some(path), Some(cache)),
            None => (None, None),
        };
        self.evaluate_nodes_in_order(eval, &mut context, mc_path, None, mc_z_cache)?;

        let row = (
            context.current_values.clone(),
            context.current_cells.clone(),
        );
        let (results, warnings) = context.into_results();
        Ok((results, row, warnings))
    }
}

/// Type a forecast or formula result as a cell.
///
/// A result with no defined value leaves the cell unavailable. A monetary
/// node's result is converted to an exact amount here, the one boundary
/// where a floating-point calculation returns to money.
///
/// # Errors
///
/// Returns an evaluation error naming the node, period and cause when a
/// monetary result cannot be represented as `Money`.
fn computed_cell(
    value: f64,
    value_type: Option<NodeValueType>,
    node_id: &NodeId,
    period_id: &PeriodId,
) -> Result<Cell> {
    if !value.is_finite() {
        return Ok(Cell::Unavailable(UnavailableReason::MissingInput));
    }
    match value_type {
        Some(NodeValueType::Monetary { currency }) => {
            finstack_quant_core::money::Money::from_model_value(value, currency)
                .map(Cell::Money)
                .map_err(|cause| {
                    Error::eval(format!(
                        "Node '{node_id}' in period {period_id}: result {value} cannot be \
                         represented as {currency} money: {cause}"
                    ))
                })
        }
        Some(NodeValueType::Scalar) | None => Ok(Cell::Scalar(value)),
    }
}

/// The market/date pair a model's capital structure is generated from when
/// the caller supplies no market: an empty market as of the first period's
/// start, so fixed-rate debt needs no market data. `None` without a capital
/// structure or periods.
fn contractual_capital_structure<'m>(
    model: &FinancialModelSpec,
    empty_market: &'m finstack_quant_core::market_data::context::MarketContext,
) -> Option<(
    &'m finstack_quant_core::market_data::context::MarketContext,
    finstack_quant_core::dates::Date,
)> {
    model
        .capital_structure
        .as_ref()
        .zip(model.periods.first())
        .map(|(_, first)| (empty_market, first.start))
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Which explicit observations an evaluation may read.
///
/// On the wire: `"full_snapshot"`, or `{"through": "2025-04-25"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ObservationCutoff {
    /// Read every observation in the model's snapshot, with no publication
    /// cutoff.
    FullSnapshot,
    /// Read an observation only when its availability date is on or before
    /// this date, inclusive through the end of the day.
    Through(
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        finstack_quant_core::dates::Date,
    ),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ModelBuilder;
    use crate::evaluator::CellStatus;
    use crate::types::{AmountOrScalar, NodeSpec, NodeType};
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::dates::PeriodId;
    use finstack_quant_core::market_data::context::MarketContext;
    use indexmap::IndexMap;
    use time::Month;

    /// STATEMENTS-EVALUATION-001 #2: a false mask is a masked cell with no
    /// numeric value, and a mask with no value leaves the cell unavailable.
    /// Neither is a zero, here or downstream.
    #[test]
    fn masked_and_unmaskable_cells_have_no_value() {
        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let model = {
            let mut builder = ModelBuilder::new("mask-states");
            builder.periods("2025Q1..Q1", None).expect("periods");
            builder.value_scalar("flag", &[]);
            builder.value_scalar("off", &[(q1, 0.0)]);
            builder.compute("unknown_mask", "42").expect("formula");
            builder
                .where_clause("unknown_mask", "flag > 0")
                .expect("predicate");
            builder.compute("masked", "42").expect("formula");
            builder
                .where_clause("masked", "off > 0")
                .expect("predicate");
            builder
                .compute("downstream", "masked + 1")
                .expect("formula");
            builder.compute("kept", "42").expect("formula");
            builder.where_clause("kept", "off == 0").expect("predicate");
            builder.build().expect("valid model")
        };

        let results = Evaluator::new().evaluate(&model).expect("evaluation");
        let cell = |node: &str| *results.get_cell(node, &q1).expect("cell");

        assert_eq!(cell("masked").status, CellStatus::Masked);
        assert_eq!(
            results.get("masked", &q1),
            None,
            "a masked cell is not zero"
        );
        assert_eq!(cell("masked").source, None);
        assert_eq!(
            cell("downstream").status,
            CellStatus::Unavailable,
            "a masked input is not read as zero downstream"
        );
        assert_eq!(cell("downstream").source, Some(CellSource::Formula));
        assert_eq!(
            cell("unknown_mask"),
            crate::evaluator::ResultCell::unavailable(UnavailableReason::MissingMask)
        );
        assert_eq!(results.get("kept", &q1), Some(42.0));
        assert_eq!(cell("kept").source, Some(CellSource::Formula));
        assert_eq!(cell("flag").status, CellStatus::Missing);
        assert_eq!(
            cell("off"),
            crate::evaluator::ResultCell::scalar(0.0)
                .expect("finite")
                .with_source(Some(CellSource::Value)),
            "a known zero is a value"
        );
    }

    #[test]
    fn evaluate_prepared_rejects_a_model_whose_formula_changed() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let build = |factor: &str| {
            let mut model_builder_1 = ModelBuilder::new("prepared-drift");
            model_builder_1
                .periods("2025Q1..Q1", None)
                .expect("valid periods");
            model_builder_1.value("revenue", &[(period, AmountOrScalar::scalar(100.0))]);
            model_builder_1
                .compute("scaled", format!("revenue * {factor}"))
                .expect("valid formula");
            model_builder_1.build().expect("valid model")
        };

        let original = build("2");
        let mut evaluator = Evaluator::new();
        let prepared = evaluator.prepare(&original).expect("prepare");

        // Same nodes, different formula: the node-set guard cannot see this.
        let edited = build("3");
        let err = evaluator
            .evaluate_prepared(&edited, &prepared)
            .expect_err("a changed formula must be rejected, not silently ignored");
        assert!(
            err.to_string().contains("formula"),
            "error should point at the changed formula: {err}"
        );

        // The unedited model still evaluates against its own plan.
        let results = evaluator
            .evaluate_prepared(&original, &prepared)
            .expect("the model the plan was built for still evaluates");
        assert_eq!(results.get("scaled", &period), Some(200.0));
    }

    /// One node resolved by a different layer in each period, plus a masked
    /// node: the result records which layer produced every cell.
    #[test]
    fn node_sources_record_value_formula_forecast_and_where_mask() {
        use crate::types::ForecastSpec;

        let q = |quarter| PeriodId::quarter(2025, quarter).expect("valid period fixture");
        // Q1 and Q2 are actuals, Q3 is a forecast period.
        let periods = finstack_quant_core::dates::build_periods("2025Q1..Q3", Some("2025Q2"))
            .expect("periods")
            .periods;
        let mut model = FinancialModelSpec::new(
            "cell-sources",
            finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                periods,
            )
            .expect("valid period plan"),
        );
        model.add_node(
            NodeSpec::new("revenue", NodeType::Mixed)
                .with_values(IndexMap::from([(q(1), AmountOrScalar::scalar(100.0))]))
                .with_forecast(ForecastSpec::growth(0.10))
                .with_formula("123"),
        );
        let mut masked = NodeSpec::new("masked", NodeType::Calculated).with_formula("42");
        masked.where_text = Some("1 == 0".to_string());
        model.add_node(masked);

        let results = Evaluator::new().evaluate(&model).expect("evaluation");

        // Explicit value wins in Q1; Q2 is an actual with no value, so the
        // formula fires (forecasts never run in actuals); Q3 is forecast.
        assert_eq!(results.get("revenue", &q(1)), Some(100.0));
        assert_eq!(
            results.get_source("revenue", &q(1)),
            Some(CellSource::Value)
        );
        assert_eq!(results.get("revenue", &q(2)), Some(123.0));
        assert_eq!(
            results.get_source("revenue", &q(2)),
            Some(CellSource::Formula)
        );
        assert_eq!(
            results.get_source("revenue", &q(3)),
            Some(CellSource::Forecast)
        );
        for quarter in 1..=3 {
            let cell = results.get_cell("masked", &q(quarter)).expect("cell");
            assert_eq!(cell.status, CellStatus::Masked);
            assert_eq!(cell.source, None);
        }

        // Every node has a cell in every period, and each value cell records
        // the source that produced it.
        for cells in results.cells.values() {
            assert_eq!(cells.len(), 3);
            for cell in cells.values() {
                assert_eq!(cell.status == CellStatus::Value, cell.source.is_some());
            }
        }

        // Status and source survive the wire format and reach the long table.
        let json = serde_json::to_value(&results).expect("serialize");
        assert_eq!(json["cells"]["revenue"]["2025Q3"]["source"], "forecast");
        assert_eq!(json["cells"]["revenue"]["2025Q1"]["value"], 100.0);
        assert_eq!(
            json["cells"]["masked"]["2025Q1"],
            serde_json::json!({ "status": "masked" })
        );
        let back: StatementResult = serde_json::from_value(json).expect("deserialize");
        assert_eq!(back.cells, results.cells);

        let table = results.to_table_long().expect("long table");
        let labels = |name: &str| match &table.column(name).expect("column").data {
            finstack_quant_core::table::TableColumnData::NullableString(labels) => labels.clone(),
            finstack_quant_core::table::TableColumnData::String(labels) => {
                labels.iter().cloned().map(Some).collect()
            }
            other => panic!("{name} must be a string column: {other:?}"),
        };
        let expected_sources: Vec<Option<String>> = results
            .cells
            .values()
            .flat_map(|cells| cells.values())
            .map(|cell| cell.source.map(|source| source.as_str().to_string()))
            .collect();
        assert_eq!(labels("source"), expected_sources);
        assert!(labels("source").contains(&Some("forecast".to_string())));
        assert!(labels("status").contains(&Some("masked".to_string())));
    }

    /// The wire form of a cell is closed: a status must agree with its
    /// value and reason.
    #[test]
    fn result_cells_reject_inconsistent_wire_forms() {
        for invalid in [
            serde_json::json!({ "status": "value" }),
            serde_json::json!({ "status": "masked", "value": 1.0 }),
            serde_json::json!({ "status": "unavailable" }),
            serde_json::json!({ "status": "missing", "reason": "missing_mask" }),
            serde_json::json!({ "status": "value", "value": 1.0, "extra": true }),
        ] {
            serde_json::from_value::<crate::evaluator::ResultCell>(invalid.clone())
                .expect_err(&invalid.to_string());
        }
        let money = serde_json::json!({
            "status": "value",
            "value": { "amount": "125.01", "currency": "USD" },
            "source": "formula"
        });
        let cell: crate::evaluator::ResultCell =
            serde_json::from_value(money.clone()).expect("money cell");
        assert_eq!(serde_json::to_value(cell).expect("serialize"), money);
    }

    #[test]
    fn as_of_hides_a_dated_observation_outside_the_actuals_range() {
        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let august = Date::from_calendar_date(2025, Month::August, 1).expect("date");
        // No period is flagged actual.
        let periods = finstack_quant_core::dates::build_periods("2025Q1..Q2", None)
            .expect("periods")
            .periods;
        let evaluate = |q2_available_on: Option<Date>| {
            let mut model = FinancialModelSpec::new(
                "as-of-policy",
                finstack_quant_core::dates::PeriodPlan::try_new(
                    finstack_quant_core::dates::PeriodCalendar::Gregorian,
                    periods.clone(),
                )
                .expect("valid period plan"),
            );
            let mut availability = IndexMap::from([(q1, as_of)]);
            if let Some(date) = q2_available_on {
                availability.insert(q2, date);
            }
            model.add_node(
                NodeSpec::new("revenue", NodeType::Mixed)
                    .with_values(IndexMap::from([
                        (q1, AmountOrScalar::scalar(100.0)),
                        (q2, AmountOrScalar::scalar(999.0)),
                    ]))
                    .with_availability_dates(availability)
                    .with_formula("123"),
            );
            model.source_snapshot = Some("original".to_string());
            Evaluator::new()
                .evaluate_with_market(
                    &model,
                    &MarketContext::new(),
                    as_of,
                    crate::evaluator::ObservationCutoff::Through(as_of),
                )
                .expect("evaluation")
                .get("revenue", &q2)
        };
        // Dated after the cutoff: hidden, so the formula answers.
        assert_eq!(evaluate(Some(august)), Some(123.0));
        // Undated value in a forecast period is an assumption and stays visible.
        assert_eq!(evaluate(None), Some(999.0));
    }

    #[test]
    fn as_of_hides_future_actual_values() {
        let periods = finstack_quant_core::dates::build_periods("2025Q1..Q2", Some("2025Q2"))
            .expect("periods")
            .periods;
        let mut model = FinancialModelSpec::new(
            "as-of-policy",
            finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                periods,
            )
            .expect("valid period plan"),
        );
        let mut values = IndexMap::new();
        values.insert(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            AmountOrScalar::scalar(100.0),
        );
        values.insert(
            PeriodId::quarter(2025, 2).expect("valid period fixture"),
            AmountOrScalar::scalar(999.0),
        );
        model.add_node(
            NodeSpec::new("revenue", NodeType::Mixed)
                .with_values(values)
                .with_availability_dates(IndexMap::from([
                    (
                        PeriodId::quarter(2025, 1).expect("valid period fixture"),
                        Date::from_calendar_date(2025, Month::January, 1).expect("date"),
                    ),
                    (
                        PeriodId::quarter(2025, 2).expect("valid period fixture"),
                        Date::from_calendar_date(2025, Month::July, 1).expect("date"),
                    ),
                ]))
                .with_formula("123"),
        );
        model.source_snapshot = Some("original".to_string());

        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let mut evaluator = Evaluator::new();
        let results = evaluator
            .evaluate_with_market(
                &model,
                &MarketContext::new(),
                as_of,
                crate::evaluator::ObservationCutoff::Through(as_of),
            )
            .expect("evaluation");

        assert_eq!(
            results.get(
                "revenue",
                &PeriodId::quarter(2025, 1).expect("valid period fixture")
            ),
            Some(100.0)
        );
        assert_eq!(
            results.get(
                "revenue",
                &PeriodId::quarter(2025, 2).expect("valid period fixture")
            ),
            Some(123.0)
        );
        // The Q2 value exists in the spec but is hidden by the as-of cutoff,
        // so the recorded source is the formula that actually fired.
        assert_eq!(
            results.get_source(
                "revenue",
                &PeriodId::quarter(2025, 1).expect("valid period fixture")
            ),
            Some(CellSource::Value)
        );
        assert_eq!(
            results.get_source(
                "revenue",
                &PeriodId::quarter(2025, 2).expect("valid period fixture")
            ),
            Some(CellSource::Formula)
        );
    }
}
