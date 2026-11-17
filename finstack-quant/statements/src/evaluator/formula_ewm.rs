//! Exponentially-weighted moving statistics: `ewm_mean`, `ewm_std`, `ewm_var`.
//!
//! The weights decay once per canonical plan slot from the model start
//! through the target (`adjust=False`, `ignore_na=False` in pandas terms). A
//! slot without a value ages the weights and carries the previous estimate
//! without adding an observation.

use crate::error::Result;
use crate::evaluator::cell::UnavailableReason;
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::formula::{eval_error, evaluate_formula, evaluate_value, require_args};
use crate::evaluator::formula_aggregates::{Series, prefix_slots};
use crate::evaluator::formula_timeseries::at;
use crate::evaluator::value::Value;
use finstack_quant_core::expr::{Expr, Function};

const MISSING: Value = Value::Unavailable(UnavailableReason::MissingInput);

/// Recursive moments over one float per slot, `NaN` for a gap. Returns the
/// mean, the biased variance, the sum of squared normalized weights and the
/// number of observations; `None` without an observation.
fn ewm_weighted_moments(values: &[f64], alpha: f64) -> Option<(f64, f64, f64, usize)> {
    let mut mean = None;
    let mut variance = 0.0;
    let mut weight_sq = 1.0;
    let mut old_weight = 1.0;
    let mut count = 0;
    for value in values {
        let Some(previous) = mean else {
            if value.is_finite() {
                mean = Some(*value);
                count = 1;
            }
            continue;
        };
        old_weight *= 1.0 - alpha;
        if !value.is_finite() {
            continue;
        }
        let retained = old_weight / (old_weight + alpha);
        let added = alpha / (old_weight + alpha);
        let delta = value - previous;
        mean = Some(previous + added * delta);
        variance = retained * variance + retained * added * delta * delta;
        weight_sq = retained * retained * weight_sq + added * added;
        old_weight = 1.0;
        count += 1;
    }
    mean.map(|mean| (mean, variance, weight_sq, count))
}

/// The decay parameter: a finite number in `(0, 1]`.
fn alpha(
    func: &str,
    expr: &Expr,
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    let alpha = evaluate_formula(expr, context, node_id)?;
    if !(alpha > 0.0 && alpha <= 1.0) {
        return Err(eval_error(
            node_id,
            format!("{func} alpha must be in (0, 1]"),
        ));
    }
    Ok(alpha)
}

/// The series in every plan slot from the model start through the target.
fn slot_series(
    expr: &Expr,
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Series> {
    let slots = prefix_slots(context)?;
    Series::collect(expr, &slots, context, node_id)
}

pub(crate) fn eval_ewm_mean(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    require_args("ewm_mean", args, 2, node_id)?;
    let alpha = alpha("ewm_mean", &args[1], context, node_id)?;
    let series = slot_series(&args[0], context, node_id)?;
    match ewm_weighted_moments(&series.floats(), alpha) {
        Some((mean, _, _, _)) => series.lift(mean, "ewm_mean()", at(context, node_id)),
        None => Ok(MISSING),
    }
}

/// Evaluate `ewm_std` or `ewm_var`.
///
/// The optional third argument toggles the squared-weight correction
/// `1 / (1 − Σŵ²)` over the normalized weights (RiskMetrics TD4; pandas
/// `bias=False`). It is on by default; a Boolean, or exactly `0` or `1`,
/// selects it explicitly. The corrected statistic needs two observations.
pub(crate) fn eval_ewm_std_or_var(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    if args.len() < 2 || args.len() > 3 {
        return Err(eval_error(
            node_id,
            format!("{func}() requires 2 or 3 arguments (series, alpha, [unbiased])"),
        ));
    }
    let alpha = alpha("ewm", &args[1], context, node_id)?;
    let corrected = match args.get(2) {
        None => true,
        Some(toggle) => {
            let invalid = || {
                eval_error(
                    node_id,
                    format!("{func}() unbiased toggle must be a Boolean, 0 or 1"),
                )
            };
            match evaluate_value(toggle, context, node_id)? {
                Value::Bool(flag) => flag,
                unavailable @ Value::Unavailable(_) => return Ok(unavailable),
                Value::Scalar(flag) | Value::Number(flag) => match flag {
                    0.0 => false,
                    1.0 => true,
                    _ => return Err(invalid()),
                },
                Value::Money(_) => return Err(invalid()),
            }
        }
    };
    let series = slot_series(&args[0], context, node_id)?;
    let Some((_, biased, weight_sq, observations)) = ewm_weighted_moments(&series.floats(), alpha)
    else {
        return Ok(MISSING);
    };
    let variance = if corrected {
        let denominator = 1.0 - weight_sq;
        if observations < 2 || denominator <= 0.0 {
            return Ok(MISSING);
        }
        biased / denominator
    } else {
        biased
    };
    let here = at(context, node_id);
    match func {
        Function::EwmVar => Ok(Value::Scalar(here.finite(variance, "ewm_var()")?)),
        Function::EwmStd => series.lift(variance.sqrt(), "ewm_std()", here),
        other => Err(eval_error(
            node_id,
            format!(
                "Function {:?} is not an exponentially weighted std/var function",
                other
            ),
        )),
    }
}
