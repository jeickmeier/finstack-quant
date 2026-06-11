//! Time-series formula functions: `lag`, `lead`, `diff`, `pct_change`,
//! `growth_rate`, `shift`.
//!
//! These operate relative to a historical period offset from the current
//! evaluation period. Moving them out of `formula.rs` keeps the main
//! dispatcher smaller and groups functions that share the same
//! "offset-period + historical column lookup" pattern.

use crate::error::Result;
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::formula::{
    build_context_for_period, eval_error, evaluate_expr, evaluate_integer_arg,
    evaluate_non_negative_integer_arg, map_err_with_node, require_args,
};
use crate::evaluator::formula_helpers::get_historical_column_value;
use crate::evaluator::results::EvalWarning;
use finstack_core::dates::PeriodId;
use finstack_core::expr::{Expr, ExprNode};
use finstack_core::math::ZERO_TOLERANCE;

/// Offset a `PeriodId` by `offset` periods (positive = forward, negative = backward).
pub(crate) fn offset_period(
    period: PeriodId,
    offset: i32,
    node_id: Option<&str>,
) -> Result<PeriodId> {
    if offset == 0 {
        return Ok(period);
    }

    let mut result = period;
    let steps = offset.unsigned_abs() as usize;

    for _ in 0..steps {
        result = if offset > 0 {
            map_err_with_node(result.next(), node_id)?
        } else {
            map_err_with_node(result.prev(), node_id)?
        };
    }

    Ok(result)
}

pub(crate) fn eval_lag(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_args("lag", args, 2, node_id)?;

    let lag_periods = evaluate_non_negative_integer_arg("lag", &args[1], context, node_id)?;

    if lag_periods == 0 {
        return evaluate_expr(&args[0], context, node_id);
    }

    let target_period = offset_period(context.period_id, -lag_periods, node_id)?;

    if let ExprNode::Column(node_name) = &args[0].node {
        if let Some(value) = get_historical_column_value(context, node_name, &target_period) {
            Ok(value)
        } else {
            Ok(f64::NAN)
        }
    } else if !context.historical_results.contains_key(&target_period) {
        // Target period precedes the model history: return NaN like the
        // column path instead of erroring against an empty context.
        Ok(f64::NAN)
    } else {
        let mut hist_ctx = build_context_for_period(target_period, context)?;
        evaluate_expr(&args[0], &mut hist_ctx, node_id)
    }
}

pub(crate) fn eval_lead(node_id: Option<&str>) -> Result<f64> {
    // Lead is intentionally unsupported to prevent forward-looking bias.
    Err(eval_error(
        node_id,
        "lead() function is not available (forward-looking operations are not supported in financial modeling)",
    ))
}

pub(crate) fn eval_diff(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    if args.is_empty() || args.len() > 2 {
        return Err(eval_error(
            node_id,
            "diff() requires 1 or 2 arguments (expression, [periods])",
        ));
    }

    // Non-negative validation: 0 is the only non-positive case to handle below.
    let lag_periods = if args.len() == 2 {
        evaluate_non_negative_integer_arg("diff", &args[1], context, node_id)?
    } else {
        1
    };

    if lag_periods == 0 {
        // diff(x, 0) == x - x. Propagate NaN from the inner expression rather
        // than collapsing to 0.0 so missing data is not silently masked.
        let v = evaluate_expr(&args[0], context, node_id)?;
        return Ok(if v.is_finite() { 0.0 } else { f64::NAN });
    }

    let target_period = offset_period(context.period_id, -lag_periods, node_id)?;

    if let ExprNode::Column(node_name) = &args[0].node {
        let current_value = context.get_value(node_name)?;
        if current_value.is_nan() {
            return Ok(f64::NAN);
        }
        if let Some(lagged_value) = get_historical_column_value(context, node_name, &target_period)
        {
            Ok(current_value - lagged_value)
        } else {
            Ok(f64::NAN)
        }
    } else {
        let current_value = evaluate_expr(&args[0], context, node_id)?;
        if current_value.is_nan() {
            return Ok(f64::NAN);
        }
        if !context.historical_results.contains_key(&target_period) {
            // Mirror the column path: missing history yields NaN, not an error.
            return Ok(f64::NAN);
        }
        let mut hist_ctx = build_context_for_period(target_period, context)?;
        let lagged_value = evaluate_expr(&args[0], &mut hist_ctx, node_id)?;
        Ok(current_value - lagged_value)
    }
}

pub(crate) fn eval_pct_change(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    if args.is_empty() || args.len() > 2 {
        return Err(eval_error(
            node_id,
            "pct_change() requires 1 or 2 arguments (expression, [periods])",
        ));
    }

    let lag_periods = if args.len() == 2 {
        evaluate_non_negative_integer_arg("pct_change", &args[1], context, node_id)?
    } else {
        1
    };

    if lag_periods == 0 {
        // pct_change(x, 0) == (x - x) / x. Propagate NaN from the inner
        // expression rather than collapsing to 0.0 so missing data is not
        // silently masked (mirrors the diff(x, 0) guard).
        let v = evaluate_expr(&args[0], context, node_id)?;
        return Ok(if v.is_finite() { 0.0 } else { f64::NAN });
    }

    let target_period = offset_period(context.period_id, -lag_periods, node_id)?;

    let (current_value, lagged_value) = if let ExprNode::Column(node_name) = &args[0].node {
        let current = context.get_value(node_name)?;
        let lagged =
            get_historical_column_value(context, node_name, &target_period).unwrap_or(f64::NAN);
        (current, lagged)
    } else {
        let current = evaluate_expr(&args[0], context, node_id)?;
        if !context.historical_results.contains_key(&target_period) {
            // Mirror the column path: missing history yields NaN, not an error.
            return Ok(f64::NAN);
        }
        let mut hist_ctx = build_context_for_period(target_period, context)?;
        let lagged = evaluate_expr(&args[0], &mut hist_ctx, node_id)?;
        (current, lagged)
    };

    if current_value.is_nan() || lagged_value.is_nan() {
        return Ok(f64::NAN);
    }

    if lagged_value.abs() < ZERO_TOLERANCE {
        tracing::warn!(
            "pct_change() division by near-zero lagged value in period {:?}",
            context.period_id
        );
        if let Some(id) = node_id {
            context.push_warning(EvalWarning::DivisionByZero {
                node_id: id.to_string(),
                period: context.period_id,
            });
        }
        Ok(f64::NAN)
    } else {
        Ok((current_value - lagged_value) / lagged_value)
    }
}

pub(crate) fn eval_growth_rate(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    if args.is_empty() || args.len() > 2 {
        return Err(eval_error(
            node_id,
            "growth_rate() requires 1 or 2 arguments (series, [periods])",
        ));
    }

    let periods_raw = if args.len() == 2 {
        evaluate_expr(&args[1], context, node_id)?
    } else {
        context.period_kind.periods_per_year() as f64
    };

    if !periods_raw.is_finite() || periods_raw <= 0.0 {
        return Err(eval_error(
            node_id,
            "growth_rate() periods must be a positive integer",
        ));
    }
    if periods_raw.fract() != 0.0 {
        return Err(eval_error(
            node_id,
            "growth_rate() periods must be a positive integer",
        ));
    }
    if periods_raw > i32::MAX as f64 {
        return Err(eval_error(
            node_id,
            "growth_rate() periods value is too large",
        ));
    }
    let periods = periods_raw as i32;

    if let ExprNode::Column(node_name) = &args[0].node {
        let current_value = context.get_value(node_name)?;
        if current_value.is_nan() {
            return Ok(f64::NAN);
        }

        let target_period = offset_period(context.period_id, -periods, node_id)?;
        if let Some(start_value) = get_historical_column_value(context, node_name, &target_period) {
            if start_value.abs() < ZERO_TOLERANCE {
                tracing::warn!(
                    "growth_rate() division by near-zero base value in period {:?}",
                    context.period_id
                );
                if let Some(id) = node_id {
                    context.push_warning(EvalWarning::DivisionByZero {
                        node_id: id.to_string(),
                        period: context.period_id,
                    });
                }
                return Ok(f64::NAN);
            }
            // CAGR is undefined for non-positive bases: with a negative start
            // value the ratio sign flips, so improving losses would report
            // negative growth (and vice versa). Policy: return NaN whenever
            // start_value <= 0.
            if start_value < 0.0 {
                return Ok(f64::NAN);
            }
            let ratio = current_value / start_value;
            if !ratio.is_finite() || ratio < 0.0 {
                return Ok(f64::NAN);
            }
            let exponent = 1.0 / periods as f64;
            let growth = ratio.powf(exponent) - 1.0;
            if growth.is_finite() {
                Ok(growth)
            } else {
                Ok(f64::NAN)
            }
        } else {
            Ok(f64::NAN)
        }
    } else {
        Err(eval_error(
            node_id,
            "growth_rate() currently supports only simple column references; use an intermediate node for complex expressions",
        ))
    }
}

pub(crate) fn eval_shift(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_args("shift", args, 2, node_id)?;
    let shift_periods = evaluate_integer_arg("shift", &args[1], context, node_id)?;

    if shift_periods == 0 {
        return evaluate_expr(&args[0], context, node_id);
    }

    // Positive shift == backward (lag-like); negative shift is forward-looking
    // and returns NaN to prevent peeking into the future.
    if shift_periods < 0 {
        return Ok(f64::NAN);
    }

    let target_period = offset_period(context.period_id, -shift_periods, node_id)?;

    if let ExprNode::Column(node_name) = &args[0].node {
        if let Some(value) = get_historical_column_value(context, node_name, &target_period) {
            Ok(value)
        } else {
            Ok(f64::NAN)
        }
    } else {
        Err(eval_error(
            node_id,
            "shift() requires a column reference as first argument; use an intermediate node for complex expressions",
        ))
    }
}
