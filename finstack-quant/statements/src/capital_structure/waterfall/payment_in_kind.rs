//! PIK toggle evaluation, target scoping, and hysteresis.

use crate::capital_structure::cashflows::CashflowBreakdown;
use crate::capital_structure::state::CapitalStructureState;
use crate::capital_structure::waterfall_spec::PikToggleSpec;
use crate::error::Result;
use crate::evaluator::EvaluationContext;
use indexmap::IndexMap;
use std::collections::HashSet;

use super::eval_value_or_formula;

/// Evaluate PIK toggle conditions and return whether PIK should be enabled.
pub(super) fn evaluate_pik_toggle(
    context: &EvaluationContext,
    pik_spec: &PikToggleSpec,
) -> Result<bool> {
    let metric_value = eval_value_or_formula(context, &pik_spec.liquidity_metric)?;
    Ok(metric_value < pik_spec.threshold)
}

/// Check whether PIK mode is currently active for an instrument.
pub(super) fn is_pik_enabled(state: &CapitalStructureState, instrument_id: &str) -> bool {
    state.pik_mode.get(instrument_id).copied().unwrap_or(false)
}

/// Apply PIK mode transitions to `state` for the active period.
///
/// Encapsulates the per-instrument hysteresis rule:
/// - Honour `min_periods_in_pik`: an instrument that *was* PIK and is being
///   asked to switch off this period must remain PIK if it has not yet
///   accrued enough consecutive periods. This prevents a single noisy
///   liquidity reading from flipping PIK off mid-cycle.
/// - Reset the consecutive-PIK counter to zero whenever PIK turns off.
///
/// Extracted from `execute_waterfall` so the orchestrator stays focused on
/// payment-priority application; this helper owns all PIK state mutation.
pub(super) fn apply_pik_transitions(
    state: &mut CapitalStructureState,
    contractual_flows: &IndexMap<String, CashflowBreakdown>,
    enable_pik: bool,
    pik_targets: Option<&HashSet<String>>,
    min_periods_in_pik: usize,
) {
    for instrument_id in contractual_flows.keys() {
        let should_apply = pik_targets
            .map(|set| set.contains(instrument_id))
            .unwrap_or(true);
        if !should_apply {
            continue;
        }
        let periods_active = state
            .pik_periods_active
            .get(instrument_id.as_str())
            .copied()
            .unwrap_or(0);
        let currently_pik = state
            .pik_mode
            .get(instrument_id.as_str())
            .copied()
            .unwrap_or(false);
        let effective_pik = if currently_pik && !enable_pik && periods_active < min_periods_in_pik {
            true
        } else {
            enable_pik
        };
        state
            .pik_mode
            .insert(instrument_id.to_string(), effective_pik);
        state.pik_periods_active.insert(
            instrument_id.to_string(),
            if effective_pik { periods_active + 1 } else { 0 },
        );
    }
}
