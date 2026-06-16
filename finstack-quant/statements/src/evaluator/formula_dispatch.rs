//! Function-call dispatch for the formula evaluator.
//!
//! Extracted from [`crate::evaluator::formula`] to keep the arithmetic /
//! comparison / control-flow core small. This module is the single place that
//! switches over [`finstack_quant_core::expr::Function`] and delegates to the
//! specialised handlers in `formula_aggregates`, `formula_ewm`,
//! `formula_timeseries`, and the inline implementations of small functions
//! (`abs`, `sign`, `sum`, `mean`, `coalesce`, `annualize`, `annualize_rate`,
//! `rank`, `quantile`).

use crate::error::Result;
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::formula::{eval_error, evaluate_expr, require_args, require_min_args};
use crate::evaluator::formula_aggregates::evaluate_historical_function;
use crate::evaluator::formula_helpers::collect_all_historical_values;
use finstack_quant_core::expr::{Expr, ExprNode, Function};
use finstack_quant_core::math::{kahan_sum, quantile_linear_or_nan, ZERO_TOLERANCE};

/// Evaluate a function call.
///
/// Pure dispatch: every arm either delegates to a specialised module or runs a
/// small inline implementation. Common preconditions (argument arity, column-
/// argument requirements) are enforced by the helpers in
/// [`crate::evaluator::formula`] so failures carry the active node id.
pub(crate) fn evaluate_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    use crate::evaluator::formula_timeseries::{
        eval_diff, eval_growth_rate, eval_lag, eval_lead, eval_pct_change, eval_shift,
    };

    match func {
        // Time-series functions
        Function::Lag => eval_lag(args, context, node_id),
        Function::Lead => eval_lead(node_id),
        Function::Diff => eval_diff(args, context, node_id),
        Function::PctChange => eval_pct_change(args, context, node_id),
        Function::GrowthRate => eval_growth_rate(args, context, node_id),
        Function::Shift => eval_shift(args, context, node_id),

        // Historical / rolling / cumulative aggregates
        Function::RollingMean
        | Function::RollingSum
        | Function::RollingStd
        | Function::RollingVar
        | Function::RollingMedian
        | Function::RollingMin
        | Function::RollingMax
        | Function::RollingCount
        | Function::Std
        | Function::Var
        | Function::Median
        | Function::CumSum
        | Function::CumProd
        | Function::CumMin
        | Function::CumMax
        | Function::Ytd
        | Function::Qtd
        | Function::FiscalYtd
        | Function::Ttm => evaluate_historical_function(func, args, context, node_id),

        // Exponentially-weighted statistics
        Function::EwmMean => crate::evaluator::formula_ewm::eval_ewm_mean(args, context, node_id),
        Function::EwmStd | Function::EwmVar => {
            crate::evaluator::formula_ewm::eval_ewm_std_or_var(func, args, context, node_id)
        }

        Function::Rank => eval_rank(args, context, node_id),
        Function::Quantile => eval_quantile(args, context, node_id),

        Function::Abs => {
            require_args("abs", args, 1, node_id)?;
            let value = evaluate_expr(&args[0], context, node_id)?;
            Ok(value.abs())
        }

        Function::Sign => {
            require_args("sign", args, 1, node_id)?;
            let value = evaluate_expr(&args[0], context, node_id)?;
            if value.is_nan() {
                Ok(f64::NAN)
            } else if value > 0.0 {
                Ok(1.0)
            } else if value < 0.0 {
                Ok(-1.0)
            } else {
                Ok(0.0)
            }
        }

        // Custom financial functions: skip non-finite (NaN or ±∞) inputs so
        // compensated summation stays well-defined. Empty finite set → NaN.
        Function::Sum => {
            require_min_args("sum", args, 1, node_id)?;
            let values = finite_arg_values(args, context, node_id)?;
            if values.is_empty() {
                Ok(f64::NAN)
            } else {
                Ok(kahan_sum(values.iter().copied()))
            }
        }

        Function::Mean => {
            require_min_args("mean", args, 1, node_id)?;
            let values = finite_arg_values(args, context, node_id)?;
            if values.is_empty() {
                Ok(f64::NAN)
            } else {
                Ok(kahan_sum(values.iter().copied()) / values.len() as f64)
            }
        }

        Function::Annualize => eval_annualize(args, context, node_id),
        Function::AnnualizeRate => eval_annualize_rate(args, context, node_id),

        Function::Coalesce => {
            require_min_args("coalesce", args, 2, node_id)?;

            let mut last_value = f64::NAN;
            for arg in args {
                let value = evaluate_expr(arg, context, node_id)?;
                last_value = value;
                if !value.is_nan() {
                    return Ok(value);
                }
            }

            // If all values are NaN, return the last one.
            Ok(last_value)
        }
    }
}

fn finite_arg_values(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Vec<f64>> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        let value = evaluate_expr(arg, context, node_id)?;
        if value.is_finite() {
            values.push(value);
        }
    }
    Ok(values)
}

/// Rank of `args[0]` among its historical observations.
///
/// 1-based, ascending; ties share the minimum rank
/// (`rank = 1 + count(historical < current)`). Non-finite current values or
/// empty histories return `NaN` so callers can distinguish "no data" from
/// "best observation" — `unwrap_or(1.0)` would lose that distinction.
fn eval_rank(args: &[Expr], context: &mut EvaluationContext, node_id: Option<&str>) -> Result<f64> {
    require_min_args("rank", args, 1, node_id)?;

    let current_value = evaluate_expr(&args[0], context, node_id)?;

    let ExprNode::Column(node_name) = &args[0].node else {
        return Err(eval_error(
            node_id,
            "rank() requires a column reference as its argument",
        ));
    };

    let all_values = collect_all_historical_values(node_name, context)?;

    if !current_value.is_finite() || all_values.is_empty() {
        return Ok(f64::NAN);
    }

    let strictly_less = all_values
        .iter()
        .filter(|&&v| v.is_finite() && v < current_value - ZERO_TOLERANCE)
        .count();
    Ok((strictly_less + 1) as f64)
}

/// Quantile of `args[0]`'s historical observations using linear interpolation
/// (R-7 / numpy default / Excel `PERCENTILE`).
///
/// `args[1]` is the quantile level in `[0, 1]`. Non-finite values are dropped
/// before interpolation; an empty post-filter set returns `NaN`.
fn eval_quantile(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_args("quantile", args, 2, node_id)?;

    let quantile = evaluate_expr(&args[1], context, node_id)?;
    if !(0.0..=1.0).contains(&quantile) {
        return Err(eval_error(node_id, "quantile must be between 0 and 1"));
    }

    let ExprNode::Column(node_name) = &args[0].node else {
        return Err(eval_error(
            node_id,
            "quantile() requires a column reference",
        ));
    };

    let mut values = collect_all_historical_values(node_name, context)?;
    values.retain(|v| v.is_finite());
    if values.is_empty() {
        return Ok(f64::NAN);
    }
    Ok(quantile_linear_or_nan(&values, quantile))
}

/// Annualise a flow value (cash flows, income, expenses) by multiplying by
/// periods-per-year. For periodic *rates*, use [`eval_annualize_rate`].
fn eval_annualize(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    if args.is_empty() || args.len() > 2 {
        return Err(eval_error(
            node_id,
            "annualize() requires 1 or 2 arguments (value, [periods_per_year])",
        ));
    }

    let value = evaluate_expr(&args[0], context, node_id)?;
    let periods_per_year = if args.len() == 2 {
        evaluate_expr(&args[1], context, node_id)?
    } else {
        context.period_kind.periods_per_year() as f64
    };

    if value.is_nan() || periods_per_year.is_nan() {
        Ok(f64::NAN)
    } else if periods_per_year <= 0.0 {
        Err(eval_error(
            node_id,
            "annualize() periods_per_year must be positive",
        ))
    } else {
        Ok(value * periods_per_year)
    }
}

/// Annualise a periodic rate using either simple or compound methodology.
///
/// `args[2]` selects the method: `0.0` for simple
/// (`rate × periods_per_year`) and any non-zero value for compound
/// (`(1 + rate)^periods_per_year - 1`). Compound results that overflow are
/// downgraded to `NaN` with a warn-level trace event.
fn eval_annualize_rate(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_args("annualize_rate", args, 3, node_id)?;

    let rate = evaluate_expr(&args[0], context, node_id)?;
    let periods_per_year = evaluate_expr(&args[1], context, node_id)?;
    let compounding = evaluate_expr(&args[2], context, node_id)?;

    if rate.is_nan() || periods_per_year.is_nan() || compounding.is_nan() {
        return Ok(f64::NAN);
    }

    if periods_per_year <= 0.0 {
        return Err(eval_error(
            node_id,
            "annualize_rate() periods_per_year must be positive",
        ));
    }

    if compounding == 0.0 {
        Ok(rate * periods_per_year)
    } else {
        let result = (1.0 + rate).powf(periods_per_year) - 1.0;
        if result.is_finite() {
            Ok(result)
        } else {
            tracing::warn!(
                "annualize_rate() overflow: (1 + {})^{} is not finite",
                rate,
                periods_per_year
            );
            Ok(f64::NAN)
        }
    }
}
