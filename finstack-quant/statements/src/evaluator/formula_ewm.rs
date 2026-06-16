//! Exponentially-weighted moving statistics: `ewm_mean`, `ewm_std`, `ewm_var`.
//!
//! All three functions share the same "collect chronological values for a
//! column" preamble and pandas-compatible bias correction semantics. Splitting
//! them out of `formula.rs` keeps the main dispatcher smaller while grouping
//! semantically related code.

use crate::error::Result;
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::formula::{eval_error, evaluate_expr, require_args};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::expr::{Expr, ExprNode, Function};
use finstack_quant_core::math::ZERO_TOLERANCE;

/// Collect chronologically-sorted (period, value) pairs for a column reference,
/// including the current period's value if set. Returns `None` if `args[0]` is
/// not a column reference.
///
/// Only periods at or before the context's evaluation period are included so
/// lagged evaluation cannot look ahead, and non-finite values are skipped to
/// match the retain-finite policy used by the other aggregates.
fn collect_column_series(column_name: &str, context: &EvaluationContext) -> Vec<(PeriodId, f64)> {
    let mut values = Vec::with_capacity(context.historical_results.len() + 1);
    for (period_id, period_results) in context.historical_results.iter() {
        if *period_id >= context.period_id {
            continue;
        }
        if let Some(value) = period_results.get(column_name) {
            if value.is_finite() {
                values.push((*period_id, *value));
            }
        }
    }
    if let Ok(current) = context.get_value(column_name) {
        if current.is_finite() {
            values.push((context.period_id, current));
        }
    }
    values.sort_by_key(|(period, _)| *period);
    values
}

/// Extract the column-reference argument or fail with a standard error.
fn column_ref_or_err<'a>(
    args: &'a [Expr],
    func_name: &str,
    node_id: Option<&str>,
) -> Result<&'a str> {
    if let ExprNode::Column(name) = &args[0].node {
        Ok(name.as_str())
    } else {
        Err(eval_error(
            node_id,
            format!("{func_name}() requires a column reference"),
        ))
    }
}

pub(crate) fn eval_ewm_mean(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_args("ewm_mean", args, 2, node_id)?;

    let alpha = evaluate_expr(&args[1], context, node_id)?;
    if !(0.0..=1.0).contains(&alpha) {
        return Err(eval_error(
            node_id,
            "ewm_mean alpha must be between 0 and 1",
        ));
    }

    let node_name = column_ref_or_err(args, "ewm_mean", node_id)?;
    let values = collect_column_series(node_name, context);

    if values.is_empty() {
        return Ok(f64::NAN);
    }

    // EWM_t = alpha * x_t + (1 - alpha) * EWM_{t-1}, initialized with x_0.
    let mut ewm = values[0].1;
    for (_, value) in values.iter().skip(1) {
        ewm = alpha * value + (1.0 - alpha) * ewm;
    }

    Ok(ewm)
}

/// Evaluate `ewm_std` or `ewm_var`. `func` determines whether to return the
/// variance directly or its square root.
///
/// Arguments:
/// - 2 args: `(series, alpha)` — bias-corrected (pandas `adjust=False, bias=False`)
/// - 3 args: `(series, alpha, adjust)` — bias correction enabled when `adjust != 0.0`
///
/// The recursion is the pandas `adjust=False` form, so the matching unbiased
/// correction is `1 / (1 − Σŵ²)` over the normalized recursion weights
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
            format!("{func}() requires 2 or 3 arguments (series, alpha, [adjust])"),
        ));
    }

    let alpha = evaluate_expr(&args[1], context, node_id)?;
    if !(0.0..=1.0).contains(&alpha) {
        return Err(eval_error(node_id, "ewm alpha must be between 0 and 1"));
    }

    // Default to bias-corrected output (pandas `adjust=False, bias=False`).
    let adjust = if args.len() == 3 {
        evaluate_expr(&args[2], context, node_id)? != 0.0
    } else {
        true
    };

    let node_name = column_ref_or_err(args, "ewm_std/var", node_id)?;
    let values = collect_column_series(node_name, context);

    if values.len() < 2 {
        return Ok(f64::NAN);
    }

    // Recursive EWM variance:
    //   ewm_var_t = (1 - alpha) * (ewm_var_{t-1} + alpha * (x_t - ewm_mean_{t-1})^2)
    let mut ewm_mean = values[0].1;
    let mut ewm_var = 0.0;

    for (_, value) in values.iter().skip(1) {
        let diff = value - ewm_mean;
        ewm_mean = alpha * value + (1.0 - alpha) * ewm_mean;
        ewm_var = (1.0 - alpha) * (ewm_var + alpha * diff * diff);
    }

    if adjust {
        // Σŵ² of the normalized `adjust=False` recursion weights, computed
        // recursively: after each new observation the previous weights scale
        // by (1 − α) and the newest gets α, so s₂ ← (1−α)²·s₂ + α².
        let n = values.len();
        let one_minus_alpha = 1.0 - alpha;
        let mut sum_w2 = 1.0_f64;
        for _ in 1..n {
            sum_w2 = one_minus_alpha * one_minus_alpha * sum_w2 + alpha * alpha;
        }
        let denom = 1.0 - sum_w2;
        if denom.abs() > ZERO_TOLERANCE {
            ewm_var /= denom;
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
