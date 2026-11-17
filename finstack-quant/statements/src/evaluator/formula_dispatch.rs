//! Function-call dispatch for the formula evaluator.
//!
//! This module is the single place that switches over
//! [`finstack_quant_core::expr::Function`]. Scalar functions (`abs`, `sign`,
//! `sum`, `mean`, `min`, `max`, `pow`, `round`, `clamp`, `is_missing`,
//! `coalesce`, `annualize`, `annualize_rate` and the elementwise math
//! helpers) run on typed values with the checked rules of
//! [`crate::evaluator::value`]. History functions delegate to
//! `formula_aggregates`, `formula_ewm` and `formula_timeseries`, which read
//! the typed value of each canonical period slot.

use crate::error::Result;
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::formula::{eval_error, evaluate_value, require_args, require_min_args};
use crate::evaluator::formula_aggregates::{
    eval_quantile, eval_rank, evaluate_cumulative_function, evaluate_rolling_function,
    evaluate_statistical_function,
};
use crate::evaluator::value::{self, Aggregate, At, Value};
use finstack_quant_core::expr::{BinOp, Expr, Function};

/// Evaluate a function call.
///
/// Every arm either delegates to a history kernel or runs a checked scalar
/// function. Argument arity is enforced by the helpers in
/// [`crate::evaluator::formula`] so failures carry the active node id.
pub(crate) fn evaluate_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    use crate::evaluator::formula_timeseries::{
        eval_diff, eval_growth_rate, eval_lag, eval_lead, eval_pct_change, eval_shift,
        evaluate_period_aggregate_function,
    };

    let at = At {
        node_id,
        period: context.period_id,
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

        // Exponentially-weighted statistics
        Function::EwmMean => crate::evaluator::formula_ewm::eval_ewm_mean(args, context, node_id),
        Function::EwmStd | Function::EwmVar => {
            crate::evaluator::formula_ewm::eval_ewm_std_or_var(func, args, context, node_id)
        }

        Function::Rank => eval_rank(args, context, node_id),
        Function::Quantile => eval_quantile(args, context, node_id),

        Function::Abs => {
            require_args("abs", args, 1, node_id)?;
            value::abs(evaluate_value(&args[0], context, node_id)?, at)
        }

        Function::Sign => {
            require_args("sign", args, 1, node_id)?;
            value::sign(evaluate_value(&args[0], context, node_id)?, at)
        }

        // Every operand must be available and in the same units. Scalars use
        // compensated summation; money is summed exactly.
        Function::Sum | Function::Mean | Function::Min | Function::Max => {
            let (name, kind) = match func {
                Function::Sum => ("sum", Aggregate::Sum),
                Function::Mean => ("mean", Aggregate::Mean),
                Function::Min => ("min", Aggregate::Min),
                _ => ("max", Aggregate::Max),
            };
            require_min_args(name, args, 1, node_id)?;
            let values = args
                .iter()
                .map(|arg| evaluate_value(arg, context, node_id))
                .collect::<Result<Vec<_>>>()?;
            value::aggregate(kind, &values, at)
        }

        Function::Pow => {
            require_args("pow", args, 2, node_id)?;
            let base = evaluate_value(&args[0], context, node_id)?;
            let exponent = evaluate_value(&args[1], context, node_id)?;
            if let Some(unavailable) = value::first_unavailable(&[base, exponent]) {
                return Ok(unavailable);
            }
            let (Some(b), Some(e)) = (
                value::scalar_argument(base, "pow()", at)?,
                value::scalar_argument(exponent, "pow()", at)?,
            ) else {
                return Ok(base);
            };
            Ok(value::like(base, at.finite(b.powf(e), "pow()")?))
        }

        Function::Round => {
            if args.is_empty() || args.len() > 2 {
                return Err(eval_error(
                    node_id,
                    "round() requires 1 or 2 arguments (value, [digits])",
                ));
            }
            let input = evaluate_value(&args[0], context, node_id)?;
            let digits = if args.len() == 2 {
                Some(crate::evaluator::formula::evaluate_integer_arg(
                    "round", &args[1], context, node_id,
                )?)
            } else {
                None
            };
            value::round(input, digits, at)
        }

        Function::Floor => unary_scalar("floor", args, context, node_id, |x| Some(x.floor())),
        Function::Ceil => unary_scalar("ceil", args, context, node_id, |x| Some(x.ceil())),
        Function::Ln => unary_scalar("ln", args, context, node_id, |x| (x > 0.0).then(|| x.ln())),
        Function::Exp => unary_scalar("exp", args, context, node_id, |x| Some(x.exp())),
        Function::Log10 => unary_scalar("log10", args, context, node_id, |x| {
            (x > 0.0).then(|| x.log10())
        }),
        Function::Sqrt => unary_scalar("sqrt", args, context, node_id, |x| {
            (x >= 0.0).then(|| x.sqrt())
        }),

        Function::Clamp => {
            require_args("clamp", args, 3, node_id)?;
            let input = evaluate_value(&args[0], context, node_id)?;
            let lo = evaluate_value(&args[1], context, node_id)?;
            let hi = evaluate_value(&args[2], context, node_id)?;
            value::clamp(input, lo, hi, at)
        }

        // True for a missing, masked or unavailable operand; an evaluation
        // error in the operand still propagates.
        Function::IsMissing => {
            require_args("is_missing", args, 1, node_id)?;
            let input = evaluate_value(&args[0], context, node_id)?;
            Ok(Value::Bool(input.is_unavailable()))
        }

        Function::Annualize => eval_annualize(args, context, node_id),
        Function::AnnualizeRate => eval_annualize_rate(args, context, node_id),

        // The first available value, including zero. Later arguments are not
        // evaluated. With none available the result is unavailable.
        Function::Coalesce => {
            require_min_args("coalesce", args, 2, node_id)?;
            let mut last = None;
            for arg in args {
                let candidate = evaluate_value(arg, context, node_id)?;
                if !candidate.is_unavailable() {
                    return Ok(candidate);
                }
                last = Some(candidate);
            }
            last.ok_or_else(|| eval_error(node_id, "coalesce() requires at least 2 arguments"))
        }
    }
}

/// A function of one unitless number. `compute` returns `None` outside the
/// function's domain.
fn unary_scalar(
    name: &str,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
    compute: impl Fn(f64) -> Option<f64>,
) -> Result<Value> {
    require_args(name, args, 1, node_id)?;
    let at = At {
        node_id,
        period: context.period_id,
    };
    let input = evaluate_value(&args[0], context, node_id)?;
    let call = format!("{name}()");
    let Some(x) = value::scalar_argument(input, &call, at)? else {
        return Ok(input);
    };
    let result = compute(x).ok_or_else(|| at.error(format!("{call} is undefined for {x}")))?;
    Ok(value::like(input, at.finite(result, &call)?))
}

/// Annualise a flow (cash flows, income, expenses) by multiplying by the
/// caller's explicit annualization factor; no factor is inferred from the
/// period kind (record CORE-CONVENTIONS-014 #9, O8). For periodic *rates*,
/// use [`eval_annualize_rate`].
fn eval_annualize(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(eval_error(
            node_id,
            "annualize() requires 2 arguments (value, annualization_factor)",
        ));
    }
    let at = At {
        node_id,
        period: context.period_id,
    };
    let input = evaluate_value(&args[0], context, node_id)?;
    let factor = evaluate_value(&args[1], context, node_id)?;
    if let Some(unavailable) = value::first_unavailable(&[input, factor]) {
        return Ok(unavailable);
    }
    positive_factor(factor, "annualize() annualization_factor", at)?;
    value::arithmetic(BinOp::Mul, input, factor, at)
}

/// A positive finite annualization factor.
fn positive_factor(factor: Value, what: &str, at: At<'_>) -> Result<f64> {
    match value::scalar_argument(factor, what, at)? {
        Some(factor) if factor.is_finite() && factor > 0.0 => Ok(factor),
        _ => Err(at.error(format!("{what} must be positive and finite"))),
    }
}

/// Annualise a decimal periodic rate.
///
/// `args[2]` selects the method explicitly: `0` for simple
/// (`rate × factor`) and `1` for compound (`(1 + rate)^factor − 1`). The
/// compound domain includes a rate of −100% and rejects rates below it.
fn eval_annualize_rate(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    require_args("annualize_rate", args, 3, node_id)?;
    let at = At {
        node_id,
        period: context.period_id,
    };
    let rate_value = evaluate_value(&args[0], context, node_id)?;
    let factor_value = evaluate_value(&args[1], context, node_id)?;
    let method = evaluate_value(&args[2], context, node_id)?;
    if let Some(unavailable) = value::first_unavailable(&[rate_value, factor_value, method]) {
        return Ok(unavailable);
    }
    let factor = positive_factor(factor_value, "annualize_rate() periods_per_year", at)?;
    let compound = match value::scalar_argument(method, "annualize_rate() method", at)? {
        Some(0.0) => false,
        Some(1.0) => true,
        _ => {
            return Err(at.error("annualize_rate() method must be 0 (simple) or 1 (compound)"));
        }
    };
    let Some(rate) = value::scalar_argument(rate_value, "annualize_rate() rate", at)? else {
        return Ok(rate_value);
    };
    let result = if compound {
        if rate < -1.0 {
            return Err(at.error(format!(
                "annualize_rate() cannot compound the rate {rate}, which is below -100%"
            )));
        }
        (1.0 + rate).powf(factor) - 1.0
    } else {
        rate * factor
    };
    Ok(value::like(
        rate_value,
        at.finite(result, "annualize_rate()")?,
    ))
}
