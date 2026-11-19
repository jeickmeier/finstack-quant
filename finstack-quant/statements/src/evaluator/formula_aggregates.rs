//! Historical statistics over canonical period slots: rolling windows,
//! full-history statistics, cumulative functions, `rank` and `quantile`.
//!
//! Every function reads the typed value of each plan slot through the target
//! period from the same evaluation run. A slot without a value is a gap, not
//! a zero: a window needs its minimum count of available observations, a
//! cumulative function needs every slot from the model start, and a
//! full-history statistic uses the observations it has and reports the count
//! when there are gaps. An error in an evaluated observation propagates.

use super::EvaluationContext;
use super::cell::UnavailableReason;
use super::formula::{
    eval_error, evaluate_formula, evaluate_non_negative_integer_arg, evaluate_value, require_args,
};
use super::formula_timeseries::{at, trailing_slots, value_at};
use super::results::EvalWarning;
use super::value::{self, Aggregate, At, Value};
use crate::error::Result;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::expr::{BinOp, Expr, ExprNode, Function};
use finstack_quant_core::math::stats::{quantile, variance};
use finstack_quant_core::money::Money;
use std::cmp::Ordering;

const MISSING: Value = Value::Unavailable(UnavailableReason::MissingInput);

/// The typed value of an expression in each slot of a run of plan slots.
pub(super) struct Series {
    values: Vec<Value>,
}

impl Series {
    /// Evaluate `expr` in every slot. An error in any slot propagates.
    pub(super) fn collect(
        expr: &Expr,
        slots: &[PeriodId],
        context: &mut EvaluationContext,
        node_id: Option<&str>,
    ) -> Result<Self> {
        let values = slots
            .iter()
            .map(|slot| value_at(expr, *slot, context, node_id))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self { values })
    }

    /// The observations that have a value, in slot order.
    pub(super) fn available(&self) -> Vec<Value> {
        self.values
            .iter()
            .copied()
            .filter(|value| !value.is_unavailable())
            .collect()
    }

    pub(super) fn has_gap(&self) -> bool {
        self.values.iter().any(|value| value.is_unavailable())
    }

    /// One float per slot, `NaN` for a gap.
    pub(super) fn floats(&self) -> Vec<f64> {
        self.values.iter().map(|value| value.float()).collect()
    }

    /// A float statistic in the units of the series: money in the series'
    /// currency, an untyped number when any observation is untyped, otherwise
    /// a scalar.
    pub(super) fn lift(&self, statistic: f64, what: &str, here: At<'_>) -> Result<Value> {
        let statistic = here.finite(statistic, what)?;
        let currency = self.values.iter().find_map(|value| match value {
            Value::Money(money) => Some(money.currency()),
            _ => None,
        });
        if let Some(currency) = currency {
            return Money::from_model_value(statistic, currency)
                .map(Value::Money)
                .map_err(|cause| here.error(format!("{what} failed ({cause})")));
        }
        Ok(
            if self
                .values
                .iter()
                .any(|value| matches!(value, Value::Number(_)))
            {
                Value::Number(statistic)
            } else {
                Value::Scalar(statistic)
            },
        )
    }
}

/// Every plan slot from the model start through the current period.
///
/// A context without a plan (a standalone expression) uses the periods it
/// holds history for.
pub(super) fn prefix_slots(context: &EvaluationContext) -> Result<Vec<PeriodId>> {
    let current = context.period_id;
    let first = context
        .history
        .model_plan()
        .ok()
        .and_then(|plan| Some((plan.first()?.id, plan.get_calendar())));
    let Some((first, calendar)) = first else {
        return Ok(super::formula::expression_periods(context)
            .into_iter()
            .collect());
    };
    let mut slots = Vec::new();
    let mut slot = first;
    while slot < current {
        slots.push(slot);
        slot = slot.shift(1, calendar)?;
    }
    slots.push(current);
    Ok(slots)
}

/// The last `window` slots through the current period.
fn window_slots(context: &EvaluationContext, window: usize) -> Result<Vec<PeriodId>> {
    if context.history.model_plan().is_ok() {
        return trailing_slots(context, u16::try_from(window).unwrap_or(u16::MAX));
    }
    let prefix = prefix_slots(context)?;
    Ok(prefix[prefix.len().saturating_sub(window)..].to_vec())
}

/// Sample variance with the `n − 1` denominator; `None` below two
/// observations.
fn sample_variance(observations: &[Value], what: &str, here: At<'_>) -> Result<Option<f64>> {
    let floats: Vec<f64> = observations.iter().map(|value| value.float()).collect();
    variance(&floats).map_err(|cause| here.error(format!("{what} failed ({cause})")))
}

/// The median: the middle observation, or the mean of the two middle ones.
fn median(observations: &[Value], what: &str, here: At<'_>) -> Result<Value> {
    let mut sorted = observations.to_vec();
    let mut failure = None;
    sorted.sort_by(|a, b| match value::order(*a, *b, what, here) {
        Ok(ordering) => ordering,
        Err(error) => {
            failure.get_or_insert(error);
            Ordering::Equal
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        Ok(sorted[middle])
    } else {
        value::aggregate(Aggregate::Mean, &sorted[middle - 1..=middle], here)
    }
}

/// A statistic shared by rolling windows and full-history functions.
/// `observations` holds at least one value.
fn statistic(
    func: &Function,
    series: &Series,
    observations: &[Value],
    here: At<'_>,
) -> Result<Value> {
    let what = format!("{func}()");
    match func {
        Function::RollingMean => value::aggregate(Aggregate::Mean, observations, here),
        Function::RollingSum => value::aggregate(Aggregate::Sum, observations, here),
        Function::RollingMin => value::aggregate(Aggregate::Min, observations, here),
        Function::RollingMax => value::aggregate(Aggregate::Max, observations, here),
        Function::RollingCount => Ok(Value::Scalar(observations.len() as f64)),
        Function::RollingMedian | Function::Median => median(observations, &what, here),
        // Sample statistics need two observations.
        Function::RollingVar | Function::Var => match sample_variance(observations, &what, here)? {
            Some(var) => Ok(Value::Scalar(here.finite(var, &what)?)),
            None => Ok(MISSING),
        },
        // The standard deviation keeps the units of the series.
        Function::RollingStd | Function::Std => match sample_variance(observations, &what, here)? {
            Some(var) => series.lift(var.sqrt(), &what, here),
            None => Ok(MISSING),
        },
        _ => Err(here.error(format!("{what} is not a historical statistic"))),
    }
}

/// Rolling statistics over the last `window` plan slots.
///
/// `window` is an exact positive integer. `min_periods` is an exact integer
/// from 1 through the window and defaults to the window; the window needs
/// that many available observations.
pub(crate) fn evaluate_rolling_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    if args.len() < 2 || args.len() > 3 {
        return Err(eval_error(
            node_id,
            format!("{func}() requires 2 or 3 arguments (series, window, [min_periods])"),
        ));
    }
    let name = func.to_string();
    let window = evaluate_non_negative_integer_arg(&name, &args[1], context, node_id)? as usize;
    if window == 0 {
        return Err(eval_error(node_id, "Window size must be greater than 0"));
    }
    let min_periods = match args.get(2) {
        Some(arg) => {
            let count = evaluate_non_negative_integer_arg(&name, arg, context, node_id)? as usize;
            if count == 0 || count > window {
                return Err(eval_error(
                    node_id,
                    format!(
                        "{func}() min_periods ({count}) must be from 1 through the window ({window})"
                    ),
                ));
            }
            count
        }
        None => window,
    };
    let slots = window_slots(context, window)?;
    let series = Series::collect(&args[0], &slots, context, node_id)?;
    let observations = series.available();
    if observations.len() < min_periods {
        return Ok(MISSING);
    }
    statistic(func, &series, &observations, at(context, node_id))
}

/// `std`, `var` and `median` over the available observations from the model
/// start through the target. A gap is reported with the sample count.
pub(crate) fn evaluate_statistical_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    require_args(&func.to_string(), args, 1, node_id)?;
    let slots = prefix_slots(context)?;
    let series = Series::collect(&args[0], &slots, context, node_id)?;
    let observations = series.available();
    if observations.is_empty() {
        return Ok(MISSING);
    }
    if series.has_gap()
        && let Some(id) = node_id
    {
        context.warnings.push(EvalWarning::GappedSample {
            node_id: id.to_string(),
            period: context.period_id,
            function: func.to_string(),
            observations: observations.len(),
            slots: slots.len(),
        });
    }
    statistic(func, &series, &observations, at(context, node_id))
}

/// `cumsum`, `cumprod`, `cummin` and `cummax` over the complete prefix from
/// the model start through the target. A gap makes the result unavailable.
pub(crate) fn evaluate_cumulative_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    require_args(&func.to_string(), args, 1, node_id)?;
    let slots = prefix_slots(context)?;
    let series = Series::collect(&args[0], &slots, context, node_id)?;
    let here = at(context, node_id);
    if let Some(unavailable) = value::first_unavailable(&series.values) {
        return Ok(unavailable);
    }
    match func {
        Function::CumSum => value::aggregate(Aggregate::Sum, &series.values, here),
        Function::CumMin => value::aggregate(Aggregate::Min, &series.values, here),
        Function::CumMax => value::aggregate(Aggregate::Max, &series.values, here),
        Function::CumProd => series
            .values
            .iter()
            .try_fold(Value::Scalar(1.0), |product, factor| {
                value::arithmetic(BinOp::Mul, product, *factor, here)
            }),
        _ => Err(eval_error(
            node_id,
            format!("Function {:?} is not a cumulative function", func),
        )),
    }
}

fn require_reference(func: &str, expr: &Expr, node_id: Option<&str>) -> Result<()> {
    if matches!(expr.node, ExprNode::Column(_) | ExprNode::CsRef { .. }) {
        Ok(())
    } else {
        Err(eval_error(
            node_id,
            format!("{func}() requires a statement-node or capital-structure reference"),
        ))
    }
}

/// Rank of the current value among the available observations through the
/// target.
///
/// 1-based with exact competition ties: tied values share the lowest rank.
/// Ascending by default; a direction flag of exactly zero selects descending.
pub(crate) fn eval_rank(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    if args.is_empty() || args.len() > 2 {
        return Err(eval_error(
            node_id,
            "rank() requires 1 or 2 arguments (series, [ascending])",
        ));
    }
    let ascending = match args.get(1) {
        Some(flag) => {
            let direction = evaluate_formula(flag, context, node_id)?;
            if !direction.is_finite() {
                return Err(eval_error(node_id, "rank() ascending flag must be finite"));
            }
            direction != 0.0
        }
        None => true,
    };
    let current = evaluate_value(&args[0], context, node_id)?;
    require_reference("rank", &args[0], node_id)?;
    if current.is_unavailable() {
        return Ok(current);
    }
    let slots = prefix_slots(context)?;
    let series = Series::collect(&args[0], &slots, context, node_id)?;
    let here = at(context, node_id);
    let ahead = if ascending {
        Ordering::Less
    } else {
        Ordering::Greater
    };
    let mut preceding = 0_usize;
    for observation in series.available() {
        if value::order(observation, current, "rank()", here)? == ahead {
            preceding += 1;
        }
    }
    Ok(Value::Scalar((preceding + 1) as f64))
}

/// R-7 quantile (linear interpolation; numpy default, Excel `PERCENTILE`) of
/// the available observations through the target, in the units of the
/// series. The level is a finite number in `[0, 1]`.
pub(crate) fn eval_quantile(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    require_args("quantile", args, 2, node_id)?;
    let level = evaluate_formula(&args[1], context, node_id)?;
    if !(0.0..=1.0).contains(&level) {
        return Err(eval_error(node_id, "quantile must be between 0 and 1"));
    }
    require_reference("quantile", &args[0], node_id)?;
    let slots = prefix_slots(context)?;
    let series = Series::collect(&args[0], &slots, context, node_id)?;
    let here = at(context, node_id);
    let mut floats: Vec<f64> = series
        .available()
        .iter()
        .map(|value| value.float())
        .collect();
    match quantile(&mut floats, level)
        .map_err(|cause| here.error(format!("quantile() failed ({cause})")))?
    {
        Some(result) => series.lift(result, "quantile()", here),
        None => Ok(MISSING),
    }
}
