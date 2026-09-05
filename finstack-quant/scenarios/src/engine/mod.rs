//! Deterministic scenario execution engine.
//!
//! The engine glues together adapters from this crate to compose multiple
//! [`ScenarioSpec`] definitions and apply them to
//! a mutable [`ExecutionContext`]. Its responsibilities are:
//! - enforce a repeatable ordering of operations
//! - dispatch each `OperationSpec` variant to the appropriate adapter function
//!   via a centralized exhaustive `match`
//! - flush market bumps **per operation** (not once per scenario) so
//!   sequential adapters see a fully-applied prior state
//! - collect reporting metadata about how many operations ran and any
//!   warnings produced during execution

mod effects;
mod hierarchy;
mod instrument_shocks;
mod types;

#[cfg(test)]
mod tests;

pub(crate) use types::HazardApplyEnv;
pub use types::{
    ApplicationEnvelope, ApplicationReport, ExecutionContext, RollForwardReport,
    ScenarioChangeManifest, ScenarioMarketTarget,
};

use crate::adapters::traits::ScenarioEffect;
use crate::error::Result;
use crate::spec::{OperationSpec, RateBindingSpec, ScenarioSpec};
use crate::warning::Warning;
use effects::{
    apply_generated_effects, flush_pending_bumps, generate_replace_curve_effects_parallel,
    independent_replace_curve_run_len, process_effects, should_parallel_replace_curves, EffectSink,
};
use finstack_quant_core::dates::HolidayCalendar;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_statements::FinancialModelSpec;
use finstack_quant_valuations::recalibration::RecalibrationProvider;
use hierarchy::{expand_hierarchy_operations, ExpansionOutcome};
use std::sync::Arc;

/// Apply one rate binding to `model`, recording a warning instead of a hard
/// error when the update produces no forecast values or fails.
///
/// Shared by the engine's Phase 2 (context-configured bindings) and Phase 3
/// (`ScenarioEffect::RateBinding` operations) so the update-and-warn logic is
/// written once. Returns `true` when [`update_rate_from_binding`] returned
/// `Ok(_)` (regardless of whether values were produced), `false` on error.
///
/// [`update_rate_from_binding`]: crate::adapters::statements::update_rate_from_binding
fn apply_rate_binding(
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

fn results_stamp(
    config: &finstack_quant_core::config::FinstackConfig,
) -> Option<finstack_quant_core::config::ResultsMeta> {
    Some(finstack_quant_core::config::results_meta(config))
}

/// Orchestrates the deterministic application of a [`ScenarioSpec`].
///
/// The engine is intentionally lightweight: it owns an immutable
/// [`FinstackConfig`](finstack_quant_core::config::FinstackConfig) (used to stamp
/// the active rounding policy into reports) and an optional shared
/// quote-recalibration provider. All other mutable inputs are supplied via
/// [`ExecutionContext`].
#[derive(Default, Clone)]
pub struct ScenarioEngine {
    /// Active configuration; its rounding mode is stamped into
    /// [`ApplicationReport::meta`].
    config: finstack_quant_core::config::FinstackConfig,
    /// Optional provider reused across calls belonging to one immutable batch.
    recalibration_provider: Option<Arc<dyn RecalibrationProvider>>,
}

impl std::fmt::Debug for ScenarioEngine {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScenarioEngine")
            .field("config", &self.config)
            .field(
                "has_recalibration_provider",
                &self.recalibration_provider.is_some(),
            )
            .finish()
    }
}

impl ScenarioEngine {
    /// Create a new scenario engine with the default [`FinstackConfig`](finstack_quant_core::config::FinstackConfig).
    ///
    /// # Examples
    /// ```rust
    /// use finstack_quant_scenarios::ScenarioEngine;
    ///
    /// let engine = ScenarioEngine::new();
    /// let other = ScenarioEngine::default();
    /// assert_eq!(format!("{:?}", engine), format!("{:?}", other));
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scenario engine carrying the caller's active configuration.
    ///
    /// The configuration's rounding mode is stamped into
    /// [`ApplicationReport::meta`] so reports reflect the policy
    /// actually in force rather than the library default.
    ///
    /// # Arguments
    ///
    /// * `config` - Active library configuration whose rounding mode is
    ///   recorded in every application report.
    #[must_use]
    pub fn with_config(config: finstack_quant_core::config::FinstackConfig) -> Self {
        Self {
            config,
            recalibration_provider: None,
        }
    }

    /// Inject the quote-recalibration service for one immutable scenario batch.
    ///
    /// # Arguments
    ///
    /// * `provider` - Shared service used for subsequent quote-replay
    ///   operations on this engine.
    #[must_use]
    pub fn with_recalibration_provider(mut self, provider: Arc<dyn RecalibrationProvider>) -> Self {
        self.recalibration_provider = Some(provider);
        self
    }

    /// The recalibration provider attached with
    /// [`with_recalibration_provider`](Self::with_recalibration_provider), if any.
    #[must_use]
    pub fn recalibration_provider(&self) -> Option<&Arc<dyn RecalibrationProvider>> {
        self.recalibration_provider.as_ref()
    }

    /// Strict composition: returns an error at compose time when the
    /// concatenated operations would be rejected at apply time.
    ///
    /// Delegates to [`ScenarioSpec::compose`]; kept as a method so existing
    /// callers holding a [`ScenarioEngine`] do not need a throwaway instance
    /// replaced. Production callers should prefer
    /// [`ScenarioSpec::compose`] directly.
    ///
    /// # Errors
    ///
    /// Returns a validation error if `scenarios` contain conflicting
    /// `hazard_bump_mode` values or more than one time-roll operation. Other
    /// conflicts remain in the composed spec and are validated when
    /// [`Self::apply`] is called.
    ///
    /// # Arguments
    ///
    /// * `scenarios` - Scenario specifications to merge in ascending priority
    ///   order. Every non-empty input must use the same `hazard_bump_mode`;
    ///   conflicting modes are rejected before composition.
    pub fn try_compose(
        &self,
        scenarios: Vec<ScenarioSpec>,
    ) -> std::result::Result<ScenarioSpec, crate::error::Error> {
        ScenarioSpec::compose(scenarios)
    }

    /// Apply a scenario specification to the execution context.
    ///
    /// Operations are applied in this order:
    /// 0. Time roll-forward, if present
    /// 1. Market data (FX, equities, vol surfaces, curves, base correlation) — all
    ///    [`MarketBump`] effects accumulated during this phase are applied to the
    ///    context in a single batched market bump call.
    /// 2. Rate bindings update (if configured)
    /// 3. Statement forecast adjustments
    /// 4. Statement re-evaluation
    ///
    /// If a [`crate::spec::OperationSpec::TimeRollForward`] sets
    /// `apply_shocks = false`, the engine returns immediately after phase 0 and
    /// does not apply the remaining operations in `spec`.
    ///
    /// # Atomicity
    ///
    /// Application is **not atomic**: operations mutate `ctx.market` (and the
    /// statement model) in place as they execute. If a later operation fails —
    /// for example a curve id that does not exist in the market — the engine
    /// returns `Err` with all earlier operations already applied and no
    /// rollback. Callers that need all-or-nothing semantics should apply the
    /// scenario to a clone of the market context and swap it in on success
    /// (the Python and WASM bindings do exactly this by operating on
    /// deserialized copies).
    ///
    /// # Errors
    ///
    /// Returns validation errors for an invalid spec, unsupported operation
    /// data, missing market objects, or hierarchy-targeted operations without
    /// an attached hierarchy. Because execution is not atomic, an error can
    /// follow successful mutation by earlier operations.
    ///
    /// # Arguments
    ///
    /// * `spec` - Validated specification defining the requested operation.
    /// * `ctx` - Market or evaluation context supplying dependencies required by the calculation.
    #[tracing::instrument(skip_all, fields(scenario_id = %spec.id))]
    pub fn apply(
        &self,
        spec: &ScenarioSpec,
        ctx: &mut ExecutionContext,
    ) -> Result<ApplicationReport> {
        // Validate up-front so malformed specs cannot reach adapters. FFI
        // bindings (Python, WASM) deserialize JSON straight into a spec and
        // call this entry point without their own validation pass.
        spec.validate()?;

        let mut applied = 0;
        let mut warnings: Vec<Warning> = Vec::new();
        let initial_as_of = ctx.as_of;
        let mut changes = ScenarioChangeManifest::default();

        let user_operations = spec.operations.len();

        // Phase -1: Expand hierarchy-targeted operations to direct operations.
        // Errors fast if the spec contains hierarchy ops but no hierarchy is
        // attached to the market context. Hierarchy targets that resolve to
        // zero curves emit a `Warning::HierarchyNoMatch` so the caller can
        // detect the unintended no-op.
        let ExpansionOutcome {
            operations: expanded_ops,
            warnings: expansion_warnings,
        } = expand_hierarchy_operations(&spec.operations, ctx.market, spec.resolution_mode)?;
        let expanded_operations = expanded_ops.len();
        warnings.extend(expansion_warnings);

        // Phase 0: Time Roll Forward (`spec.validate()` already enforced the
        // at-most-one invariant; no need to re-count here.)
        let mut time_roll: Option<RollForwardReport> = None;
        for op in expanded_ops.iter() {
            if let OperationSpec::TimeRollForward {
                period,
                apply_shocks,
                roll_mode,
            } = op
            {
                let _span = tracing::info_span!("phase_0_time_roll", period = %period).entered();
                let mut hazard_rolls = Vec::new();
                if *apply_shocks && spec.hazard_bump_mode == crate::HazardBumpMode::SolveToPar {
                    for operation in expanded_ops.iter() {
                        if let OperationSpec::CurveParallelBp {
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
                        } = operation
                        {
                            if !hazard_rolls.iter().any(|(id, _)| id == curve_id) {
                                let (discount_id, _) =
                                    crate::adapters::curves::resolve_discount_curve_id(
                                        ctx.market,
                                        discount_curve_id.as_ref(),
                                        Some(curve_id),
                                    )?;
                                hazard_rolls.push((curve_id.clone(), discount_id));
                            }
                        }
                    }
                }
                let roll_report = crate::adapters::time_roll::apply_time_roll_forward_with_credit(
                    ctx,
                    period,
                    *roll_mode,
                    &hazard_rolls,
                    self.recalibration_provider.as_deref(),
                )?;
                applied += 1;

                // Valuation failures during the roll must not vanish: surface
                // each as a structured warning so callers that only inspect
                // the ApplicationReport still see them.
                for (instrument_id, reason) in &roll_report.failed_instruments {
                    warnings.push(Warning::TimeRollInstrumentFailed {
                        instrument_id: instrument_id.clone(),
                        reason: reason.clone(),
                    });
                }

                let stop_after_roll = !*apply_shocks;
                time_roll = Some(roll_report);
                changes.as_of_changed = ctx.as_of != initial_as_of;
                changes.all_dirty |= changes.as_of_changed;

                if stop_after_roll {
                    return Ok(ApplicationReport {
                        operations_applied: applied,
                        user_operations,
                        expanded_operations,
                        changes,
                        warnings,
                        meta: results_stamp(&self.config),
                        time_roll,
                    });
                }
            }
        }

        let has_rate_bindings = ctx.rate_bindings.is_some();
        let mut deferred_stmts = Vec::new();
        let mut pending_bumps: Vec<MarketBump> = Vec::new();

        // A direct rate bump changes replay dependencies without changing the
        // hazard curve. Retain the calibration market for each hazard until
        // that curve itself is rebuilt, including after a horizon requote.
        let initial_market = Arc::new(ctx.market.clone());
        let mut hazard_sources: indexmap::IndexMap<_, _> = ctx
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

        // Phase 1: Generate effects and split into market bumps (intra-op
        // batched), curve replacements, instrument shocks, and deferred
        // statement ops. Bumps from the previous iteration are flushed before
        // generating effects for the next op so adapters always observe a
        // fully-applied prior-op market state — this preserves the sequential
        // semantics that downstream cross-curve calibrations depend on.
        //
        // Consecutive independent ParCDS / inflation replacements (distinct
        // curve ids) may be generated in parallel after that flush. Dependent
        // pairs such as discount-then-hazard stay sequential.
        {
            let _span = tracing::info_span!("phase_1_market", ops = expanded_operations).entered();
            let mut sink = EffectSink {
                pending_bumps: &mut pending_bumps,
                deferred_stmts: &mut deferred_stmts,
                warnings: &mut warnings,
                applied: &mut applied,
                changes: &mut changes,
            };
            let mut idx = 0;
            while idx < expanded_ops.len() {
                if let OperationSpec::TimeRollForward { .. } = &expanded_ops[idx] {
                    idx += 1;
                    continue; // handled in Phase 0
                }

                // Apply any bumps queued by the previous iteration so the
                // adapter's `ctx.market` reads reflect everything done so far.
                flush_pending_bumps(sink.pending_bumps, ctx.market)?;

                let env = HazardApplyEnv {
                    mode: spec.hazard_bump_mode,
                    provider: self.recalibration_provider.as_deref(),
                    source_markets: Some(&hazard_sources),
                };
                let run_len = independent_replace_curve_run_len(&expanded_ops[idx..]);
                let run = &expanded_ops[idx..idx + run_len];
                let processed = if should_parallel_replace_curves(run) {
                    let batches = generate_replace_curve_effects_parallel(run, ctx, &env)?;
                    for (op, effects) in run.iter().zip(batches) {
                        apply_generated_effects(op, effects, ctx, &mut sink)?;
                    }
                    run_len
                } else {
                    process_effects(&expanded_ops[idx], ctx, &env, &mut sink)?;
                    1
                };
                for op in &expanded_ops[idx..idx + processed] {
                    if let OperationSpec::CurveParallelBp {
                        curve_kind: crate::CurveKind::ParCDS,
                        curve_id,
                        ..
                    }
                    | OperationSpec::CurveNodeBp {
                        curve_kind: crate::CurveKind::ParCDS,
                        curve_id,
                        ..
                    } = op
                    {
                        hazard_sources.insert(curve_id.clone(), Arc::new(ctx.market.clone()));
                    }
                }
                idx += processed;
            }

            // Flush any remaining bumps before moving on to statements.
            flush_pending_bumps(sink.pending_bumps, ctx.market)?;
        }

        // Phase 2: Rate bindings update (from context configuration).
        //
        // The map key is authoritative for routing; mismatched binding.node_id
        // is a hard error so the caller fixes the binding upstream rather than
        // discovering a silent rewrite later.
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

                let Some(model) = ctx.model.as_deref_mut() else {
                    return Err(crate::error::Error::missing_statement_model("rate binding"));
                };
                apply_rate_binding(binding, model, ctx.market, ctx.calendar, &mut warnings);
            }
        }

        // Phase 3: Statement Operations (Deferred)
        let mut applied_stmt_ops = 0usize;
        {
            let _span = tracing::info_span!("phase_3_statements").entered();
            for effect in deferred_stmts {
                match effect {
                    ScenarioEffect::RateBinding { binding } => {
                        if let Some(rb) = &mut ctx.rate_bindings {
                            rb.insert(binding.node_id.clone(), binding.clone());
                        }
                        let Some(model) = ctx.model.as_deref_mut() else {
                            return Err(crate::error::Error::missing_statement_model(
                                "rate binding",
                            ));
                        };
                        if apply_rate_binding(
                            &binding,
                            model,
                            ctx.market,
                            ctx.calendar,
                            &mut warnings,
                        ) {
                            applied += 1;
                            applied_stmt_ops += 1;
                        }
                    }
                    ScenarioEffect::StmtForecastPercent { node_id, pct } => {
                        let Some(model) = ctx.model.as_deref_mut() else {
                            return Err(crate::error::Error::missing_statement_model(
                                "statement forecast percent",
                            ));
                        };
                        match crate::adapters::statements::apply_forecast_percent(
                            model,
                            node_id.as_str(),
                            pct,
                        ) {
                            Ok(true) => {
                                applied += 1;
                                applied_stmt_ops += 1;
                            }
                            Ok(false) => warnings.push(Warning::StatementNodeNoValues {
                                node_id: node_id.as_str().to_string(),
                                op: "forecast_percent".to_string(),
                            }),
                            Err(e) => warnings.push(Warning::StatementOpFailed {
                                node_id: node_id.as_str().to_string(),
                                op: "forecast_percent".to_string(),
                                reason: e.to_string(),
                            }),
                        }
                    }
                    ScenarioEffect::StmtForecastAssign { node_id, value } => {
                        let Some(model) = ctx.model.as_deref_mut() else {
                            return Err(crate::error::Error::missing_statement_model(
                                "statement forecast assign",
                            ));
                        };
                        match crate::adapters::statements::apply_forecast_assign(
                            model,
                            node_id.as_str(),
                            value,
                            None,
                        ) {
                            Ok(true) => {
                                applied += 1;
                                applied_stmt_ops += 1;
                            }
                            Ok(false) => warnings.push(Warning::StatementNodeNoValues {
                                node_id: node_id.as_str().to_string(),
                                op: "forecast_assign".to_string(),
                            }),
                            Err(e) => warnings.push(Warning::StatementOpFailed {
                                node_id: node_id.as_str().to_string(),
                                op: "forecast_assign".to_string(),
                                reason: e.to_string(),
                            }),
                        }
                    }
                    _ => {}
                }
            }
        }

        // Phase 4: Re-evaluate statements only if statement work was performed.
        if applied_stmt_ops > 0 || has_rate_bindings {
            let _span = tracing::info_span!("phase_4_reevaluate").entered();
            let Some(model) = ctx.model.as_deref_mut() else {
                return Err(crate::error::Error::missing_statement_model(
                    "statement re-evaluation",
                ));
            };
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
            warnings,
            meta: results_stamp(&self.config),
            time_roll,
        })
    }
}
