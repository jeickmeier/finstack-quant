use super::formula::{
    calculate_mean, calculate_median, calculate_std, calculate_variance,
    collect_all_historical_values, collect_expression_values_sorted,
    collect_expression_window_values, collect_period_range_values, collect_rolling_window_values,
    eval_error, evaluate_non_negative_integer_arg, require_args, require_min_args,
};
use super::results::EvalWarning;
use super::EvaluationContext;
use crate::error::Result;
use finstack_quant_core::dates::PeriodKind;
use finstack_quant_core::expr::{Expr, ExprNode, Function};
use finstack_quant_core::math::{finite_count, finite_max_or_nan, finite_min_or_nan, neumaier_sum};

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

fn collect_window_values(
    arg: &Expr,
    context: &EvaluationContext,
    window: usize,
    node_id: Option<&str>,
) -> Result<Vec<f64>> {
    if let ExprNode::Column(node_name) = &arg.node {
        collect_rolling_window_values(node_name, context, window)
    } else {
        collect_expression_window_values(arg, context, window, node_id)
    }
}

fn collect_all_values(
    arg: &Expr,
    context: &EvaluationContext,
    node_id: Option<&str>,
) -> Result<Vec<f64>> {
    if let ExprNode::Column(node_name) = &arg.node {
        collect_all_historical_values(node_name, context)
    } else {
        Ok(collect_expression_values_sorted(arg, context, node_id)?
            .values()
            .copied()
            .collect())
    }
}

/// Sum finite values, returning `NaN` when none are present.
///
/// Period aggregates (`ytd`, `qtd`, `fiscal_ytd`, `ttm`) use this so "no
/// valid observations in window" surfaces as `NaN` instead of masquerading
/// as a real zero — matching the empty-finite convention in rolling and
/// cumulative aggregates (`evaluate_rolling_function`,
/// `evaluate_cumulative_function`).
#[inline]
fn sum_finite_or_nan(values: &[f64]) -> f64 {
    let filtered = retain_finite(values);
    if filtered.is_empty() {
        f64::NAN
    } else {
        neumaier_sum(filtered.iter().copied())
    }
}

pub(crate) fn evaluate_historical_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    match func {
        Function::RollingMean
        | Function::RollingSum
        | Function::RollingStd
        | Function::RollingVar
        | Function::RollingMedian
        | Function::RollingMin
        | Function::RollingMax
        | Function::RollingCount => evaluate_rolling_function(func, args, context, node_id),
        Function::Std | Function::Var | Function::Median => {
            evaluate_statistical_function(func, args, context, node_id)
        }
        Function::CumSum | Function::CumProd | Function::CumMin | Function::CumMax => {
            evaluate_cumulative_function(func, args, context, node_id)
        }
        Function::Ytd | Function::Qtd | Function::FiscalYtd | Function::Ttm => {
            evaluate_period_aggregate_function(func, args, context, node_id)
        }
        _ => Err(eval_error(
            node_id,
            format!("Function {:?} is not a historical aggregate", func),
        )),
    }
}

fn evaluate_rolling_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_args(&func.to_string(), args, 2, node_id)?;

    let window =
        evaluate_non_negative_integer_arg(&func.to_string(), &args[1], context, node_id)? as usize;
    if window == 0 {
        return Err(eval_error(node_id, "Window size must be greater than 0"));
    }

    let raw_values = collect_window_values(&args[0], context, window, node_id)?;

    // Skip non-finite values (see `retain_finite` doc). Note: `RollingCount`
    // counts finite observations, matching pandas `rolling().count()`.
    let values = retain_finite(&raw_values);

    if values.is_empty() {
        return Ok(f64::NAN);
    }

    match func {
        Function::RollingMean => calculate_mean(&values),
        Function::RollingSum => Ok(neumaier_sum(values.iter().copied())),
        Function::RollingStd => calculate_std(&values),
        Function::RollingVar => calculate_variance(&values),
        Function::RollingMedian => calculate_median(&values),
        Function::RollingMin => Ok(finite_min_or_nan(&values)),
        Function::RollingMax => Ok(finite_max_or_nan(&values)),
        Function::RollingCount => Ok(finite_count(&values) as f64),
        _ => Err(eval_error(
            node_id,
            format!("Function {:?} is not a rolling window function", func),
        )),
    }
}

fn evaluate_statistical_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_min_args(&func.to_string(), args, 1, node_id)?;

    let raw_values = collect_all_values(&args[0], context, node_id)?;
    let values = retain_finite(&raw_values);

    match func {
        Function::Std => calculate_std(&values),
        Function::Var => calculate_variance(&values),
        Function::Median => calculate_median(&values),
        _ => Err(eval_error(
            node_id,
            format!("Function {:?} is not a statistical function", func),
        )),
    }
}

fn evaluate_cumulative_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_min_args(&func.to_string(), args, 1, node_id)?;

    let raw_values = collect_all_values(&args[0], context, node_id)?;
    let values = retain_finite(&raw_values);

    if values.is_empty() {
        return Ok(f64::NAN);
    }

    match func {
        Function::CumSum => Ok(neumaier_sum(values.iter().copied())),
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
                        context.push_warning(EvalWarning::NonFiniteValue {
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
        Function::CumMin => Ok(finite_min_or_nan(&values)),
        Function::CumMax => Ok(finite_max_or_nan(&values)),
        _ => Err(eval_error(
            node_id,
            format!("Function {:?} is not a cumulative function", func),
        )),
    }
}

fn evaluate_period_aggregate_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    match func {
        Function::Ytd => {
            require_args("ytd", args, 1, node_id)?;
            let current = context.period_id;
            let start_of_year = match context.period_kind {
                PeriodKind::Daily => finstack_quant_core::dates::PeriodId::day(current.year, 1),
                PeriodKind::Quarterly => {
                    finstack_quant_core::dates::PeriodId::quarter(current.year, 1)
                }
                PeriodKind::Monthly => finstack_quant_core::dates::PeriodId::month(current.year, 1),
                PeriodKind::Weekly => finstack_quant_core::dates::PeriodId::week(current.year, 1),
                PeriodKind::SemiAnnual => {
                    finstack_quant_core::dates::PeriodId::half(current.year, 1)
                }
                PeriodKind::Annual => finstack_quant_core::dates::PeriodId::annual(current.year),
            };

            if let ExprNode::Column(node_name) = &args[0].node {
                let values =
                    collect_period_range_values(node_name, context, start_of_year, current)?;
                Ok(sum_finite_or_nan(&values))
            } else {
                Err(eval_error(
                    node_id,
                    "ytd() currently supports only simple column references; use an intermediate node for complex expressions",
                ))
            }
        }
        Function::Qtd => {
            require_args("qtd", args, 1, node_id)?;
            if context.period_kind != PeriodKind::Monthly {
                return Err(eval_error(
                    node_id,
                    "qtd() is only supported for monthly period models",
                ));
            }

            let current = context.period_id;
            let month = current.index as u32;
            let quarter_start_month = ((month - 1) / 3) * 3 + 1;
            let start = finstack_quant_core::dates::PeriodId::month(
                current.year,
                quarter_start_month as u8,
            );

            if let ExprNode::Column(node_name) = &args[0].node {
                let values = collect_period_range_values(node_name, context, start, current)?;
                Ok(sum_finite_or_nan(&values))
            } else {
                Err(eval_error(
                    node_id,
                    "qtd() currently supports only simple column references; use an intermediate node for complex expressions",
                ))
            }
        }
        Function::FiscalYtd => {
            require_args("fiscal_ytd", args, 2, node_id)?;
            if context.period_kind != PeriodKind::Monthly {
                return Err(eval_error(
                    node_id,
                    "fiscal_ytd() is only supported for monthly period models",
                ));
            }

            let start_month_raw = super::formula::evaluate_expr(&args[1], context, node_id)?;
            if !start_month_raw.is_finite()
                || start_month_raw.fract() != 0.0
                || !(1.0..=12.0).contains(&start_month_raw)
            {
                return Err(eval_error(
                    node_id,
                    "fiscal_ytd() fiscal_start_month must be an integer between 1 and 12",
                ));
            }
            let start_month = start_month_raw as u8;
            let current = context.period_id;
            let current_month = current.index;
            let fiscal_start_year = if current_month >= start_month as u16 {
                current.year
            } else {
                current.year - 1
            };
            let start = finstack_quant_core::dates::PeriodId::month(fiscal_start_year, start_month);

            if let ExprNode::Column(node_name) = &args[0].node {
                let values = collect_period_range_values(node_name, context, start, current)?;
                Ok(sum_finite_or_nan(&values))
            } else {
                Err(eval_error(
                    node_id,
                    "fiscal_ytd() currently supports only simple column references; use an intermediate node for complex expressions",
                ))
            }
        }
        Function::Ttm => {
            require_args("ttm", args, 1, node_id)?;
            let window = context.period_kind.periods_per_year() as usize;
            let values = collect_window_values(&args[0], context, window, node_id)?;
            if values.len() < window || !values.iter().all(|value| value.is_finite()) {
                return Ok(f64::NAN);
            }
            Ok(sum_finite_or_nan(&values))
        }
        _ => Err(eval_error(
            node_id,
            format!("Function {:?} is not a period aggregate", func),
        )),
    }
}
