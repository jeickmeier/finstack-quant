//! Exponentially-weighted moving statistics: `ewm_mean`, `ewm_std`, `ewm_var`.
//!
//! All three functions share the same "collect chronological values for the
//! series expression" preamble and pandas-compatible semantics (`adjust=False`,
//! `ignore_na=False`): a NaN observation is skipped, but the decay weight
//! still advances across the gap, exactly as pandas weights observations by
//! absolute position. Splitting them out of `formula.rs` keeps the main
//! dispatcher smaller while grouping semantically related code.

use crate::error::Result;
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::formula::{
    collect_expression_values_sorted, eval_error, evaluate_formula, require_args,
};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::expr::{Expr, ExprNode, Function};
use finstack_quant_core::math::ZERO_TOLERANCE;

/// Recursive `adjust=False, ignore_na=False` moments. Missing slots decay the
/// old weight; each observation normalizes the old distribution and new weight
/// before the next update. Squared weights track the unbiased correction.
fn ewm_weighted_moments(values: &[(PeriodId, f64)], alpha: f64) -> Option<(f64, f64, f64, usize)> {
    let mut mean = None;
    let mut variance = 0.0;
    let mut weight_sq = 1.0;
    let mut old_weight = 1.0;
    let mut count = 0;
    for (_, value) in values {
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

/// Validate an EWM decay parameter: pandas requires `0 < alpha <= 1`.
/// `alpha = 0` would freeze the mean at the oldest value and zero the
/// variance bias-correction denominator.
fn validate_alpha(alpha: f64, func_name: &str, node_id: Option<&str>) -> Result<()> {
    if !(alpha > 0.0 && alpha <= 1.0) {
        return Err(eval_error(
            node_id,
            format!("{func_name} alpha must be in (0, 1]"),
        ));
    }
    Ok(())
}

/// Collect the chronological series for an EWM function's first argument.
///
/// Reuses the shared expression collector, keeping non-finite slots for
/// positional decay and trimming only trailing non-finite observations.
fn collect_series(
    expr: &Expr,
    context: &EvaluationContext,
    node_id: Option<&str>,
) -> Result<Vec<(PeriodId, f64)>> {
    let sorted = collect_expression_values_sorted(expr, context, node_id)?;
    // A bare column uses only the evaluated current value. A same-period
    // history row must not stand in for a node that is not evaluated yet.
    let include_current = match &expr.node {
        ExprNode::Column(name) => context.get_value(name).is_ok(),
        _ => true,
    };
    let mut values: Vec<(PeriodId, f64)> = sorted
        .iter()
        .filter(|(period, _)| include_current || **period != context.period_id)
        .map(|(period, value)| (*period, *value))
        .collect();
    while values.last().is_some_and(|(_, v)| !v.is_finite()) {
        values.pop();
    }
    Ok(values)
}

pub(crate) fn eval_ewm_mean(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_args("ewm_mean", args, 2, node_id)?;

    let alpha = evaluate_formula(&args[1], context, node_id)?;
    validate_alpha(alpha, "ewm_mean", node_id)?;

    let values = collect_series(&args[0], context, node_id)?;

    match ewm_weighted_moments(&values, alpha) {
        Some((mean, _, _, _)) => Ok(mean),
        None => Ok(f64::NAN),
    }
}

/// Evaluate `ewm_std` or `ewm_var`. `func` determines whether to return the
/// variance directly or its square root.
///
/// Arguments:
/// - 2 args: `(series, alpha)` — bias-corrected (pandas `adjust=False, bias=False`)
/// - 3 args: `(series, alpha, unbiased)` — bias correction enabled when
///   `unbiased != 0.0` (pandas `bias=False`); `0.0` returns the biased
///   (`bias=True`) variance. Note this flag is pandas' `bias` toggle, not its
///   `adjust` parameter: the weighting is always the `adjust=False` recursion.
///
/// The unbiased correction is `1 / (1 − Σŵ²)` over the normalized weights
/// (RiskMetrics TD4 convention; pandas `adjust=False, bias=False`). It
/// converges to the standard Bessel correction `n/(n-1)` as alpha → 0
/// (equal weighting).
pub(crate) fn eval_ewm_std_or_var(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    if args.len() < 2 || args.len() > 3 {
        return Err(eval_error(
            node_id,
            format!("{func}() requires 2 or 3 arguments (series, alpha, [unbiased])"),
        ));
    }

    let alpha = evaluate_formula(&args[1], context, node_id)?;
    validate_alpha(alpha, "ewm", node_id)?;

    // Default to bias-corrected output (pandas `adjust=False, bias=False`).
    let unbiased = if args.len() == 3 {
        evaluate_formula(&args[2], context, node_id)? != 0.0
    } else {
        true
    };

    let values = collect_series(&args[0], context, node_id)?;

    let Some((_, biased_var, sum_w2, n_obs)) = ewm_weighted_moments(&values, alpha) else {
        return Ok(f64::NAN);
    };
    if n_obs < 2 && unbiased {
        return Ok(f64::NAN);
    }

    let mut ewm_var = biased_var;
    if unbiased {
        let denom = 1.0 - sum_w2;
        if denom.abs() > ZERO_TOLERANCE {
            ewm_var /= denom;
        } else {
            return Ok(f64::NAN);
        }
    }

    match func {
        Function::EwmVar => Ok(ewm_var),
        Function::EwmStd => Ok(ewm_var.sqrt()),
        other => Err(eval_error(
            node_id,
            format!(
                "Function {:?} is not an exponentially weighted std/var function",
                other
            ),
        )),
    }
}
