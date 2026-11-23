//! Core [`Evaluator`] period loop, DAG execution, and Monte Carlo entry points.

use super::prepared::*;
use crate::error::{Error, Result};
use crate::evaluator::cell::{Cell, UnavailableReason};
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::forecast_eval;
use crate::evaluator::formula::evaluate_value;
use crate::evaluator::monte_carlo::{MonteCarloAccumulator, MonteCarloConfig, MonteCarloResults};
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
    pub(super) compiled_cache: std::sync::Arc<IndexMap<NodeId, Expr>>,

    /// Resolved units of the prepared model's nodes (Arc-shared like the
    /// compiled expressions); they type each computed cell.
    pub(super) node_value_types: std::sync::Arc<IndexMap<NodeId, NodeValueType>>,

    /// Cached forecast results: node_id → (period_id → value)
    pub(super) forecast_cache: IndexMap<NodeId, forecast_eval::ForecastRun>,

    /// Optional check suite run after evaluation to produce inline validation
    pub(super) check_suite: Option<std::sync::Arc<crate::checks::CheckSuite>>,

    /// As-of visibility cutoff for the current evaluation run. Actual periods
    /// starting after this date have hidden explicit values and are treated as
    /// forecast periods by forecast evaluation.
    pub(super) visibility_cutoff: Option<finstack_quant_core::dates::Date>,
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
    pub(super) fn evaluate_inner(
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
        let setup = self.run_setup(model, &prepared, capital_structure)?;
        let (mut results, all_warnings) =
            self.evaluate_periods(model, &prepared, &setup, capital_structure, None)?;

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
            capital_structure
                .filter(|_| valuation_date.is_some())
                .map(|(market, _)| Self::market_content_hash(market))
                .transpose()?
                .flatten(),
        )?;

        Ok(results)
    }

    /// Everything a run needs that does not vary by Monte Carlo path.
    ///
    /// `capital_structure` is the market and cashflow date the model's
    /// instruments are generated from; `None` skips the capital structure.
    pub(super) fn run_setup(
        &self,
        model: &FinancialModelSpec,
        prepared: &PreparedEvaluation,
        capital_structure: Option<(
            &finstack_quant_core::market_data::context::MarketContext,
            finstack_quant_core::dates::Date,
        )>,
    ) -> Result<RunSetup> {
        let history = PeriodHistory::with_periods(
            std::sync::Arc::clone(&prepared.node_to_column),
            &model.periods,
        );
        let Some((market_ctx, as_of)) = capital_structure else {
            return Ok(RunSetup {
                history,
                capital: None,
            });
        };

        // Only nodes whose formula or where clause reaches `cs.*` (and their
        // dependents) need the capital-structure pass.
        let mut cs_seed_nodes = HashSet::new();
        for (node_id, spec) in &model.nodes {
            for text in [spec.formula_text.as_deref(), spec.where_text.as_deref()]
                .into_iter()
                .flatten()
            {
                if crate::utils::formula::contains_capital_structure_reference(text)? {
                    cs_seed_nodes.insert(node_id.clone());
                }
            }
        }
        let affected_nodes = dependent_closure(&prepared.dag, &cs_seed_nodes);

        let Some(instruments) = self.build_instruments(model)? else {
            return Ok(RunSetup {
                history,
                capital: None,
            });
        };
        let mut opening_state = crate::capital_structure::CapitalStructureState::new();
        if let Some(first_period) = model.periods.first() {
            for (instrument_id, instrument) in &instruments {
                let opening_balance = instrument_opening_balance(
                    instrument.as_ref(),
                    market_ctx,
                    as_of,
                    first_period.start,
                )
                .map_err(|err| missing_market_data(instrument_id, err))?;
                opening_state
                    .opening_balances
                    .insert(instrument_id.clone(), opening_balance);
            }
        }
        Ok(RunSetup {
            history,
            capital: Some(CapitalSetup {
                instruments,
                opening_state,
                affected_nodes,
            }),
        })
    }

    /// Run every period in timeline order on a prepared plan.
    ///
    /// `setup` is the run's path-invariant state and `capital_structure` the
    /// market and cashflow date its instruments were generated from.
    /// `mc_path` is the Monte Carlo path (`None` for a single
    /// deterministic run); each path runs this same loop, capital-structure
    /// waterfall included. Shared by [`evaluate_inner`](Self::evaluate_inner),
    /// [`evaluate_prepared_inner`](Self::evaluate_prepared_inner) and
    /// [`evaluate_monte_carlo`](Self::evaluate_monte_carlo).
    pub(super) fn evaluate_periods(
        &mut self,
        model: &FinancialModelSpec,
        prepared: &PreparedEvaluation,
        setup: &RunSetup,
        capital_structure: Option<(
            &finstack_quant_core::market_data::context::MarketContext,
            finstack_quant_core::dates::Date,
        )>,
        mc_path: Option<PathId>,
    ) -> Result<(StatementResult, Vec<EvalWarning>)> {
        let mut mc_z_cache = McZCache::new();
        let capital = capital_structure.zip(setup.capital.as_ref());
        // Each run, and each path, starts from the seeded opening state.
        let mut cs_state = capital.map(|(_, capital)| capital.opening_state.clone());

        let mut historical = std::sync::Arc::new(setup.history.clone());
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
                if let (Some(((market_ctx, as_of), capital)), Some(state)) =
                    (capital, cs_state.as_mut())
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
                            instruments: &capital.instruments,
                            state,
                            affected_nodes: &capital.affected_nodes,
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

    pub(super) fn evaluate_monte_carlo_inner(
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
            None => {
                self.visibility_cutoff = None;
                contractual_capital_structure(model, &empty_market)
            }
        };

        // Run paths — parallel via rayon on native targets, serial on wasm32
        // (rayon's thread pool is unavailable in single-threaded wasm).
        // Both paths are deterministic for a given seed, and both report the
        // lowest-index failing path when several paths fail.
        let setup = self.run_setup(model, &prepared, capital_structure)?;
        let run_single_path = |path_idx: usize| -> Result<(StatementResult, Vec<EvalWarning>)> {
            let mut path_eval = self.clone();
            path_eval.forecast_cache.clear();

            let path = PathId {
                run_seed: config.seed,
                path: path_idx as u64,
            };
            path_eval.evaluate_periods(model, &prepared, &setup, capital_structure, Some(path))
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
                                acc.push_path(path_idx, &path_results, warnings)
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
                acc.push_path(path_idx, &path_results, warnings)?;
            }
            acc
        };

        let provenance = super::super::RunProvenance::for_run(
            model,
            context.map_or(ObservationCutoff::FullSnapshot, |source| source.cutoff),
            context.map(|source| source.as_of),
            context
                .map(|source| Self::market_content_hash(source.market))
                .transpose()?
                .flatten(),
        )?;
        accumulator.finish(provenance)
    }

    pub(super) fn market_content_hash(
        market: &finstack_quant_core::market_data::context::MarketContext,
    ) -> Result<Option<String>> {
        use super::super::results::hash_error;
        use finstack_quant_core::market_data::context::MarketContextState;

        // Evaluation can use a live FX provider without a coherent capture capability.
        let Ok(snapshot) = MarketContextState::try_from(market) else {
            return Ok(None);
        };
        finstack_quant_core::canonical::content_hash(&snapshot)
            .map(Some)
            .map_err(hash_error)
    }

    /// Populate value types, set metadata, and run the optional check suite.
    pub(super) fn finalize_results(
        &self,
        results: &mut StatementResult,
        model: &FinancialModelSpec,
        prepared: &PreparedEvaluation,
        meta: EvalStats,
        valuation_date: Option<finstack_quant_core::dates::Date>,
        market_content_hash: Option<String>,
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
            market_content_hash,
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
    pub(super) fn evaluate_period(
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

        Ok(context.into_results())
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
pub(super) fn computed_cell(
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
pub(super) fn contractual_capital_structure<'m>(
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
