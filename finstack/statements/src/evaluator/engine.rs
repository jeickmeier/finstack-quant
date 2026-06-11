//! Main evaluator implementation.

use crate::dsl;
use crate::error::{Error, Result};
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::dag::{evaluate_order, DependencyGraph};
use crate::evaluator::forecast_eval;
use crate::evaluator::formula::evaluate_formula;
use crate::evaluator::formula_helpers::is_truthy;
use crate::evaluator::monte_carlo::{
    MonteCarloAccumulator, MonteCarloConfig, MonteCarloResults, PathResult,
};
use crate::evaluator::precedence::{resolve_node_value_with_policy, NodeValueSource};
use crate::evaluator::results::{EvalWarning, ResultsMeta, StatementResult};
use crate::evaluator::{capital_structure_runtime, capital_structure_runtime::dependent_closure};
use crate::types::{FinancialModelSpec, NodeId};
use finstack_core::dates::PeriodId;
use finstack_core::expr::Expr;
use indexmap::IndexMap;
use std::collections::HashSet;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

/// Cached structural analysis from [`Evaluator::prepare`], allowing repeated
/// evaluations of the same model structure without recompilation.
pub struct PreparedEvaluation {
    /// Topologically sorted node evaluation order.
    eval_order: Vec<NodeId>,
    /// Map from node id to its column index in the evaluation order.
    node_to_column: std::sync::Arc<IndexMap<NodeId, usize>>,
    /// Dependency graph retained for downstream consumers such as capital structure affected-node computation.
    dag: DependencyGraph,
    /// Compiled expressions snapshotted at prepare time so prepared
    /// evaluations are independent of the evaluator's mutable compiled cache
    /// (which other `evaluate` calls may overwrite).
    compiled_cache: std::sync::Arc<IndexMap<NodeId, Expr>>,
}

/// Evaluator for financial models.
///
/// The evaluator compiles formulas, resolves dependencies, and evaluates
/// nodes period-by-period according to precedence rules.
///
/// # Example
///
/// ```rust
/// # use finstack_statements::builder::ModelBuilder;
/// # use finstack_statements::evaluator::Evaluator;
/// # use finstack_core::dates::PeriodId;
/// let model = ModelBuilder::new("demo")
///     .periods("2025Q1..Q2", None)?
///     .value("revenue", &[
///         (PeriodId::quarter(2025, 1), 100_000.0.into()),
///         (PeriodId::quarter(2025, 2), 105_000.0.into()),
///     ])
///     .compute("gross_profit", "revenue * 0.6")?
///     .build()?;
///
/// let mut evaluator = Evaluator::new();
/// let results = evaluator.evaluate(&model)?;
/// assert_eq!(results.get("revenue", &PeriodId::quarter(2025, 1)), Some(100_000.0));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct Evaluator {
    /// Cached compiled expressions (Arc-shared across Monte Carlo path clones)
    compiled_cache: std::sync::Arc<IndexMap<NodeId, Expr>>,

    /// Cached forecast results: node_id → (period_id → value)
    forecast_cache: IndexMap<NodeId, IndexMap<PeriodId, f64>>,

    /// Optional check suite run after evaluation to produce inline validation
    check_suite: Option<std::sync::Arc<crate::checks::CheckSuite>>,

    /// As-of visibility cutoff for the current evaluation run. Actual periods
    /// starting after this date have hidden explicit values and are treated as
    /// forecast periods by forecast evaluation.
    visibility_cutoff: Option<finstack_core::dates::Date>,
}

impl Evaluator {
    /// Create a new evaluator.
    #[must_use = "creating an evaluator has no effect without calling evaluate()"]
    pub fn new() -> Self {
        Self {
            compiled_cache: std::sync::Arc::new(IndexMap::new()),
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
    /// Use this method when the model contains capital-structure `cs.*` references
    /// and you need instrument pricing. For models without capital structure, call
    /// [`Evaluator::evaluate`] instead.
    ///
    /// # As-of visibility
    ///
    /// The `as_of` date is also the explicit-value visibility cutoff. Explicit
    /// values on actual periods are visible only when `period.start <= as_of`.
    /// Actual periods that start after `as_of` keep their place in the model
    /// timeline, but their explicit values are hidden and the evaluator resolves
    /// the node through forecast or formula fallbacks using the normal precedence
    /// rules. This avoids leaking future actuals while preserving the current
    /// actual period when the as-of date is on or after that period's start.
    ///
    /// `actuals_until` still controls which periods are classified as actuals.
    /// `as_of` only controls whether explicit values in those actual periods are
    /// visible during this evaluation run.
    ///
    /// # Arguments
    ///
    /// * `model` - The financial model specification
    /// * `market_ctx` - Market context for pricing instruments
    /// * `as_of` - Valuation date for pricing and explicit-value visibility
    ///
    /// # Returns
    ///
    /// Returns `StatementResult` containing the evaluated values for all nodes and periods.
    pub fn evaluate_with_market(
        &mut self,
        model: &FinancialModelSpec,
        market_ctx: &finstack_core::market_data::context::MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> Result<StatementResult> {
        self.evaluate_inner(model, Some(market_ctx), Some(as_of))
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
    fn evaluate_inner(
        &mut self,
        model: &FinancialModelSpec,
        market_ctx: Option<&finstack_core::market_data::context::MarketContext>,
        as_of: Option<finstack_core::dates::Date>,
    ) -> Result<StatementResult> {
        let _span = tracing::info_span!(
            "statements.evaluate",
            model_id = model.id.as_str(),
            periods = model.periods.len(),
            nodes = model.nodes.len(),
            has_market = market_ctx.is_some(),
            has_as_of = as_of.is_some()
        )
        .entered();
        #[cfg(not(target_arch = "wasm32"))]
        let start = Instant::now();

        let prepared = self.init_eval_plan(model)?;
        self.visibility_cutoff = as_of;

        let cs_seed_nodes: HashSet<NodeId> = model
            .nodes
            .iter()
            .filter_map(|(node_id, spec)| {
                if spec
                    .formula_text
                    .as_deref()
                    .is_some_and(|text| text.contains("cs."))
                    || spec
                        .where_text
                        .as_deref()
                        .is_some_and(|text| text.contains("cs."))
                {
                    Some(node_id.clone())
                } else {
                    None
                }
            })
            .collect();

        let cs_affected_nodes = dependent_closure(&prepared.dag, &cs_seed_nodes);

        // Initialize capital structure state for dynamic evaluation
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
                let opening_balance = capital_structure_runtime::resolve_opening_balance(
                    instrument.as_ref(),
                    market_ctx,
                    as_of_date,
                    first_period.start,
                )?;

                state
                    .opening_balances
                    .insert(instrument_id.clone(), opening_balance);
            }
        }

        // Evaluate period-by-period
        let mut historical: std::sync::Arc<IndexMap<PeriodId, IndexMap<String, f64>>> =
            std::sync::Arc::new(IndexMap::new());
        let mut historical_cs: std::sync::Arc<
            IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>,
        > = std::sync::Arc::new(IndexMap::new());
        let mut all_warnings = Vec::new();
        let mut results = StatementResult::new();

        // Accumulator for capital structure cashflows across all periods
        let mut cs_cashflows_accum = crate::capital_structure::CapitalStructureCashflows::new();
        let mut has_cs = false;

        // Sequential evaluation for all models
        for period in &model.periods {
            let explicit_values_visible =
                !period.is_actual || as_of.is_none_or(|as_of_date| period.start <= as_of_date);
            let is_actual_for_eval = period.is_actual && explicit_values_visible;
            let (period_results, period_warnings) =
                if let (Some(market_ctx), Some(as_of), Some(ref mut state), Some(insts)) =
                    (market_ctx, as_of, cs_state.as_mut(), instruments.as_ref())
                {
                    let (vals, warns, period_cs) = self.evaluate_period_dynamic(
                        model,
                        period,
                        is_actual_for_eval,
                        explicit_values_visible,
                        &prepared.eval_order,
                        &prepared.node_to_column,
                        &historical,
                        &historical_cs,
                        market_ctx,
                        as_of,
                        insts,
                        state,
                        &cs_affected_nodes,
                    )?;

                    cs_cashflows_accum.set_period(period_cs);
                    has_cs = true;

                    (vals, warns)
                } else {
                    self.evaluate_period(
                        model,
                        &period.id,
                        is_actual_for_eval,
                        explicit_values_visible,
                        &prepared.eval_order,
                        &prepared.node_to_column,
                        &historical,
                        &historical_cs,
                    )?
                };

            all_warnings.extend(period_warnings.into_iter());

            // Store in results
            for (node_id, value) in &period_results {
                results
                    .nodes
                    .entry(node_id.to_owned())
                    .or_default()
                    .insert(period.id, *value);
            }

            // Add to historical context for next period (move, not clone)
            std::sync::Arc::make_mut(&mut historical).insert(period.id, period_results);
            if has_cs {
                let period_snapshot = build_cs_period_snapshot(&cs_cashflows_accum, period.id);
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

        self.finalize_results(
            &mut results,
            model,
            ResultsMeta {
                #[cfg(not(target_arch = "wasm32"))]
                eval_time_ms: Some(start.elapsed().as_millis() as u64),
                #[cfg(target_arch = "wasm32")]
                eval_time_ms: None,
                num_nodes: model.nodes.len(),
                num_periods: model.periods.len(),
                numeric_mode: crate::evaluator::NumericMode::Float64,
                rounding_context: None,
                parallel: false,
                warnings: all_warnings,
            },
        )?;

        Ok(results)
    }

    /// Evaluate a financial model over all periods without market context.
    ///
    /// Use this for models that do not reference capital structure (`cs.*`).
    /// For capital-structure aware evaluation, use
    /// [`Evaluator::evaluate_with_market`] and provide market data.
    ///
    /// # Arguments
    ///
    /// * `model` - The financial model specification
    /// # Returns
    ///
    /// Returns `StatementResult` containing the evaluated values for all nodes and periods.
    pub fn evaluate(&mut self, model: &FinancialModelSpec) -> Result<StatementResult> {
        self.evaluate_inner(model, None, None).inspect_err(|err| {
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
    /// input values.
    pub fn prepare(&mut self, model: &FinancialModelSpec) -> Result<PreparedEvaluation> {
        self.init_eval_plan(model)
    }

    /// Evaluate a model reusing previously compiled formulas and evaluation order.
    ///
    /// The model must have the same structure (same nodes and formulas) as the one
    /// passed to [`prepare`](Self::prepare); only node *values* may differ. This
    /// avoids formula recompilation, DAG construction, and cycle detection on every
    /// call — typically saving 30-50 % of total evaluation time.
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
        self.forecast_cache.clear();
        self.visibility_cutoff = None;
        // Use the compiled expressions captured at prepare time, not whatever
        // the evaluator's mutable cache currently holds (a later `evaluate` of
        // a different model sharing node ids would otherwise leak its formulas
        // into this prepared evaluation).
        self.compiled_cache = std::sync::Arc::clone(&prepared.compiled_cache);

        let mut historical: std::sync::Arc<IndexMap<PeriodId, IndexMap<String, f64>>> =
            std::sync::Arc::new(IndexMap::new());
        let historical_cs: std::sync::Arc<
            IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>,
        > = std::sync::Arc::new(IndexMap::new());
        let mut all_warnings = Vec::new();
        let mut results = StatementResult::new();

        for period in &model.periods {
            let (period_results, period_warnings) = self.evaluate_period(
                model,
                &period.id,
                period.is_actual,
                true,
                &prepared.eval_order,
                &prepared.node_to_column,
                &historical,
                &historical_cs,
            )?;
            all_warnings.extend(period_warnings);
            for (node_id, value) in &period_results {
                results
                    .nodes
                    .entry(node_id.to_owned())
                    .or_default()
                    .insert(period.id, *value);
            }
            std::sync::Arc::make_mut(&mut historical).insert(period.id, period_results);
        }

        self.finalize_results(
            &mut results,
            model,
            ResultsMeta {
                eval_time_ms: None,
                num_nodes: model.nodes.len(),
                num_periods: model.periods.len(),
                numeric_mode: crate::evaluator::NumericMode::Float64,
                rounding_context: None,
                parallel: false,
                warnings: all_warnings,
            },
        )?;

        Ok(results)
    }

    /// Evaluate a financial model in Monte Carlo mode.
    ///
    /// This method replays the model `n_paths` times with independent, but
    /// deterministic, seeds for stochastic forecast methods and aggregates
    /// the resulting distribution into percentile bands.
    ///
    /// Monte Carlo evaluation currently focuses on statement forecasts and
    /// does not support capital structure (`capital_structure`) integration.
    pub fn evaluate_monte_carlo(
        &mut self,
        model: &FinancialModelSpec,
        config: &MonteCarloConfig,
    ) -> Result<MonteCarloResults> {
        self.evaluate_monte_carlo_inner(model, config)
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
    ) -> Result<MonteCarloResults> {
        let _span = tracing::info_span!(
            "statements.evaluate_monte_carlo",
            model_id = model.id.as_str(),
            paths = config.n_paths,
            periods = model.periods.len(),
            nodes = model.nodes.len()
        )
        .entered();
        if config.n_paths == 0 {
            return Err(Error::eval(
                "Monte Carlo configuration requires n_paths > 0",
            ));
        }

        if model.capital_structure.is_some() {
            return Err(Error::eval(
                "Monte Carlo evaluation for statements does not yet support capital_structure. \
                 Run Monte Carlo on the underlying instruments using finstack-valuations \
                 for capital structure analysis.",
            ));
        }

        let prepared = self.init_eval_plan(model)?;

        // Run paths — parallel via rayon on native targets, serial on wasm32
        // (rayon's thread pool is unavailable in single-threaded wasm).
        // Both paths are deterministic for a given seed.
        let run_single_path = |path_idx: usize| -> Result<PathResult> {
            let mut path_eval = self.clone();
            path_eval.forecast_cache.clear();

            let seed_offset = config
                .seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(path_idx as u64);
            let mut mc_z_cache: IndexMap<NodeId, IndexMap<PeriodId, f64>> = IndexMap::new();
            let mut historical: std::sync::Arc<IndexMap<PeriodId, IndexMap<String, f64>>> =
                std::sync::Arc::new(IndexMap::new());
            let mut all_warnings = Vec::new();

            for period in &model.periods {
                let (period_results, warnings) = path_eval.evaluate_period_mc(
                    model,
                    &period.id,
                    period.is_actual,
                    &prepared.eval_order,
                    &prepared.node_to_column,
                    &historical,
                    seed_offset,
                    &mut mc_z_cache,
                )?;
                all_warnings.extend(warnings);
                std::sync::Arc::make_mut(&mut historical).insert(period.id, period_results);
            }

            let mut node_map: IndexMap<String, IndexMap<PeriodId, f64>> = IndexMap::new();
            for (period_id, values) in historical.iter() {
                for (node_id, value) in values {
                    node_map
                        .entry(node_id.clone())
                        .or_default()
                        .insert(*period_id, *value);
                }
            }
            Ok((node_map, all_warnings))
        };

        let accumulator_seed = MonteCarloAccumulator::new(model, config)?;

        #[cfg(not(target_arch = "wasm32"))]
        let accumulator = {
            use rayon::prelude::*;
            (0..config.n_paths)
                .into_par_iter()
                .try_fold(
                    || accumulator_seed.empty_like(),
                    |mut acc, path_idx| {
                        let (path_results, warnings) = run_single_path(path_idx)?;
                        acc.push_path(path_idx, path_results, warnings)?;
                        Ok(acc)
                    },
                )
                .try_reduce(
                    || accumulator_seed.empty_like(),
                    |left, right| left.merge(right),
                )?
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

    /// Reset caches, build the dependency graph, compile formulas, and derive
    /// the topological evaluation order. Shared by [`evaluate_inner`](Self::evaluate_inner),
    /// [`prepare`](Self::prepare), and [`evaluate_monte_carlo`](Self::evaluate_monte_carlo).
    fn init_eval_plan(&mut self, model: &FinancialModelSpec) -> Result<PreparedEvaluation> {
        self.compiled_cache = std::sync::Arc::new(IndexMap::new());
        self.forecast_cache.clear();
        self.visibility_cutoff = None;
        let dag = DependencyGraph::from_model(model)?;
        dag.detect_cycles()?;
        let eval_order = evaluate_order(&dag)?;
        self.compile_formulas(model)?;
        let node_to_column = std::sync::Arc::new(
            eval_order
                .iter()
                .enumerate()
                .map(|(i, node_id)| (node_id.clone(), i))
                .collect(),
        );
        Ok(PreparedEvaluation {
            eval_order,
            node_to_column,
            dag,
            compiled_cache: std::sync::Arc::clone(&self.compiled_cache),
        })
    }

    /// Populate value types, set metadata, and run the optional check suite.
    fn finalize_results(
        &self,
        results: &mut StatementResult,
        model: &FinancialModelSpec,
        meta: ResultsMeta,
    ) -> Result<()> {
        results.populate_value_types(model)?;
        results.meta = meta;
        if let Some(suite) = &self.check_suite {
            results.check_report = Some(suite.run(model, results)?);
        }
        Ok(())
    }

    /// Compile all formulas in the model.
    fn compile_formulas(&mut self, model: &FinancialModelSpec) -> Result<()> {
        let cache = std::sync::Arc::make_mut(&mut self.compiled_cache);
        for (node_id, node_spec) in &model.nodes {
            compile_text_into_cache(cache, node_id.clone(), &node_spec.formula_text)?;
            let where_key = NodeId::new(format!("__where__{}", node_id));
            compile_text_into_cache(cache, where_key, &node_spec.where_text)?;
        }
        Ok(())
    }

    /// Evaluate nodes in topological order within a context.
    ///
    /// This is the shared inner loop used by all period evaluation methods.
    /// It handles where-clause masking, precedence resolution, forecast/formula
    /// dispatch, and storing results in the context.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn evaluate_nodes_in_order(
        &mut self,
        model: &FinancialModelSpec,
        period_id: &PeriodId,
        is_actual: bool,
        explicit_values_visible: bool,
        eval_order: &[NodeId],
        context: &mut EvaluationContext,
        seed_offset: Option<u64>,
        node_filter: Option<&HashSet<NodeId>>,
        mut mc_z_cache: Option<&mut IndexMap<NodeId, IndexMap<PeriodId, f64>>>,
    ) -> Result<()> {
        let visibility_cutoff = self.visibility_cutoff;
        for node_id in eval_order {
            if let Some(filter) = node_filter {
                if !filter.contains(node_id.as_str()) {
                    continue;
                }
            }

            let node_spec = model
                .get_node(node_id.as_str())
                .ok_or_else(|| Error::eval(format!("Node '{}' not found in model", node_id)))?;

            if node_spec.where_text.is_some() {
                let where_key = NodeId::new(format!("__where__{}", node_id));
                if let Some(where_expr) = self.compiled_cache.get(&where_key) {
                    let where_result =
                        evaluate_formula(where_expr, context, Some(node_id.as_str()))?;
                    if !is_truthy(where_result) {
                        context.set_value(node_id.as_str(), 0.0)?;
                        continue;
                    }
                }
            }

            let value = {
                let source = resolve_node_value_with_policy(
                    node_spec,
                    period_id,
                    is_actual,
                    explicit_values_visible,
                )?;
                let mut mc_z_wrapper: Option<&mut IndexMap<NodeId, IndexMap<PeriodId, f64>>> =
                    mc_z_cache.as_deref_mut();
                match source {
                    NodeValueSource::Value(v) => Ok(v),
                    NodeValueSource::Forecast => forecast_eval::evaluate_forecast(
                        node_spec,
                        model,
                        period_id,
                        context,
                        &mut self.forecast_cache,
                        seed_offset,
                        &mut mc_z_wrapper,
                        visibility_cutoff,
                    ),
                    NodeValueSource::Formula => {
                        let expr = self.compiled_cache.get(node_id).ok_or_else(|| {
                            Error::eval(format!("No compiled formula for node '{}'", node_id))
                        })?;
                        evaluate_formula(expr, context, Some(node_id.as_str()))
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

            context.set_value(node_id.as_str(), value)?;
        }

        Ok(())
    }

    /// Evaluate a single period.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_period(
        &mut self,
        model: &FinancialModelSpec,
        period_id: &PeriodId,
        is_actual: bool,
        explicit_values_visible: bool,
        eval_order: &[NodeId],
        node_to_column: &std::sync::Arc<IndexMap<NodeId, usize>>,
        historical: &std::sync::Arc<IndexMap<PeriodId, IndexMap<String, f64>>>,
        historical_cs: &std::sync::Arc<
            IndexMap<PeriodId, crate::capital_structure::CapitalStructureCashflows>,
        >,
    ) -> Result<(IndexMap<String, f64>, Vec<EvalWarning>)> {
        let mut context = EvaluationContext::new_with_history(
            *period_id,
            std::sync::Arc::clone(node_to_column),
            std::sync::Arc::clone(historical),
            std::sync::Arc::clone(historical_cs),
        );

        self.evaluate_nodes_in_order(
            model,
            period_id,
            is_actual,
            explicit_values_visible,
            eval_order,
            &mut context,
            None,
            None,
            None,
        )?;

        Ok(context.into_results())
    }

    /// Evaluate a single period for Monte Carlo paths.
    ///
    /// Identical to [`evaluate_period`] but passes a seed offset to perturb
    /// stochastic forecast methods.
    #[allow(clippy::too_many_arguments)]
    fn evaluate_period_mc(
        &mut self,
        model: &FinancialModelSpec,
        period_id: &PeriodId,
        is_actual: bool,
        eval_order: &[NodeId],
        node_to_column: &std::sync::Arc<IndexMap<NodeId, usize>>,
        historical: &std::sync::Arc<IndexMap<PeriodId, IndexMap<String, f64>>>,
        seed_offset: u64,
        mc_z_cache: &mut IndexMap<NodeId, IndexMap<PeriodId, f64>>,
    ) -> Result<(IndexMap<String, f64>, Vec<EvalWarning>)> {
        let mut context = EvaluationContext::new(
            *period_id,
            std::sync::Arc::clone(node_to_column),
            std::sync::Arc::clone(historical),
        );

        self.evaluate_nodes_in_order(
            model,
            period_id,
            is_actual,
            true,
            eval_order,
            &mut context,
            Some(seed_offset),
            None,
            Some(mc_z_cache),
        )?;

        Ok(context.into_results())
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a per-period snapshot of `accum` containing only the entries for `period_id`.
///
/// Called after each period in `evaluate_inner` to populate `historical_cs` without
/// carrying O(P²×I) data — only the current period's slice is stored per historical entry.
fn build_cs_period_snapshot(
    accum: &crate::capital_structure::CapitalStructureCashflows,
    period_id: PeriodId,
) -> crate::capital_structure::CapitalStructureCashflows {
    let mut snapshot = crate::capital_structure::CapitalStructureCashflows::new();
    for (inst_id, period_map) in &accum.by_instrument {
        if let Some(breakdown) = period_map.get(&period_id) {
            snapshot
                .by_instrument
                .entry(inst_id.clone())
                .or_default()
                .insert(period_id, breakdown.clone());
        }
    }
    if let Some(breakdown) = accum.totals.get(&period_id) {
        snapshot.totals.insert(period_id, breakdown.clone());
    }
    for (currency, period_map) in &accum.totals_by_currency {
        if let Some(breakdown) = period_map.get(&period_id) {
            snapshot
                .totals_by_currency
                .entry(*currency)
                .or_default()
                .insert(period_id, breakdown.clone());
        }
    }
    snapshot.reporting_currency = accum.reporting_currency;
    snapshot
}

fn compile_text_into_cache(
    cache: &mut IndexMap<NodeId, Expr>,
    key: NodeId,
    text: &Option<String>,
) -> Result<()> {
    if let Some(t) = text {
        if !cache.contains_key(&key) {
            let expr = dsl::parse_and_compile(t)?;
            cache.insert(key, expr);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::ModelBuilder;
    use crate::types::{AmountOrScalar, NodeSpec, NodeType};
    use finstack_core::dates::Date;
    use finstack_core::dates::PeriodId;
    use finstack_core::market_data::context::MarketContext;
    use indexmap::IndexMap;
    use time::Month;

    #[test]
    fn where_clause_treats_nan_as_false() {
        let model = ModelBuilder::new("nan-where")
            .periods("2025Q1..Q1", None)
            .expect("valid period range")
            .compute("guarded_metric", "42")
            .expect("valid formula")
            .where_clause("0 / 0")
            .build()
            .expect("valid model");

        let mut evaluator = Evaluator::new();
        let results = evaluator
            .evaluate(&model)
            .expect("evaluation should succeed");

        assert_eq!(
            results.get("guarded_metric", &PeriodId::quarter(2025, 1)),
            Some(0.0)
        );
    }

    #[test]
    fn as_of_hides_future_actual_values() {
        let periods = finstack_core::dates::build_periods("2025Q1..Q2", Some("2025Q2"))
            .expect("periods")
            .periods;
        let mut model = FinancialModelSpec::new("as-of-policy", periods);
        let mut values = IndexMap::new();
        values.insert(PeriodId::quarter(2025, 1), AmountOrScalar::scalar(100.0));
        values.insert(PeriodId::quarter(2025, 2), AmountOrScalar::scalar(999.0));
        model.add_node(
            NodeSpec::new("revenue", NodeType::Mixed)
                .with_values(values)
                .with_formula("123"),
        );

        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let mut evaluator = Evaluator::new();
        let results = evaluator
            .evaluate_with_market(&model, &MarketContext::new(), as_of)
            .expect("evaluation");

        assert_eq!(
            results.get("revenue", &PeriodId::quarter(2025, 1)),
            Some(100.0)
        );
        assert_eq!(
            results.get("revenue", &PeriodId::quarter(2025, 2)),
            Some(123.0)
        );
    }
}
