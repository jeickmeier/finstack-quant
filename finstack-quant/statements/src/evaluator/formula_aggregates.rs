//! Formula-evaluation helpers for statement expressions.
//!
use super::EvaluationContext;
use super::formula::{
    collect_expression_values_sorted, collect_expression_window_values, eval_error,
    evaluate_non_negative_integer_arg, require_args,
};
use super::results::EvalWarning;
use crate::error::Result;
use finstack_quant_core::expr::{Expr, Function};
use finstack_quant_core::math::stats::{mean, median, variance};
use finstack_quant_core::math::{
    stats::{finite_count, finite_max, finite_min},
    summation::neumaier_sum,
};

#[inline]
fn eval_opt_stat(
    res: finstack_quant_core::Result<Option<f64>>,
    node_id: Option<&str>,
) -> Result<f64> {
    res.map(|opt| opt.unwrap_or(f64::NAN))
        .map_err(|e| eval_error(node_id, e.to_string()))
}

#[inline]
fn eval_sum(res: finstack_quant_core::Result<f64>, node_id: Option<&str>) -> Result<f64> {
    res.map_err(|e| eval_error(node_id, e.to_string()))
}

/// NaN policy for aggregate helpers: **skip non-finite values** (pandas
/// `skipna=True`).
///
/// This is necessary because the compensated accumulators (`kahan_sum` /
/// `neumaier_sum`) have undefined behavior when fed NaN or ±∞ — a single
/// non-finite input silently corrupts the running compensation term and
/// poisons every subsequent sum. Filtering up front keeps rolling and
/// cumulative aggregates aligned with the period-aggregate helpers (`ytd`,
/// `ttm`, etc.) which already filter NaN.
#[inline]
fn retain_finite(values: &[f64]) -> Vec<f64> {
    values.iter().copied().filter(|v| v.is_finite()).collect()
}

fn collect_all_values(
    arg: &Expr,
    context: &EvaluationContext,
    node_id: Option<&str>,
) -> Result<Vec<f64>> {
    Ok(collect_expression_values_sorted(arg, context, node_id)?
        .values()
        .copied()
        .collect())
}

/// Sum finite values, returning `NaN` when none are present.
///
/// Period aggregates (`ytd`, `qtd`, `fiscal_ytd`, `ttm`) use this so "no
/// valid observations in window" surfaces as `NaN` instead of masquerading
/// as a real zero — matching the empty-finite convention in rolling and
/// cumulative aggregates (`evaluate_rolling_function`,
/// `evaluate_cumulative_function`).
#[inline]
pub(crate) fn evaluate_rolling_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    if args.len() < 2 || args.len() > 3 {
        return Err(eval_error(
            node_id,
            format!("{func}() requires 2 or 3 arguments (series, window, [min_periods])"),
        ));
    }

    let window =
        evaluate_non_negative_integer_arg(&func.to_string(), &args[1], context, node_id)? as usize;
    if window == 0 {
        return Err(eval_error(node_id, "Window size must be greater than 0"));
    }

    // pandas parity: `min_periods` defaults to the window size, so a partial
    // window (or a window containing NaN) yields NaN rather than silently
    // computing a k-point statistic labeled as a `window`-period one. Pass an
    // explicit third argument to relax it (e.g. `rolling_mean(x, 4, 1)` for
    // the old expanding-until-full behavior).
    let min_periods = if args.len() == 3 {
        let mp = evaluate_non_negative_integer_arg(&func.to_string(), &args[2], context, node_id)?
            as usize;
        if mp > window {
            return Err(eval_error(
                node_id,
                format!("{func}() min_periods ({mp}) cannot exceed window ({window})"),
            ));
        }
        mp.max(1)
    } else {
        window
    };

    let raw_values = collect_expression_window_values(&args[0], context, window, node_id)?;

    // Skip non-finite values (see `retain_finite` doc). Note: `RollingCount`
    // counts finite observations, matching pandas `rolling().count()`.
    let values = retain_finite(&raw_values);

    if values.len() < min_periods {
        return Ok(f64::NAN);
    }

    match func {
        Function::RollingMean => eval_opt_stat(mean(&values), node_id),
        Function::RollingSum => eval_sum(neumaier_sum(values.iter().copied()), node_id),
        Function::RollingStd => {
            eval_opt_stat(variance(&values).map(|opt| opt.map(f64::sqrt)), node_id)
        }
        Function::RollingVar => eval_opt_stat(variance(&values), node_id),
        Function::RollingMedian => eval_opt_stat(median(&values), node_id),
        Function::RollingMin => Ok(finite_min(&values).unwrap_or(f64::NAN)),
        Function::RollingMax => Ok(finite_max(&values).unwrap_or(f64::NAN)),
        Function::RollingCount => Ok(finite_count(&values) as f64),
        _ => Err(eval_error(
            node_id,
            format!("Function {:?} is not a rolling window function", func),
        )),
    }
}

pub(crate) fn evaluate_statistical_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_args(&func.to_string(), args, 1, node_id)?;

    let raw_values = collect_all_values(&args[0], context, node_id)?;
    let values = retain_finite(&raw_values);

    match func {
        Function::Std => eval_opt_stat(variance(&values).map(|opt| opt.map(f64::sqrt)), node_id),
        Function::Var => eval_opt_stat(variance(&values), node_id),
        Function::Median => eval_opt_stat(median(&values), node_id),
        _ => Err(eval_error(
            node_id,
            format!("Function {:?} is not a statistical function", func),
        )),
    }
}

pub(crate) fn evaluate_cumulative_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_args(&func.to_string(), args, 1, node_id)?;

    let raw_values = collect_all_values(&args[0], context, node_id)?;
    let values = retain_finite(&raw_values);

    if values.is_empty() {
        return Ok(f64::NAN);
    }

    match func {
        Function::CumSum => eval_sum(neumaier_sum(values.iter().copied()), node_id),
        Function::CumProd => {
            let mut product = 1.0_f64;
            for &v in &values {
                product *= v;
                if !product.is_finite() {
                    tracing::warn!(
                        "cumprod() overflow detected in period {:?}",
                        context.period_id
                    );
                    if let Some(id) = node_id {
                        context.warnings.push(EvalWarning::NonFiniteValue {
                            node_id: id.to_string(),
                            period: context.period_id,
                            value: product,
                        });
                    }
                    return Ok(f64::NAN);
                }
            }
            Ok(product)
        }
        Function::CumMin => Ok(finite_min(&values).unwrap_or(f64::NAN)),
        Function::CumMax => Ok(finite_max(&values).unwrap_or(f64::NAN)),
        _ => Err(eval_error(
            node_id,
            format!("Function {:?} is not a cumulative function", func),
        )),
    }
}
