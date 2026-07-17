//! Calibration execution engine.
//!
//! Orchestrates the execution of a calibration plan.

// `ExecuteError::Envelope` carries the full diagnostic payload (available
// IDs, breakdowns, suggestions). The size is the price of preserving that
// context across the engine's cold error path; boxing the variant would
// hurt ergonomics for a single allocator call we never make on the hot path.
#![allow(clippy::result_large_err)]

use super::schema::{CalibrationEnvelope, CalibrationPlan};
use crate::calibration::api::context_builder;
use crate::calibration::api::errors::EnvelopeError;
use crate::calibration::api::market_datum::MarketDatum;
use crate::calibration::api::schema::CalibrationStep;
use crate::calibration::api::schema::{CalibrationResult, CalibrationResultEnvelope};
use crate::calibration::config::CalibrationConfig;
use crate::calibration::step_runtime;
use crate::calibration::step_runtime::{OutputKey, StepOutcome};
use crate::calibration::validation::preflight_step;
use crate::calibration::CalibrationReport;
use crate::market::quotes::market_quote::MarketQuote;
use finstack_quant_core::explain::{ExplanationTrace, TraceEntry};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Internal error type that preserves structured [`EnvelopeError`] detail
/// across the engine pipeline.
///
/// The public [`execute`] flattens this back to [`finstack_quant_core::Error`] for
/// backwards compatibility. Bindings that want full structured detail (e.g.,
/// `worst_quote_id` on solver non-convergence) call
/// [`execute_with_diagnostics`] directly.
#[derive(Debug, thiserror::Error)]
pub enum ExecuteError {
    /// Structured envelope-shaped failure (currently only solver
    /// non-convergence). Static envelope validation is a separate,
    /// opt-in pass exposed via [`super::validate::validate`] and the
    /// `dry_run` / `dependency_graph_json` bindings; `execute` does not
    /// run it automatically.
    #[error(transparent)]
    Envelope(EnvelopeError),
    /// Other (legacy, stringly-typed) errors from the engine pipeline:
    /// quote-set lookup, preflight validation, market-context construction,
    /// and step-runtime failures.
    #[error(transparent)]
    Other(finstack_quant_core::Error),
}

impl From<finstack_quant_core::Error> for ExecuteError {
    fn from(e: finstack_quant_core::Error) -> Self {
        Self::Other(e)
    }
}

impl From<EnvelopeError> for ExecuteError {
    fn from(e: EnvelopeError) -> Self {
        Self::Envelope(e)
    }
}

impl From<ExecuteError> for finstack_quant_core::Error {
    fn from(e: ExecuteError) -> Self {
        match e {
            ExecuteError::Envelope(env) => env.into(),
            ExecuteError::Other(other) => other,
        }
    }
}

// =============================================================================
// Helper Types
// =============================================================================

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
        ExecuteError::Other(finstack_quant_core::Error::Input(
            finstack_quant_core::InputError::NotFound {
                id: format!("Quote set '{}' not found", step.quote_set),
            },
        ))
    })?;
    ids.iter()
        .map(|qid| {
            quote_index.by_id.get(qid.as_str()).cloned().ok_or_else(|| {
                ExecuteError::Other(finstack_quant_core::Error::Input(
                    finstack_quant_core::InputError::NotFound {
                        id: format!(
                            "Quote ID '{}' (referenced by quote_set '{}') not in market_data",
                            qid, step.quote_set
                        ),
                    },
                ))
            })
        })
        .collect()
}

/// A step with its associated quotes, ready for batch execution.
///
/// Holds an owned `Vec<MarketQuote>` because quotes are resolved on demand
/// from the envelope's flat `market_data` list per step (rather than being
/// pre-materialized on the plan as in v2).
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
    Error(finstack_quant_core::Error),
}

/// Builder for accumulating steps that can execute in parallel.
struct ParallelBatchBuilder<'a> {
    plan: &'a CalibrationPlan,
    quote_index: &'a QuoteIndex<'a>,
    curve_outputs: HashSet<CurveId>,
    surface_outputs: HashSet<CurveId>,
    scalar_outputs: HashSet<String>,
    batch: Vec<StepBatchItem<'a>>,
}

impl<'a> ParallelBatchBuilder<'a> {
    fn new(plan: &'a CalibrationPlan, quote_index: &'a QuoteIndex<'a>) -> Self {
        Self {
            plan,
            quote_index,
            curve_outputs: HashSet::default(),
            surface_outputs: HashSet::default(),
            scalar_outputs: HashSet::default(),
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
            Ok(q) => q,
            Err(ExecuteError::Other(e)) => return BatchAddResult::Error(e),
            Err(ExecuteError::Envelope(e)) => return BatchAddResult::Error(e.into()),
        };

        // Preflight validation
        if let Err(err) = preflight_step(step, &quotes, context, &self.plan.settings) {
            return if self.batch.is_empty() {
                BatchAddResult::Error(err)
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
        match step_runtime::output_key(step) {
            OutputKey::Curve(id) => self.curve_outputs.contains(&id),
            OutputKey::Surface(id) => self.surface_outputs.contains(&id),
            OutputKey::Scalar(key) => self.scalar_outputs.contains(&key),
        }
    }

    fn record_output(&mut self, step: &CalibrationStep) {
        match step_runtime::output_key(step) {
            OutputKey::Curve(id) => {
                self.curve_outputs.insert(id);
            }
            OutputKey::Surface(id) => {
                self.surface_outputs.insert(id);
            }
            OutputKey::Scalar(key) => {
                self.scalar_outputs.insert(key);
            }
        }
    }

    fn depends_on_batch_outputs(&self, step: &CalibrationStep) -> bool {
        input_curve_ids(step)
            .iter()
            .any(|id| self.curve_outputs.contains(id) || self.surface_outputs.contains(id))
    }

    /// Take the accumulated batch, resetting internal state for next batch.
    fn take_batch(&mut self) -> Vec<StepBatchItem<'a>> {
        self.curve_outputs.clear();
        self.surface_outputs.clear();
        self.scalar_outputs.clear();
        std::mem::take(&mut self.batch)
    }

    /// Check if batch is empty.
    fn is_empty(&self) -> bool {
        self.batch.is_empty()
    }
}

fn input_curve_ids(step: &CalibrationStep) -> Vec<CurveId> {
    step.params
        .io()
        .reads
        .into_iter()
        .map(CurveId::from)
        .collect()
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
        for (k, v) in &report.residuals {
            self.aggregated_residuals
                .insert(format!("{step_id}:{k}"), *v);
        }
        self.total_iterations += report.iterations;
        self.step_reports.insert(step_id.to_string(), report);
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

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
fn aggregate_plan_report(state: ExecutionState, config: &CalibrationConfig) -> CalibrationReport {
    let all_steps_success = state.step_reports.values().all(|r| r.success);
    let all_steps_validation_passed = state.step_reports.values().all(|r| r.validation_passed);

    let mut report = CalibrationReport::new(
        state.aggregated_residuals,
        state.total_iterations,
        all_steps_success && all_steps_validation_passed,
        if all_steps_success && all_steps_validation_passed {
            "Plan execution completed"
        } else {
            "Plan execution completed with failures"
        },
    );
    report.update_metadata("type", "plan_execution");
    report.update_metadata("method", "plan_execution");
    report.update_metadata(
        "solver_tolerance",
        format!("{:.2e}", config.solver.tolerance()),
    );

    if !all_steps_validation_passed {
        let failures = collect_validation_failures(&state.step_reports);
        report = report.with_validation_result(false, Some(failures.join("; ")));
    }

    if let Some(trace) = merge_step_traces(&state.step_reports, config) {
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
) -> Result<Vec<StepOutcome>> {
    if batch.len() == 1 {
        let item = &batch[0];
        let outcome = step_runtime::execute(item.step, &item.quotes, context, settings)?;
        return Ok(vec![outcome]);
    }

    batch
        .par_iter()
        .map(|item| step_runtime::execute(item.step, &item.quotes, context, settings))
        .collect()
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
            return Err(bad_fit_envelope_error(&item.step.id, &failing.report).into());
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
                BatchAddResult::Error(e) => return Err(e.into()),
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

        preflight_step(step, &quotes, context, &plan.settings)?;

        tracing::debug!(step_id = %step.id, quotes = quotes.len(), "executing calibration step");
        let outcome = step_runtime::execute(step, &quotes, context, &plan.settings)?;
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
            return Err(bad_fit_envelope_error(&step.id, &report).into());
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

// =============================================================================
// Public API
// =============================================================================

/// Execute a full [`CalibrationEnvelope`] plan.
///
/// Primary entry point — flattens any structured envelope failure to
/// [`finstack_quant_core::Error`] for backwards-compatible callers. Bindings that
/// want full structured detail (e.g., `worst_quote_id` on solver
/// non-convergence) should call [`execute_with_diagnostics`] directly.
///
/// # Arguments
///
/// * `envelope` - Typed calibration plan, inputs, quote sets, and solver
///   settings to execute in declared dependency order.
pub fn execute(envelope: &CalibrationEnvelope) -> Result<CalibrationResultEnvelope> {
    execute_with_diagnostics(envelope).map_err(Into::into)
}

/// Execute a full [`CalibrationEnvelope`] plan, preserving structured
/// envelope errors.
///
/// Returns [`ExecuteError::Envelope`] for solver non-convergence (carries
/// `worst_quote_id`, `tolerance`, etc.) and [`ExecuteError::Other`] for the
/// remaining stringly-typed failures from the engine pipeline. Callers that
/// don't need the structured detail can use [`execute`] instead.
///
/// # Arguments
///
/// * `envelope` - Typed calibration plan, inputs, quote sets, and solver
///   settings to execute in declared dependency order.
pub fn execute_with_diagnostics(
    envelope: &CalibrationEnvelope,
) -> std::result::Result<CalibrationResultEnvelope, ExecuteError> {
    let _span = tracing::info_span!(
        "calibration_plan",
        plan_id = %envelope.plan.id,
        steps = envelope.plan.steps.len(),
    )
    .entered();

    let plan = &envelope.plan;
    plan.settings.validate().map_err(ExecuteError::Other)?;
    let mut context = context_builder::build_initial_context(
        &envelope.prior_market,
        &envelope.market_data,
        &plan.settings,
    )
    .map_err(ExecuteError::Other)?;
    let quote_index = QuoteIndex::new(&envelope.market_data);
    let mut state = ExecutionState::new();

    if plan.settings.use_parallel {
        execute_parallel(plan, &quote_index, &mut context, &mut state)?;
    } else {
        execute_sequential(plan, &quote_index, &mut context, &mut state)?;
    }

    let step_reports = state.step_reports.clone();
    let aggregated_report = aggregate_plan_report(state, &plan.settings);

    let result = CalibrationResult {
        final_market: (&context).into(),
        report: aggregated_report.clone(),
        step_reports,
        results_meta: finstack_quant_core::config::results_meta(
            &finstack_quant_core::config::FinstackConfig::default(),
        ),
    };

    tracing::info!(
        success = %aggregated_report.success,
        max_residual = %aggregated_report.max_residual,
        iterations = %aggregated_report.iterations,
        "calibration plan completed"
    );

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
    fn aggregated_report_computes_rmse_and_objective() {
        let mut step_reports = BTreeMap::new();
        step_reports.insert(
            "s1".to_string(),
            CalibrationReport::new(BTreeMap::from([("a".to_string(), 3.0)]), 2, true, "ok"),
        );
        step_reports.insert(
            "s2".to_string(),
            CalibrationReport::new(BTreeMap::from([("b".to_string(), 4.0)]), 3, true, "ok"),
        );

        let aggregated_residuals =
            BTreeMap::from([("s1:a".to_string(), 3.0), ("s2:b".to_string(), 4.0)]);
        let cfg = crate::calibration::config::CalibrationConfig {
            solver: crate::calibration::solver::SolverConfig::brent_default().with_tolerance(1e-12),
            ..Default::default()
        };
        let state = make_test_state(aggregated_residuals, 5, step_reports);
        let report = aggregate_plan_report(state, &cfg);

        let expected = ((3.0_f64 * 3.0 + 4.0 * 4.0) / 2.0).sqrt();
        assert!((report.rmse - expected).abs() < 1e-12);
        assert!((report.objective_value - expected).abs() < 1e-12);
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

        let cfg = crate::calibration::config::CalibrationConfig {
            explain: finstack_quant_core::explain::ExplainOpts::enabled(),
            ..Default::default()
        };
        let state = make_test_state(BTreeMap::new(), 0, step_reports);
        let report = aggregate_plan_report(state, &cfg);
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

        let cfg = crate::calibration::config::CalibrationConfig::default();
        let state = make_test_state(BTreeMap::new(), 1, step_reports);
        let report = aggregate_plan_report(state, &cfg);

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
}
