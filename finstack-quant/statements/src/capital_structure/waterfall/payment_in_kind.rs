//! PIK toggle request: the liquidity metric against its threshold.

use crate::capital_structure::waterfall_spec::PikToggleSpec;
use crate::error::Result;
use crate::evaluator::EvaluationContext;

use super::eval_number;

/// Evaluate the toggle and return whether the period requests PIK.
///
/// A metric below the threshold requests PIK; a metric at or above it
/// requests the base coupon terms, each in the metric's own units. A metric
/// with no value is an error.
pub(super) fn evaluate_pik_toggle(
    context: &EvaluationContext,
    pik_spec: &PikToggleSpec,
    warnings: &mut Vec<crate::evaluator::EvalWarning>,
) -> Result<bool> {
    let metric_value = eval_number(context, &pik_spec.liquidity_metric, warnings)?;
    Ok(metric_value < pik_spec.threshold)
}
