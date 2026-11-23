//! [`ScenarioEngine`] execution driver and statement/market operation runners.

use super::effects::{
    EffectSink, apply_generated_effects, flush_pending_bumps,
    generate_replace_curve_effects_parallel, independent_replace_curve_run_len, process_effects,
    should_parallel_replace_curves,
};
use super::hierarchy::{ExpansionOutcome, expand_hierarchy_operations, verify_active_target};
use super::*;
use crate::error::Result;
use crate::spec::{OperationSpec, RateBindingSpec, ScenarioSpec};
use crate::warning::Warning;
use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
use finstack_quant_core::dates::HolidayCalendar;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_statements::types::NodeId;
use finstack_quant_valuations::recalibration::RecalibrationProvider;
use std::sync::Arc;

/// Apply one rate binding, recording a warning instead of a hard error.
///
/// Returns `true` when the binding update succeeded (including the no-forecast
/// case) and `false` when it failed.
pub(super) fn apply_rate_binding(
    binding: &RateBindingSpec,
    model: &mut FinancialModelSpec,
    market: &mut MarketContext,
    calendar: Option<&dyn HolidayCalendar>,
    warnings: &mut Vec<Warning>,
) -> bool {
    match crate::adapters::statements::update_rate_from_binding(binding, model, market, calendar) {
        Ok(true) => true,
        Ok(false) => {
            warnings.push(Warning::RateBindingNoForecastValues {
                node_id: binding.node_id.as_str().to_string(),
                curve_id: binding.curve_id.as_str().to_string(),
            });
            true
        }
        Err(e) => {
            warnings.push(Warning::RateBindingFailed {
                node_id: binding.node_id.as_str().to_string(),
                curve_id: binding.curve_id.as_str().to_string(),
                reason: e.to_string(),
            });
            false
        }
    }
}

/// A statement operation deferred until every market operation has applied.
pub(super) enum StatementOp<'a> {
    /// Percentage forecast adjustment on a statement node.
    ForecastPercent { node_id: &'a NodeId, pct: f64 },
    /// Absolute forecast assignment on a statement node.
    ForecastAssign { node_id: &'a NodeId, value: f64 },
    /// Statement rate binding applied after market shocks.
    RateBinding(&'a RateBindingSpec),
}

impl<'a> StatementOp<'a> {
    fn from_operation(op: &'a OperationSpec) -> Option<Self> {
        match op {
            OperationSpec::StmtForecastPercent { node_id, pct } => {
                Some(Self::ForecastPercent { node_id, pct: *pct })
            }
            OperationSpec::StmtForecastAssign { node_id, value } => Some(Self::ForecastAssign {
                node_id,
                value: *value,
            }),
            OperationSpec::RateBinding { binding } => Some(Self::RateBinding(binding)),
            _ => None,
        }
    }
}

pub(super) fn statement_model<'m>(
    model: &'m mut Option<&mut FinancialModelSpec>,
    operation: &str,
) -> Result<&'m mut FinancialModelSpec> {
    model
        .as_deref_mut()
        .ok_or_else(|| crate::error::Error::missing_statement_model(operation))
}

pub(super) fn record_forecast_result(
    result: Result<bool>,
    node_id: &NodeId,
    op: &str,
    applied: &mut usize,
    applied_stmt_ops: &mut usize,
    warnings: &mut Vec<Warning>,
) {
    match result {
        Ok(true) => {
            *applied += 1;
            *applied_stmt_ops += 1;
        }
        Ok(false) => warnings.push(Warning::StatementNodeNoValues {
            node_id: node_id.as_str().to_string(),
            op: op.to_string(),
        }),
        Err(e) => warnings.push(Warning::StatementOpFailed {
            node_id: node_id.as_str().to_string(),
            op: op.to_string(),
            reason: e.to_string(),
        }),
    }
}

pub(super) fn par_cds_ids(op: &OperationSpec) -> Option<(&CurveId, Option<&CurveId>)> {
    match op {
        OperationSpec::CurveParallelBp {
            curve_kind: crate::CurveKind::ParCDS,
            curve_id,
            discount_curve_id,
            ..
        }
        | OperationSpec::CurveNodeBp {
            curve_kind: crate::CurveKind::ParCDS,
            curve_id,
            discount_curve_id,
            ..
        } => Some((curve_id, discount_curve_id.as_ref())),
        _ => None,
    }
}

pub(super) fn par_cds_hazard_rolls(
    ops: &[OperationSpec],
    market: &MarketContext,
) -> Result<Vec<(CurveId, CurveId)>> {
    let mut hazard_rolls = Vec::new();
    for op in ops {
        let Some((curve_id, discount_curve_id)) = par_cds_ids(op) else {
            continue;
        };
        if hazard_rolls.iter().any(|(id, _)| id == curve_id) {
            continue;
        }
        let (discount_id, _) = crate::adapters::curves::resolve_discount_curve_id(
            market,
            discount_curve_id,
            Some(curve_id),
        )?;
        hazard_rolls.push((curve_id.clone(), discount_id));
    }
    Ok(hazard_rolls)
}

/// Error message for instrument-scoped operations applied without an inventory.
pub(super) const MISSING_INSTRUMENTS: &str = "scenario contains instrument-scoped operations \
    (instrument_price_pct_by_*, instrument_spread_bp_by_*, asset_correlation_pts, \
    prepay_default_correlation_pts) but no instruments were supplied; supply the \
    instrument inventory or remove those operations";

/// Deterministic scenario applicator.
///
/// Owns the active [`FinstackConfig`](finstack_quant_core::config::FinstackConfig)
/// (stamped into reports) and the quote-recalibration provider used by
/// solve-to-par ParCDS shocks and hazard time-roll replay. Every constructor
/// attaches a fresh [`CachedRecalibrationProvider`]; callers sharing one cache
/// across an immutable batch replace it with
/// [`with_recalibration_provider`](Self::with_recalibration_provider).
/// All other mutable inputs come from [`ExecutionContext`].
#[derive(Clone)]
pub struct ScenarioEngine {
    /// Active configuration; its rounding mode is stamped into
    /// [`ApplicationReport::meta`].
    config: finstack_quant_core::config::FinstackConfig,
    /// Provider reused across calls belonging to one immutable batch.
    recalibration_provider: Arc<dyn RecalibrationProvider>,
}

impl Default for ScenarioEngine {
    fn default() -> Self {
        Self::with_config(finstack_quant_core::config::FinstackConfig::default())
    }
}

impl std::fmt::Debug for ScenarioEngine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScenarioEngine")
            .field("config", &self.config)
            .finish()
    }
}

impl ScenarioEngine {
    /// Create a new scenario engine with the default configuration and a
    /// fresh [`CachedRecalibrationProvider`].
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_scenarios::ScenarioEngine;
    ///
    /// let engine = ScenarioEngine::new();
    /// let _provider = engine.get_recalibration_provider();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scenario engine carrying the caller's active configuration.
    ///
    /// The configuration's rounding mode is stamped into
    /// [`ApplicationReport::meta`]. A fresh [`CachedRecalibrationProvider`]
    /// is attached, so solve-to-par ParCDS shocks and hazard time-roll replay
    /// work without further wiring.
    ///
    /// # Arguments
    ///
    /// * `config` - Active library configuration whose rounding mode is
    ///   recorded in every application report.
    #[must_use]
    pub fn with_config(config: finstack_quant_core::config::FinstackConfig) -> Self {
        Self {
            config,
            recalibration_provider: Arc::new(CachedRecalibrationProvider::default()),
        }
    }

    /// Replace the default quote-recalibration service with one shared across
    /// an immutable scenario batch.
    ///
    /// # Arguments
    ///
    /// * `provider` - Shared service used for subsequent quote-replay
    ///   operations on this engine.
    #[must_use]
    pub fn with_recalibration_provider(mut self, provider: Arc<dyn RecalibrationProvider>) -> Self {
        self.recalibration_provider = provider;
        self
    }

    /// The recalibration provider this engine uses: the default
    /// [`CachedRecalibrationProvider`] or the one attached with
    /// [`with_recalibration_provider`](Self::with_recalibration_provider).
    #[must_use]
    pub fn get_recalibration_provider(&self) -> &Arc<dyn RecalibrationProvider> {
        &self.recalibration_provider
    }

    /// Borrow the engine-owned configuration used for execution and reporting.
    pub(crate) fn get_config(&self) -> &finstack_quant_core::config::FinstackConfig {
        &self.config
    }

    /// Apply a scenario specification to the execution context.
    ///
    /// Operations are applied in this order:
    /// 0. Time roll-forward, if present
    /// 1. Market data (FX, equities, vol surfaces, curves, base correlation) —
    ///    adjacent [`MarketBump`] effects are batched. Collection-specific and
    ///    quote-replay effects flush preceding bumps before application.
    /// 2. Rate bindings update (if configured)
    /// 3. Statement forecast adjustments
    /// 4. Statement re-evaluation
    ///
    /// If a [`crate::spec::OperationSpec::TimeRollForward`] sets
    /// `apply_shocks = false`, the engine returns immediately after phase 0 and
    /// does not apply the remaining operations in `spec`.
    /// Advancing calibrated hazard curves requires a recalibration provider;
    /// a missing provider fails before changing the market or valuation date.
    /// A business-day adjustment to the opening date leaves the market unchanged.
    ///
    /// # Atomicity
    ///
    /// Hierarchy expansion (Phase −1), time roll-forward (Phase 0), and market
    /// operations (Phase 1) are staged against a private clone of `ctx.market`
    /// and a local `as_of` copy and committed back to `ctx` only after market
    /// execution succeeds; if selection, time roll, active-target binding, or
    /// any market shock fails, the caller's `ctx.market` and `ctx.as_of` remain
    /// unchanged. Downstream statement operations still execute sequentially
    /// after market commit.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `spec` contains instrument-scoped
    /// operations (instrument price or spread shocks, asset or prepay-default
    /// correlation shocks) and `ctx.instruments` is `None`; this check runs
    /// after spec validation and before any mutation. Returns validation
    /// errors for an invalid spec, unsupported operation data, hierarchy
    /// expansion failures, missing or wrong-kind active market objects, or
    /// hierarchy-targeted operations without an attached hierarchy.
    ///
    /// # Arguments
    ///
    /// * `spec` - Scenario operations to apply, including hierarchy targets.
    /// * `ctx` - Mutable market, optional statements, instruments, and as-of date.
    #[tracing::instrument(skip_all, fields(scenario_id = %spec.id))]
    pub fn apply(
        &self,
        spec: &ScenarioSpec,
        ctx: &mut ExecutionContext,
    ) -> Result<ApplicationReport> {
        spec.validate()?;
        if spec.mutates_instruments() && ctx.instruments.is_none() {
            return Err(crate::error::Error::validation(MISSING_INSTRUMENTS));
        }

        let mut applied = 0;
        let mut warnings: Vec<Warning> = Vec::new();
        let initial_as_of = ctx.as_of;
        let mut changes = ScenarioChangeManifest::default();

        let user_operations = spec.operations.len();

        let ExpansionOutcome {
            operations: expanded_ops,
            required_targets,
        } = expand_hierarchy_operations(&spec.operations, ctx.market, spec.resolution_mode)?;
        let expanded_operations = expanded_ops.len();

        let mut candidate_market = ctx.market.clone();
        let time_roll = {
            let mut staged = ExecutionContext {
                market: &mut candidate_market,
                model: ctx.model.as_deref_mut(),
                instruments: ctx.instruments.as_deref_mut(),
                rate_bindings: None,
                calendar: ctx.calendar,
                as_of: ctx.as_of,
            };

            let time_roll = if let Some((period, apply_shocks, roll_mode)) =
                expanded_ops.iter().find_map(|op| match op {
                    OperationSpec::TimeRollForward {
                        period,
                        apply_shocks,
                        roll_mode,
                    } => Some((period, *apply_shocks, *roll_mode)),
                    _ => None,
                }) {
                let _span = tracing::info_span!("phase_0_time_roll", period = %period).entered();
                let credit_ops = if apply_shocks {
                    expanded_ops.as_ref()
                } else {
                    &[]
                };
                let hazard_rolls = par_cds_hazard_rolls(credit_ops, staged.market)?;
                let roll_report = crate::adapters::time_roll::apply_time_roll_forward(
                    &mut staged,
                    period,
                    roll_mode,
                    &hazard_rolls,
                    &*self.recalibration_provider,
                )?;
                applied += 1;
                for (instrument_id, reason) in &roll_report.failed_instruments {
                    warnings.push(Warning::TimeRollInstrumentFailed {
                        instrument_id: instrument_id.clone(),
                        reason: reason.clone(),
                    });
                }
                changes.as_of_changed = staged.as_of != initial_as_of;
                changes.all_dirty |= changes.as_of_changed;
                if !apply_shocks {
                    let committed_as_of = staged.as_of;
                    drop(staged);
                    *ctx.market = candidate_market;
                    ctx.as_of = committed_as_of;
                    return Ok(ApplicationReport {
                        operations_applied: applied,
                        user_operations,
                        expanded_operations,
                        changes,
                        applied_shocks: Vec::new(),
                        warnings,
                        meta: Some(finstack_quant_core::config::results_meta(&self.config)),
                        time_roll: Some(roll_report),
                    });
                }
                Some(roll_report)
            } else {
                None
            };

            let mut pending_bumps: Vec<MarketBump> = Vec::new();
            let mut applied_shocks: Vec<AppliedShock> = Vec::new();

            // Retain each hazard's calibration market until that curve is rebuilt.
            let initial_market = Arc::new(staged.market.clone());
            let mut hazard_sources: indexmap::IndexMap<_, _> = staged
                .market
                .iter_curves()
                .filter(|(_, curve)| {
                    matches!(
                        curve,
                        finstack_quant_core::market_data::context::CurveStorage::Hazard(_)
                    )
                })
                .map(|(id, _)| (id.clone(), Arc::clone(&initial_market)))
                .collect();

            {
                let _span =
                    tracing::info_span!("phase_1_market", ops = expanded_operations).entered();
                let mut sink = EffectSink {
                    pending_bumps: &mut pending_bumps,
                    warnings: &mut warnings,
                    applied: &mut applied,
                    changes: &mut changes,
                    applied_shocks: &mut applied_shocks,
                };
                let mut idx = 0;
                while idx < expanded_ops.len() {
                    if let OperationSpec::TimeRollForward { .. } = &expanded_ops[idx] {
                        idx += 1;
                        continue;
                    }
                    if StatementOp::from_operation(&expanded_ops[idx]).is_some() {
                        idx += 1;
                        continue;
                    }

                    flush_pending_bumps(sink.pending_bumps, staged.market)?;

                    let env = HazardApplyEnv {
                        provider: &*self.recalibration_provider,
                        source_markets: Some(&hazard_sources),
                    };
                    let run_len = independent_replace_curve_run_len(&expanded_ops[idx..]);
                    let run = &expanded_ops[idx..idx + run_len];
                    let parallel_run = should_parallel_replace_curves(run);
                    let check_len = if parallel_run { run_len } else { 1 };
                    for target in required_targets
                        .get(idx..idx + check_len)
                        .into_iter()
                        .flatten()
                        .flatten()
                    {
                        verify_active_target(staged.market, target)?;
                    }
                    let processed = if parallel_run {
                        let batches = generate_replace_curve_effects_parallel(run, &staged, &env)?;
                        for (op, effects) in run.iter().zip(batches) {
                            apply_generated_effects(op, effects, &mut staged, &mut sink)?;
                        }
                        run_len
                    } else {
                        process_effects(&expanded_ops[idx], &mut staged, &env, &mut sink)?;
                        1
                    };
                    for op in &expanded_ops[idx..idx + processed] {
                        if let Some((curve_id, _)) = par_cds_ids(op) {
                            hazard_sources
                                .insert(curve_id.clone(), Arc::new(staged.market.clone()));
                        }
                    }
                    idx += processed;
                }

                flush_pending_bumps(sink.pending_bumps, staged.market)?;
            }

            let committed_as_of = staged.as_of;
            (time_roll, applied_shocks, committed_as_of)
        };
        let (time_roll, applied_shocks, committed_as_of) = time_roll;
        *ctx.market = candidate_market;
        ctx.as_of = committed_as_of;

        let has_rate_bindings = ctx.rate_bindings.is_some();
        let deferred_stmts: Vec<StatementOp<'_>> = expanded_ops
            .iter()
            .filter_map(StatementOp::from_operation)
            .collect();

        if let Some(bindings) = &ctx.rate_bindings {
            let _span = tracing::info_span!("phase_2_rate_bindings").entered();
            for (node_id, binding) in bindings {
                if binding.node_id != *node_id {
                    return Err(crate::error::Error::Validation(format!(
                        "Rate binding node_id mismatch: map key '{node_id}' does not equal \
                         binding.node_id '{}'. The map key is authoritative for routing; \
                         rebuild the binding with node_id set to the map key.",
                        binding.node_id
                    )));
                }

                let model = statement_model(&mut ctx.model, "rate binding")?;
                apply_rate_binding(binding, model, ctx.market, ctx.calendar, &mut warnings);
            }
        }

        let mut applied_stmt_ops = 0usize;
        {
            let _span = tracing::info_span!("phase_3_statements").entered();
            for statement_op in deferred_stmts {
                match statement_op {
                    StatementOp::RateBinding(binding) => {
                        if let Some(rb) = &mut ctx.rate_bindings {
                            rb.insert(binding.node_id.clone(), binding.clone());
                        }
                        let model = statement_model(&mut ctx.model, "rate binding")?;
                        if apply_rate_binding(
                            binding,
                            model,
                            ctx.market,
                            ctx.calendar,
                            &mut warnings,
                        ) {
                            applied += 1;
                            applied_stmt_ops += 1;
                        }
                    }
                    StatementOp::ForecastPercent { node_id, pct } => {
                        let model = statement_model(&mut ctx.model, "statement forecast percent")?;
                        record_forecast_result(
                            crate::adapters::statements::apply_forecast_percent(
                                model,
                                node_id.as_str(),
                                pct,
                            ),
                            node_id,
                            "forecast_percent",
                            &mut applied,
                            &mut applied_stmt_ops,
                            &mut warnings,
                        );
                    }
                    StatementOp::ForecastAssign { node_id, value } => {
                        let model = statement_model(&mut ctx.model, "statement forecast assign")?;
                        record_forecast_result(
                            crate::adapters::statements::apply_forecast_assign(
                                model,
                                node_id.as_str(),
                                value,
                            ),
                            node_id,
                            "forecast_assign",
                            &mut applied,
                            &mut applied_stmt_ops,
                            &mut warnings,
                        );
                    }
                }
            }
        }

        if applied_stmt_ops > 0 || has_rate_bindings {
            let _span = tracing::info_span!("phase_4_reevaluate").entered();
            let model = statement_model(&mut ctx.model, "statement re-evaluation")?;
            match crate::adapters::statements::reevaluate_model(model) {
                Ok(eval_warnings) => warnings.extend(eval_warnings),
                Err(e) => warnings.push(Warning::ModelReevaluationFailed {
                    reason: e.to_string(),
                }),
            }
        }

        changes.as_of_changed = ctx.as_of != initial_as_of;
        changes.all_dirty |= changes.as_of_changed;

        Ok(ApplicationReport {
            operations_applied: applied,
            user_operations,
            expanded_operations,
            changes,
            applied_shocks,
            warnings,
            meta: Some(finstack_quant_core::config::results_meta(&self.config)),
            time_roll,
        })
    }
}
