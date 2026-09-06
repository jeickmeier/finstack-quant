//! Calibration execution engine.
//!
//! Orchestrates the execution of a calibration plan.

// `ExecuteError::Envelope` carries the full diagnostic payload (available
// IDs, breakdowns, suggestions). The size is the price of preserving that
// context across the engine's cold error path; boxing the variant would
// hurt ergonomics for a single allocator call we never make on the hot path.
#![allow(clippy::result_large_err)]

use super::schema::{CalibrationEnvelope, CalibrationPlan};
use crate::api::context_builder;
use crate::api::errors::{EnvelopeError, StrictLoadDiagnostic};
use crate::api::market_datum::MarketDatum;
use crate::api::schema::CalibrationStep;
use crate::api::schema::{CalibrationResult, CalibrationResultEnvelope};
use crate::config::CalibrationConfig;
use crate::quotes::market_quote::MarketQuote;
use crate::step_runtime;
use crate::step_runtime::StepOutcome;
use crate::validation::preflight_step;
use crate::CalibrationReport;
use finstack_quant_core::explain::{ExplanationTrace, TraceEntry};
use finstack_quant_core::market_data::context::MarketContext;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Stage at which calibration execution failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStage {
    /// Strict request ingestion or static envelope validation.
    Ingestion,
    /// Runtime configuration validation.
    Configuration,
    /// Market-context reconstruction.
    Context,
    /// Step quote resolution or preflight checks.
    Preflight,
    /// Target construction, pricing, or solver invocation.
    Target,
    /// Final fit acceptance after solver termination.
    Solver,
}

impl ExecutionStage {
    /// Stable snake-case stage identifier used by host-language bindings.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ingestion => "ingestion",
            Self::Configuration => "configuration",
            Self::Context => "context",
            Self::Preflight => "preflight",
            Self::Target => "target",
            Self::Solver => "solver",
        }
    }
}

/// Solver diagnostics attached to a structured execution failure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionSolverDiagnostics {
    /// Maximum absolute residual at termination.
    pub max_residual: f64,
    /// Fit-acceptance tolerance.
    pub tolerance: f64,
    /// Solver iterations performed.
    pub iterations: u32,
    /// Worst-fitting quote identifier, when available.
    pub worst_quote_id: Option<String>,
    /// Signed worst-quote residual, when available.
    pub worst_quote_residual: Option<f64>,
}

/// Stable cross-language execution-error payload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecutionErrorDetails {
    /// Failure stage.
    pub stage: ExecutionStage,
    /// Failing calibration step, when the stage is step-scoped.
    pub step_id: Option<String>,
    /// Programmatic error category.
    pub category: String,
    /// Solver diagnostics for fit-acceptance failures.
    pub solver_diagnostics: Option<ExecutionSolverDiagnostics>,
    /// Human-readable underlying cause.
    pub cause: String,
    /// Original structured envelope error, when applicable.
    pub envelope_error: Option<EnvelopeError>,
    /// Structured strict-load diagnostics (JSON pointer, message, expected
    /// version, ...) when ingestion rejected the document; empty otherwise.
    #[serde(default)]
    pub diagnostics: Vec<StrictLoadDiagnostic>,
}

/// Engine error retaining a single structured contract across all stages.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExecuteError {
    /// Static validation or final fit failure.
    #[error("{error}")]
    Envelope {
        /// Failure stage.
        stage: ExecutionStage,
        /// Structured underlying envelope error.
        #[source]
        error: EnvelopeError,
    },
    /// Configuration, context, preflight, or target failure.
    #[error("{source}")]
    Other {
        /// Failure stage.
        stage: ExecutionStage,
        /// Failing step identifier, when applicable.
        step_id: Option<String>,
        /// Underlying core error.
        source: finstack_quant_core::Error,
    },
}

impl ExecuteError {
    /// Wrap a structured envelope failure at its execution stage.
    ///
    /// # Arguments
    ///
    /// * `stage` - Pipeline stage that rejected the request or fit.
    /// * `error` - Structured envelope validation or solver-acceptance cause.
    pub fn envelope(stage: ExecutionStage, error: EnvelopeError) -> Self {
        Self::Envelope { stage, error }
    }

    /// Wrap a core error with stage and optional step context.
    ///
    /// # Arguments
    ///
    /// * `stage` - Pipeline stage where the underlying operation failed.
    /// * `step_id` - Failing calibration step, or `None` for plan-wide stages.
    /// * `source` - Underlying core error.
    pub fn other(
        stage: ExecutionStage,
        step_id: Option<String>,
        source: finstack_quant_core::Error,
    ) -> Self {
        Self::Other {
            stage,
            step_id,
            source,
        }
    }

    /// Materialize the stable cross-language error payload.
    pub fn details(&self) -> ExecutionErrorDetails {
        match self {
            Self::Envelope { stage, error } => {
                let solver_diagnostics = match error {
                    EnvelopeError::SolverNotConverged {
                        max_residual,
                        tolerance,
                        iterations,
                        worst_quote_id,
                        worst_quote_residual,
                        ..
                    } => Some(ExecutionSolverDiagnostics {
                        max_residual: *max_residual,
                        tolerance: *tolerance,
                        iterations: *iterations,
                        worst_quote_id: worst_quote_id.clone(),
                        worst_quote_residual: *worst_quote_residual,
                    }),
                    _ => None,
                };
                let diagnostics = match error {
                    EnvelopeError::StrictLoad { diagnostics, .. } => diagnostics.clone(),
                    _ => Vec::new(),
                };
                ExecutionErrorDetails {
                    stage: *stage,
                    step_id: error.step_id().map(str::to_string),
                    category: error.kind_str().to_string(),
                    solver_diagnostics,
                    cause: error.to_string(),
                    envelope_error: Some(error.clone()),
                    diagnostics,
                }
            }
            Self::Other {
                stage,
                step_id,
                source,
            } => ExecutionErrorDetails {
                stage: *stage,
                step_id: step_id.clone(),
                category: core_error_category(source),
                solver_diagnostics: None,
                cause: source.to_string(),
                envelope_error: None,
                diagnostics: Vec::new(),
            },
        }
    }

    /// Serialize the stable error payload as compact wire JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.details()).unwrap_or_else(|error| {
            serde_json::json!({
                "stage": "target",
                "category": "json_serialize",
                "cause": error.to_string(),
            })
            .to_string()
        })
    }
}

fn core_error_category(error: &finstack_quant_core::Error) -> String {
    match error {
        finstack_quant_core::Error::Calibration { category, .. } => category.clone(),
        finstack_quant_core::Error::Input(_) => "input".to_string(),
        finstack_quant_core::Error::Validation(_) => "validation".to_string(),
        _ => "execution".to_string(),
    }
}

impl From<finstack_quant_core::Error> for ExecuteError {
    fn from(source: finstack_quant_core::Error) -> Self {
        Self::other(ExecutionStage::Target, None, source)
    }
}

impl From<EnvelopeError> for ExecuteError {
    fn from(error: EnvelopeError) -> Self {
        Self::envelope(ExecutionStage::Ingestion, error)
    }
}

impl From<ExecuteError> for finstack_quant_core::Error {
    fn from(error: ExecuteError) -> Self {
        let details = error.details();
        finstack_quant_core::Error::Calibration {
            message: details.cause,
            category: details.category,
        }
    }
}

/// Quote lookup table built once per plan execution.
struct QuoteIndex<'a> {
    by_id: HashMap<&'a str, MarketQuote>,
}

impl<'a> QuoteIndex<'a> {
    fn new(market_data: &'a [MarketDatum]) -> Self {
        Self {
            by_id: market_data
                .iter()
                .filter_map(|d| d.as_quote().map(|q| (d.id(), q)))
                .collect(),
        }
    }
}

/// Resolve the [`MarketQuote`] list for a step by looking up its `quote_set`
/// and then materializing each quote from the execution-scoped quote index.
fn resolve_step_quotes(
    plan: &CalibrationPlan,
    quote_index: &QuoteIndex<'_>,
    step: &CalibrationStep,
) -> std::result::Result<Vec<MarketQuote>, ExecuteError> {
    let ids = plan.quote_sets.get(&step.quote_set).ok_or_else(|| {
        ExecuteError::other(
            ExecutionStage::Preflight,
            Some(step.id.clone()),
            finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                id: format!("Quote set '{}' not found", step.quote_set),
            }),
        )
    })?;
    ids.iter()
        .map(|qid| {
            quote_index.by_id.get(qid.as_str()).cloned().ok_or_else(|| {
                ExecuteError::other(
                    ExecutionStage::Preflight,
                    Some(step.id.clone()),
                    finstack_quant_core::Error::Input(finstack_quant_core::InputError::NotFound {
                        id: format!(
                            "Quote ID '{}' (referenced by quote_set '{}') not in market_data",
                            qid, step.quote_set
                        ),
                    }),
                )
            })
        })
        .collect()
}

/// A step with its associated quotes, ready for batch execution.
///
/// Holds an owned `Vec<MarketQuote>` because quotes are resolved on demand
/// from the envelope's flat `market_data` list per step (rather than being
/// pre-materialized on the plan as in canonical).
struct StepBatchItem<'a> {
    step: &'a CalibrationStep,
    quotes: Vec<MarketQuote>,
}

/// Result of trying to add a step to a parallel batch.
enum BatchAddResult {
    /// Step was added to the batch.
    Added,
    /// Step cannot be added (output conflict or preflight failed with non-empty batch).
    Stop,
    /// Preflight failed and batch is empty - propagate the error.
    Error(ExecuteError),
}

/// Builder for accumulating steps that can execute in parallel.
struct ParallelBatchBuilder<'a> {
    plan: &'a CalibrationPlan,
    quote_index: &'a QuoteIndex<'a>,
    writes: HashSet<String>,
    batch: Vec<StepBatchItem<'a>>,
}

impl<'a> ParallelBatchBuilder<'a> {
    fn new(plan: &'a CalibrationPlan, quote_index: &'a QuoteIndex<'a>) -> Self {
        Self {
            plan,
            quote_index,
            writes: HashSet::default(),
            batch: Vec::new(),
        }
    }

    /// Try to add a step to the batch.
    fn try_add(&mut self, step: &'a CalibrationStep, context: &MarketContext) -> BatchAddResult {
        if !self.batch.is_empty() && self.depends_on_batch_outputs(step) {
            return BatchAddResult::Stop;
        }

        if self.would_conflict(step) {
            return BatchAddResult::Stop;
        }

        let quotes = match resolve_step_quotes(self.plan, self.quote_index, step) {
            Ok(quotes) => quotes,
            Err(error) => return BatchAddResult::Error(error),
        };

        if let Err(error) = preflight_step(step, &quotes, context, &self.plan.settings) {
            return if self.batch.is_empty() {
                BatchAddResult::Error(ExecuteError::other(
                    ExecutionStage::Preflight,
                    Some(step.id.clone()),
                    error,
                ))
            } else {
                BatchAddResult::Stop
            };
        }

        self.record_output(step);
        self.batch.push(StepBatchItem { step, quotes });
        BatchAddResult::Added
    }

    /// Check if adding this step would create an output conflict.
    fn would_conflict(&self, step: &CalibrationStep) -> bool {
        step.params
            .io()
            .writes
            .iter()
            .any(|write| self.writes.contains(write))
    }

    fn record_output(&mut self, step: &CalibrationStep) {
        self.writes.extend(step.params.io().writes);
    }

    fn depends_on_batch_outputs(&self, step: &CalibrationStep) -> bool {
        step.params
            .io()
            .reads
            .iter()
            .any(|read| self.writes.contains(read))
    }

    /// Take the accumulated batch, resetting internal state for next batch.
    fn take_batch(&mut self) -> Vec<StepBatchItem<'a>> {
        self.writes.clear();
        std::mem::take(&mut self.batch)
    }

    /// Check if batch is empty.
    fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }
}

/// Aggregated execution state for collecting results.
struct ExecutionState {
    aggregated_residuals: BTreeMap<String, f64>,
    total_iterations: usize,
    step_reports: BTreeMap<String, CalibrationReport>,
}

impl ExecutionState {
    fn new() -> Self {
        Self {
            aggregated_residuals: BTreeMap::new(),
            total_iterations: 0,
            step_reports: BTreeMap::new(),
        }
    }

    /// Record a step's execution result.
    fn record_result(&mut self, step_id: &str, report: CalibrationReport) {
        let tolerance = report
            .metadata
            .get("success_tolerance")
            .or_else(|| report.metadata.get("tolerance"))
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or_else(|| report.solver_config.tolerance());
        for (key, residual) in &report.residuals {
            self.aggregated_residuals.insert(
                format!("{step_id}:{key}:tolerance_ratio"),
                residual.abs() / tolerance,
            );
        }
        self.total_iterations += report.iterations;
        self.step_reports.insert(step_id.to_string(), report);
    }
}

/// Merges explanation traces from individual calibration steps into a plan-level trace.
fn merge_step_traces(
    step_reports: &BTreeMap<String, CalibrationReport>,
    config: &CalibrationConfig,
) -> Option<ExplanationTrace> {
    if !config.explain.enabled {
        return None;
    }

    let mut merged = ExplanationTrace::new("calibration_plan");
    for (step_id, report) in step_reports {
        merged.push(
            TraceEntry::ComputationStep {
                name: format!("step:{step_id}"),
                description: "Begin step trace".to_string(),
                metadata: None,
            },
            config.explain.max_entries,
        );

        if let Some(step_trace) = report.explanation.as_ref() {
            for entry in &step_trace.entries {
                merged.push(entry.clone(), config.explain.max_entries);
            }
            if step_trace.is_truncated() {
                merged.truncated = Some(true);
            }
        }
    }
    Some(merged)
}

/// Aggregates per-step reports into a single plan execution report.
fn aggregate_plan_report(
    aggregated_residuals: BTreeMap<String, f64>,
    total_iterations: usize,
    step_reports: &BTreeMap<String, CalibrationReport>,
    config: &CalibrationConfig,
) -> CalibrationReport {
    let all_steps_success = step_reports.values().all(|report| report.success);
    let all_steps_validation_passed = step_reports.values().all(|report| report.validation_passed);

    let mut report = CalibrationReport::new(
        aggregated_residuals,
        total_iterations,
        all_steps_success && all_steps_validation_passed,
        if all_steps_success && all_steps_validation_passed {
            "Plan execution completed"
        } else {
            "Plan execution completed with failures"
        },
    )
    .with_plan_aggregation(step_reports);
    report.update_metadata(
        "market_freshness_status",
        if config.market_freshness.is_verifiable() {
            "verified"
        } else {
            "unverified"
        },
    );
    report.update_metadata(
        "market_quote_side",
        match config.market_freshness.quote_side {
            crate::config::MarketQuoteSide::Mid => "mid",
            crate::config::MarketQuoteSide::Bid => "bid",
            crate::config::MarketQuoteSide::Ask => "ask",
        },
    );
    report.update_metadata(
        "bid_ask_validation",
        "not_applicable_single_side_quote_schema",
    );
    if let Some(timestamp) = config.market_freshness.snapshot_timestamp.as_ref() {
        report.update_metadata("market_snapshot_timestamp", timestamp);
    }
    report.update_metadata("type", "plan_execution");
    report.update_metadata("method", "plan_execution");
    report.update_metadata(
        "solver_tolerance",
        format!("{:.2e}", config.solver.tolerance()),
    );
    report.update_metadata("residual_units", "absolute_residual_over_step_tolerance");
    report.update_metadata("raw_residuals", "retained_in_step_reports");

    if !all_steps_validation_passed {
        let failures = collect_validation_failures(step_reports);
        report = report.with_validation_result(false, Some(failures.join("; ")));
    }

    if let Some(trace) = merge_step_traces(step_reports, config) {
        report = report.with_explanation(trace);
    }

    report
}

/// Collect validation failure messages from step reports.
fn collect_validation_failures(step_reports: &BTreeMap<String, CalibrationReport>) -> Vec<String> {
    step_reports
        .iter()
        .filter(|(_, r)| !r.validation_passed)
        .map(|(step_id, r)| {
            format!(
                "{step_id}:{}",
                r.validation_error.as_deref().unwrap_or("validation failed")
            )
        })
        .collect()
}

/// Execute a batch of steps in parallel.
fn execute_batch(
    batch: &[StepBatchItem],
    context: &MarketContext,
    settings: &CalibrationConfig,
) -> std::result::Result<Vec<StepOutcome>, ExecuteError> {
    if batch.len() == 1 {
        let item = &batch[0];
        let outcome =
            step_runtime::execute(item.step, &item.quotes, context, settings).map_err(|error| {
                ExecuteError::other(ExecutionStage::Target, Some(item.step.id.clone()), error)
            })?;
        return Ok(vec![outcome]);
    }

    let run_item = |item: &StepBatchItem| {
        step_runtime::execute(item.step, &item.quotes, context, settings).map_err(|error| {
            ExecuteError::other(ExecutionStage::Target, Some(item.step.id.clone()), error)
        })
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        batch.par_iter().map(run_item).collect()
    }

    #[cfg(target_arch = "wasm32")]
    {
        batch.iter().map(run_item).collect()
    }
}

/// Apply batch results to context and state.
///
/// When `fail_on_bad_fit` is set and any step in the batch did not converge,
/// the batch is treated atomically: **no** step's output is installed and a
/// `Calibration` error is propagated for the first failing step. This
/// preserves the parallel path's equivalence to the sequential path for
/// convergence gating.
fn apply_batch_results(
    batch: Vec<StepBatchItem>,
    results: Vec<StepOutcome>,
    context: &mut MarketContext,
    state: &mut ExecutionState,
    fail_on_bad_fit: bool,
) -> std::result::Result<(), ExecuteError> {
    if fail_on_bad_fit {
        if let Some((item, failing)) = batch
            .iter()
            .zip(results.iter())
            .find(|(_, r)| !r.report.success)
        {
            return Err(ExecuteError::envelope(
                ExecutionStage::Solver,
                bad_fit_envelope_error(&item.step.id, &failing.report),
            ));
        }
    }
    for (item, result) in batch.into_iter().zip(results) {
        let StepOutcome {
            output,
            report,
            credit_index_update,
        } = result;
        step_runtime::apply_output(context, output, credit_index_update);
        state.record_result(&item.step.id, report);
    }
    Ok(())
}

/// Execute steps in parallel mode.
fn execute_parallel(
    plan: &CalibrationPlan,
    quote_index: &QuoteIndex<'_>,
    context: &mut MarketContext,
    state: &mut ExecutionState,
) -> std::result::Result<(), ExecuteError> {
    let mut index = 0;
    while index < plan.steps.len() {
        let mut builder = ParallelBatchBuilder::new(plan, quote_index);

        // Build batch of independent steps
        while index < plan.steps.len() {
            match builder.try_add(&plan.steps[index], context) {
                BatchAddResult::Added => index += 1,
                BatchAddResult::Stop => break,
                BatchAddResult::Error(error) => return Err(error),
            }
        }

        if builder.is_empty() {
            continue;
        }

        let batch = builder.take_batch();
        tracing::debug!(
            batch_size = batch.len(),
            step_ids = ?batch.iter().map(|b| b.step.id.as_str()).collect::<Vec<_>>(),
            "executing parallel calibration batch"
        );
        let results = execute_batch(&batch, context, &plan.settings)?;
        apply_batch_results(
            batch,
            results,
            context,
            state,
            plan.settings.fail_on_bad_fit,
        )?;
    }
    Ok(())
}

/// Execute steps in sequential mode.
fn execute_sequential(
    plan: &CalibrationPlan,
    quote_index: &QuoteIndex<'_>,
    context: &mut MarketContext,
    state: &mut ExecutionState,
) -> std::result::Result<(), ExecuteError> {
    for step in &plan.steps {
        let quotes = resolve_step_quotes(plan, quote_index, step)?;

        preflight_step(step, &quotes, context, &plan.settings).map_err(|error| {
            ExecuteError::other(ExecutionStage::Preflight, Some(step.id.clone()), error)
        })?;

        tracing::debug!(step_id = %step.id, quotes = quotes.len(), "executing calibration step");
        let outcome =
            step_runtime::execute(step, &quotes, context, &plan.settings).map_err(|error| {
                ExecuteError::other(ExecutionStage::Target, Some(step.id.clone()), error)
            })?;
        let StepOutcome {
            output,
            report,
            credit_index_update,
        } = outcome;
        tracing::debug!(
            step_id = %step.id,
            success = %report.success,
            iterations = %report.iterations,
            max_residual = %report.max_residual,
            "calibration step complete"
        );
        if plan.settings.fail_on_bad_fit && !report.success {
            return Err(ExecuteError::envelope(
                ExecutionStage::Solver,
                bad_fit_envelope_error(&step.id, &report),
            ));
        }
        step_runtime::apply_output(context, output, credit_index_update);
        state.record_result(&step.id, report);
    }
    Ok(())
}

/// Build the structured envelope error describing a step that failed to
/// converge.
///
/// Carries the worst-fitting quote derived from `report.residuals` so
/// downstream code can pattern-match on the error kind and surface the
/// failing quote ID without re-parsing the message.
fn bad_fit_envelope_error(step_id: &str, report: &CalibrationReport) -> EnvelopeError {
    // Prefer the actual success-gate tolerance (recorded in metadata by
    // `for_type_with_tolerance`) over the solver's internal root-finder
    // tolerance, which is a different quantity and misleading here.
    let tolerance = report
        .metadata
        .get("success_tolerance")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or_else(|| report.solver_config.tolerance());
    EnvelopeError::SolverNotConverged {
        step_id: step_id.to_string(),
        max_residual: report.max_residual,
        tolerance,
        iterations: report.iterations.try_into().unwrap_or(u32::MAX),
        worst_quote_id: report.worst_quote_id.clone(),
        worst_quote_residual: report.worst_quote_residual,
    }
}

/// Parse a JSON calibration envelope and execute it.
///
/// Combines the internal `parse_envelope` loader with [`execute`] so host
/// bindings do not reimplement the parse-then-run path.
///
/// # Arguments
///
/// * `envelope_json` - UTF-8 JSON calibration-envelope document in the
///   canonical v1 flat-market shape.
///
/// # Errors
///
/// Returns [`ExecuteError`] when strict loading rejects the document or
/// [`execute`] fails after a successful parse.
pub fn execute_json(
    envelope_json: &str,
) -> std::result::Result<CalibrationResultEnvelope, ExecuteError> {
    let envelope = super::validate::parse_envelope(envelope_json)?;
    execute(&envelope)
}

/// Execute a full [`CalibrationEnvelope`] plan.
///
/// Returns a structured [`ExecuteError`] for ingestion, configuration,
/// context, preflight, target, and solver-acceptance failures (including
/// `worst_quote_id` on non-convergence). `From<ExecuteError>` maps that
/// payload to [`finstack_quant_core::Error`] so `?` still works in
/// functions that return `finstack_quant_core::Result`.
///
/// Static validation is fail-fast: the first envelope error is returned.
/// [`super::validate::dry_run`] lists every static error without solving.
///
/// # Arguments
///
/// * `envelope` - Typed calibration plan, inputs, quote sets, and solver
///   settings to execute in declared dependency order.
pub fn execute(
    envelope: &CalibrationEnvelope,
) -> std::result::Result<CalibrationResultEnvelope, ExecuteError> {
    let _span = tracing::info_span!(
        "calibration_plan",
        plan_id = %envelope.plan.id,
        steps = envelope.plan.steps.len(),
    )
    .entered();

    if let Some(error) = super::validate::validate(envelope)
        .errors
        .into_iter()
        .next()
    {
        return Err(ExecuteError::envelope(ExecutionStage::Ingestion, error));
    }
    let plan = &envelope.plan;
    plan.settings
        .validate()
        .map_err(|error| ExecuteError::other(ExecutionStage::Configuration, None, error))?;
    let mut context = context_builder::build_initial_context(
        &envelope.prior_market,
        &envelope.market_data,
        &plan.settings,
    )
    .map_err(|error| ExecuteError::other(ExecutionStage::Context, None, error))?;
    let quote_index = QuoteIndex::new(&envelope.market_data);
    let mut state = ExecutionState::new();

    if plan.settings.use_parallel {
        execute_parallel(plan, &quote_index, &mut context, &mut state)?;
    } else {
        execute_sequential(plan, &quote_index, &mut context, &mut state)?;
    }

    let ExecutionState {
        aggregated_residuals,
        total_iterations,
        step_reports,
    } = state;
    let aggregated_report = aggregate_plan_report(
        aggregated_residuals,
        total_iterations,
        &step_reports,
        &plan.settings,
    );
    tracing::info!(
        success = %aggregated_report.success,
        max_residual = %aggregated_report.max_residual,
        iterations = %aggregated_report.iterations,
        "calibration plan completed"
    );

    let result = CalibrationResult {
        final_market: (&context).into(),
        report: aggregated_report,
        step_reports,
        results_meta: finstack_quant_core::config::results_meta(
            &finstack_quant_core::config::FinstackConfig::default(),
        ),
    };

    Ok(CalibrationResultEnvelope::new(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::explain::ExplanationTrace;

    /// Helper to create an ExecutionState for testing.
    fn make_test_state(
        residuals: BTreeMap<String, f64>,
        iterations: usize,
        step_reports: BTreeMap<String, CalibrationReport>,
    ) -> ExecutionState {
        ExecutionState {
            aggregated_residuals: residuals,
            total_iterations: iterations,
            step_reports,
        }
    }

    #[test]
    fn aggregated_report_uses_dimensionless_tolerance_ratios() {
        let cfg = crate::config::CalibrationConfig {
            solver: crate::solver::SolverConfig::default().with_tolerance(1e-12),
            ..Default::default()
        };
        let mut state = ExecutionState::new();
        state.record_result(
            "s1",
            CalibrationReport::for_type_with_tolerance(
                "pv",
                BTreeMap::from([("a".to_string(), 3.0)]),
                2,
                3.0,
            ),
        );
        state.record_result(
            "s2",
            CalibrationReport::for_type_with_tolerance(
                "vol",
                BTreeMap::from([("b".to_string(), 4.0)]),
                3,
                2.0,
            ),
        );
        let report = aggregate_plan_report(
            state.aggregated_residuals,
            state.total_iterations,
            &state.step_reports,
            &cfg,
        );

        // Plan aggregation keeps the raw residual aggregates on `rmse` /
        // `max_residual` and exposes the tolerance-normalized values on the
        // `*_ratio` fields, so the dimensionless assertion reads `rmse_ratio`.
        let expected = ((1.0_f64 + 4.0) / 2.0).sqrt();
        assert!(
            (report.rmse_ratio.expect("plan aggregation sets rmse_ratio") - expected).abs() < 1e-12
        );
        assert!((report.objective_value - expected).abs() < 1e-12);
        assert_eq!(
            report.metadata.get("residual_units").map(String::as_str),
            Some("absolute_residual_over_step_tolerance")
        );
        assert!(report
            .residuals
            .keys()
            .all(|key| key.ends_with(":tolerance_ratio")));
    }

    #[test]
    fn aggregated_report_merges_step_traces_when_enabled() {
        let mut step_reports = BTreeMap::new();
        let mut r1 = CalibrationReport::new(BTreeMap::new(), 0, true, "ok");
        r1.explanation = Some(ExplanationTrace {
            trace_type: "calibration".to_string(),
            entries: vec![TraceEntry::ComputationStep {
                name: "inner".to_string(),
                description: "inner step".to_string(),
                metadata: None,
            }],
            truncated: None,
        });
        step_reports.insert("s1".to_string(), r1);

        let cfg = crate::config::CalibrationConfig {
            explain: finstack_quant_core::explain::ExplainOpts::enabled(),
            ..Default::default()
        };
        let state = make_test_state(BTreeMap::new(), 0, step_reports);
        let report = aggregate_plan_report(
            state.aggregated_residuals,
            state.total_iterations,
            &state.step_reports,
            &cfg,
        );
        let trace = report.explanation.expect("merged explanation");
        assert!(
            trace
                .entries
                .iter()
                .any(|e| matches!(e, TraceEntry::ComputationStep { name, .. } if name == "inner")),
            "expected merged trace to contain the step's entries"
        );
    }

    #[test]
    fn aggregated_report_surfaces_validation_failures() {
        let mut step_reports = BTreeMap::new();
        let failed = CalibrationReport::new(BTreeMap::new(), 1, true, "converged")
            .with_validation_result(false, Some("invalid curve shape".to_string()));
        step_reports.insert("curve_step".to_string(), failed);

        let cfg = crate::config::CalibrationConfig::default();
        let state = make_test_state(BTreeMap::new(), 1, step_reports);
        let report = aggregate_plan_report(
            state.aggregated_residuals,
            state.total_iterations,
            &state.step_reports,
            &cfg,
        );

        assert!(!report.validation_passed);
        assert!(!report.success);
        let msg = report
            .validation_error
            .as_deref()
            .expect("validation error should be present");
        assert!(
            msg.contains("curve_step:invalid curve shape"),
            "expected step id and reason in validation error: {msg}"
        );
    }
    #[test]
    fn execution_errors_share_stage_step_category_and_cause_contract() {
        let preflight = ExecuteError::other(
            ExecutionStage::Preflight,
            Some("hazard".to_string()),
            finstack_quant_core::Error::Validation("missing discount curve".to_string()),
        );
        let details = preflight.details();
        assert_eq!(details.stage, ExecutionStage::Preflight);
        assert_eq!(details.step_id.as_deref(), Some("hazard"));
        assert_eq!(details.category, "validation");
        assert!(details.cause.contains("missing discount curve"));
        let wire: serde_json::Value =
            serde_json::from_str(&preflight.to_json()).expect("compact error JSON");
        assert_eq!(wire["stage"], "preflight");

        let solver = ExecuteError::envelope(
            ExecutionStage::Solver,
            EnvelopeError::SolverNotConverged {
                step_id: "hazard".to_string(),
                max_residual: 2e-6,
                tolerance: 1e-6,
                iterations: 12,
                worst_quote_id: Some("CDS-5Y".to_string()),
                worst_quote_residual: Some(2e-6),
            },
        );
        let details = solver.details();
        assert_eq!(details.stage, ExecutionStage::Solver);
        assert_eq!(details.category, "solver_not_converged");
        let source = std::error::Error::source(&solver).expect("envelope source must be preserved");
        assert!(source.to_string().contains("hazard"));
        assert_eq!(
            details
                .solver_diagnostics
                .as_ref()
                .and_then(|diagnostics| diagnostics.worst_quote_id.as_deref()),
            Some("CDS-5Y")
        );
    }
    #[test]
    fn parallel_batch_conflicts_use_all_step_io_writes() {
        let xccy: CalibrationStep = serde_json::from_value(serde_json::json!({
            "id": "xccy-a",
            "quote_set": "quotes",
            "kind": "xccy_basis",
            "curve_id": "EUR-OIS",
            "currency": "EUR",
            "base_date": "2025-01-01",
            "fx_spot": 1.1,
            "domestic_discount_id": "USD-OIS",
            "basis_spread_curve_id": "EUR-BASIS"
        }))
        .expect("first xccy step");
        let conflicting: CalibrationStep = serde_json::from_value(serde_json::json!({
            "id": "xccy-b",
            "quote_set": "quotes",
            "kind": "xccy_basis",
            "curve_id": "EUR-BASIS",
            "currency": "EUR",
            "base_date": "2025-01-01",
            "fx_spot": 1.1,
            "domestic_discount_id": "USD-OIS"
        }))
        .expect("conflicting xccy step");
        let student_a: CalibrationStep = serde_json::from_value(serde_json::json!({
            "id": "student-a",
            "quote_set": "quotes",
            "kind": "student_t",
            "tranche_instrument_id": "TRANCHE",
            "base_correlation_curve_id": "CORR"
        }))
        .expect("first student step");
        let student_b: CalibrationStep = serde_json::from_value(serde_json::json!({
            "id": "student-b",
            "quote_set": "quotes",
            "kind": "student_t",
            "tranche_instrument_id": "TRANCHE",
            "base_correlation_curve_id": "CORR"
        }))
        .expect("second student step");
        let plan = CalibrationPlan {
            id: "io-conflicts".to_string(),
            description: None,
            quote_sets: Default::default(),
            steps: vec![xccy, conflicting, student_a, student_b],
            settings: Default::default(),
        };
        let market_data = Vec::new();
        let quote_index = QuoteIndex::new(&market_data);
        let mut builder = ParallelBatchBuilder::new(&plan, &quote_index);

        builder.record_output(&plan.steps[0]);
        assert!(
            builder.would_conflict(&plan.steps[1]),
            "secondary XCCY basis-curve write must conflict"
        );
        builder.take_batch();
        builder.record_output(&plan.steps[2]);
        assert!(
            builder.would_conflict(&plan.steps[3]),
            "duplicate Student-t scalar write must conflict"
        );
    }
}
