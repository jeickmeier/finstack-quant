//! Time-series formula functions: `lag`, `lead`, `diff`, `pct_change`,
//! `growth_rate`, `shift`, and the period-to-date and trailing windows.
//!
//! Offsets are exact non-negative integers stepped in the model plan's
//! calendar. A positive offset reads the typed cell of that slot from the same
//! evaluation run, so money keeps its currency and exactness. A slot the run
//! has not evaluated is unavailable; a fractional offset, a future read and a
//! period outside the plan's calendar are errors.

use crate::error::Result;
use crate::evaluator::cell::UnavailableReason;
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::formula::{
    build_context_for_period, eval_error, evaluate_formula, evaluate_integer_arg,
    evaluate_non_negative_integer_arg, evaluate_value, require_args,
};
use crate::evaluator::results::EvalWarning;
use crate::evaluator::value::{self, Aggregate, At, Value};
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::{DayCount, DayCountContext, PeriodId, PeriodKind};
use finstack_quant_core::expr::{BinOp, Expr, ExprNode, Function};

const MISSING: Value = Value::Unavailable(UnavailableReason::MissingInput);
const UNDEFINED: Value = Value::Unavailable(UnavailableReason::UndefinedChange);

/// The period `offset` periods from the current one, stepped in the model
/// plan's calendar (record CORE-CONVENTIONS-014 #7).
///
/// `Ok(None)` means the target leaves the supported date range, so the
/// series has no value there; a source that is not a period of the plan's
/// calendar is an error, not a missing value.
pub(crate) fn offset_period(context: &EvaluationContext, offset: i32) -> Result<Option<PeriodId>> {
    let period = context.period_id;
    let calendar = context.history.model_plan()?.calendar;
    period.shift(0, calendar)?;
    Ok(period.shift(offset, calendar).ok())
}

pub(super) fn at<'a>(context: &EvaluationContext, node_id: Option<&'a str>) -> At<'a> {
    At {
        node_id,
        period: context.period_id,
    }
}

/// Read an expression at an earlier calendar slot without compressing gaps.
/// A slot the run has not evaluated is unavailable.
fn historical_value(
    expr: &Expr,
    target_period: PeriodId,
    context: &EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    let has_snapshot = context
        .historical_capital_structure_cashflows
        .contains_key(&target_period);
    match &expr.node {
        ExprNode::Column(name) => Ok(context
            .history
            .get_operand(name, &target_period)
            .unwrap_or(MISSING)),
        ExprNode::CsRef {
            component,
            instrument_or_total,
        } => {
            if !has_snapshot {
                return Ok(MISSING);
            }
            context
                .get_historical_cs_value(component, instrument_or_total, &target_period)
                .map(Value::from_float)
        }
        _ => {
            if !context.history.contains_key(&target_period) && !has_snapshot {
                return Ok(MISSING);
            }
            let mut historical = build_context_for_period(target_period, context);
            evaluate_value(expr, &mut historical, node_id)
        }
    }
}

/// The expression's value in `period`: the current evaluation or a past slot.
pub(super) fn value_at(
    expr: &Expr,
    period: PeriodId,
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    if period == context.period_id {
        evaluate_value(expr, context, node_id)
    } else {
        historical_value(expr, period, context, node_id)
    }
}

/// The value `offset` periods back; the current value for a zero offset.
fn value_back(
    expr: &Expr,
    offset: i32,
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    if offset == 0 {
        return evaluate_value(expr, context, node_id);
    }
    match offset_period(context, -offset)? {
        Some(target) => historical_value(expr, target, context, node_id),
        None => Ok(MISSING),
    }
}

/// The optional offset of `diff` and `pct_change`, one period by default.
fn optional_offset(
    func: &str,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<i32> {
    if args.is_empty() || args.len() > 2 {
        return Err(eval_error(
            node_id,
            format!("{func}() requires 1 or 2 arguments (expression, [periods])"),
        ));
    }
    match args.get(1) {
        Some(offset) => evaluate_non_negative_integer_arg(func, offset, context, node_id),
        None => Ok(1),
    }
}

fn warn_zero_base(context: &mut EvaluationContext, node_id: Option<&str>) {
    if let Some(id) = node_id {
        context.warnings.push(EvalWarning::DivisionByZero {
            node_id: id.to_string(),
            period: context.period_id,
        });
    }
}

pub(crate) fn eval_lag(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    require_args("lag", args, 2, node_id)?;
    let offset = evaluate_non_negative_integer_arg("lag", &args[1], context, node_id)?;
    value_back(&args[0], offset, context, node_id)
}

pub(crate) fn eval_lead(node_id: Option<&str>) -> Result<Value> {
    // Lead is intentionally unsupported to prevent forward-looking bias.
    Err(eval_error(
        node_id,
        "lead() function is not available (forward-looking operations are not supported in financial modeling)",
    ))
}

pub(crate) fn eval_shift(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    require_args("shift", args, 2, node_id)?;
    let offset = evaluate_integer_arg("shift", &args[1], context, node_id)?;
    if offset < 0 {
        return Err(at(context, node_id).error("shift() cannot read a future period"));
    }
    value_back(&args[0], offset, context, node_id)
}

/// `diff(x, n)`: current minus the value `n` periods back, in the input's
/// units.
pub(crate) fn eval_diff(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    let offset = optional_offset("diff", args, context, node_id)?;
    let current = evaluate_value(&args[0], context, node_id)?;
    let base = value_back(&args[0], offset, context, node_id)?;
    value::arithmetic(BinOp::Sub, current, base, at(context, node_id))
}

/// `pct_change(x, n)`: the signed decimal `(current − base) / base`, also for
/// a negative base. An exactly zero base is unavailable.
pub(crate) fn eval_pct_change(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    let offset = optional_offset("pct_change", args, context, node_id)?;
    let current = evaluate_value(&args[0], context, node_id)?;
    let base = value_back(&args[0], offset, context, node_id)?;
    if let Some(unavailable) = value::first_unavailable(&[current, base]) {
        return Ok(unavailable);
    }
    if base.float() == 0.0 {
        warn_zero_base(context, node_id);
        return Ok(UNDEFINED);
    }
    let here = at(context, node_id);
    let change = value::arithmetic(BinOp::Sub, current, base, here)?;
    value::arithmetic(BinOp::Div, change, base, here)
}

/// `growth_rate(x, n)`: the compound annual growth rate
/// `(ending / base)^(1 / years) − 1` over an exact positive lookback.
///
/// The base must be strictly positive and the ending value non-negative; a
/// zero ending value is −100%. Daily and weekly plans measure years as
/// Actual/365 Fixed between the periods' final included dates; other plans
/// divide the lookback by the reporting slots per year.
pub(crate) fn eval_growth_rate(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    if args.len() != 2 {
        return Err(eval_error(
            node_id,
            "growth_rate() requires 2 arguments (series, periods)",
        ));
    }
    let periods = evaluate_non_negative_integer_arg("growth_rate", &args[1], context, node_id)?;
    if periods == 0 {
        return Err(eval_error(
            node_id,
            "growth_rate() periods must be a positive integer",
        ));
    }
    let Some(target_period) = offset_period(context, -periods)? else {
        return Ok(MISSING);
    };
    let ending = evaluate_value(&args[0], context, node_id)?;
    let base = historical_value(&args[0], target_period, context, node_id)?;
    if let Some(unavailable) = value::first_unavailable(&[ending, base]) {
        return Ok(unavailable);
    }
    let here = at(context, node_id);
    let (ending, base) = (ending.float(), base.float());
    if base == 0.0 {
        warn_zero_base(context, node_id);
    }
    // A non-positive base cannot define a compound growth rate. In particular,
    // improving losses must not be reported as negative growth.
    if base <= 0.0 || ending < 0.0 {
        return Ok(UNDEFINED);
    }
    let elapsed_years = match context.period_kind {
        PeriodKind::Daily | PeriodKind::Weekly => {
            let dated = |period| {
                period_observation_date(period, context).ok_or_else(|| {
                    eval_error(
                        node_id,
                        "growth_rate() requires explicit dates for fiscal daily/weekly periods",
                    )
                })
            };
            DayCount::Act365F.year_fraction(
                dated(target_period)?,
                dated(context.period_id)?,
                DayCountContext::default(),
            )?
        }
        kind => {
            // Month-block kinds partition every year into a fixed number of
            // periods, so an offset converts to years exactly.
            let slots = kind.slots_per_year().ok_or_else(|| {
                eval_error(
                    node_id,
                    "growth_rate() month-block periods have a slot count",
                )
            })?;
            f64::from(periods) / f64::from(slots)
        }
    };
    if !elapsed_years.is_finite() || elapsed_years <= 0.0 {
        return Err(eval_error(
            node_id,
            "growth_rate() requires increasing observation dates",
        ));
    }
    let growth = (ending / base).powf(1.0 / elapsed_years) - 1.0;
    Ok(Value::Scalar(here.finite(growth, "growth_rate()")?))
}

/// Daily and weekly observations belong to the final included period date.
/// Model bounds take precedence so a fiscal year's shortened final week has
/// its actual length. Standalone Gregorian contexts can derive their dates.
fn period_observation_date(period: PeriodId, context: &EvaluationContext) -> Option<Date> {
    if let Some((_, end)) = context.history.period_bounds(&period) {
        return end.previous_day();
    }
    if period.is_fiscal() {
        return None;
    }
    match period.kind() {
        PeriodKind::Daily | PeriodKind::Weekly => period.bounds().ok()?.1.previous_day(),
        _ => None,
    }
}

/// Sum of the expression over every slot of a window. Every slot must hold a
/// value in the same units; otherwise the window is unavailable.
fn window_sum(
    expr: &Expr,
    slots: &[PeriodId],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    let values = slots
        .iter()
        .map(|slot| value_at(expr, *slot, context, node_id))
        .collect::<Result<Vec<_>>>()?;
    value::aggregate(Aggregate::Sum, &values, at(context, node_id))
}

/// The `count` consecutive plan slots that end at the current period.
pub(super) fn trailing_slots(context: &EvaluationContext, count: u16) -> Result<Vec<PeriodId>> {
    let calendar = context.history.model_plan()?.calendar;
    (0..i32::from(count))
        .rev()
        .map(|back| Ok(context.period_id.shift(-back, calendar)?))
        .collect()
}

/// `ytd`, `qtd`, `fiscal_ytd` and `ttm`: additive sums over complete windows
/// of the model's reporting calendar.
pub(crate) fn evaluate_period_aggregate_function(
    func: &Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    let current = context.period_id;
    let slots = match func {
        // The reporting year of the plan, including a fiscal plan's own year.
        Function::Ytd => {
            require_args("ytd", args, 1, node_id)?;
            trailing_slots(context, current.index())?
        }
        // The Gregorian or fiscal quarter the month belongs to.
        Function::Qtd => {
            require_args("qtd", args, 1, node_id)?;
            if context.period_kind != PeriodKind::Monthly {
                return Err(eval_error(
                    node_id,
                    "qtd() is only supported for monthly period models",
                ));
            }
            trailing_slots(context, (current.index() - 1) % 3 + 1)?
        }
        Function::FiscalYtd => {
            require_args("fiscal_ytd", args, 2, node_id)?;
            if context.period_kind != PeriodKind::Monthly || current.is_fiscal() {
                return Err(eval_error(
                    node_id,
                    "fiscal_ytd() is only supported for Gregorian monthly period models; a \
                     fiscal plan uses ytd()",
                ));
            }
            let start_month = evaluate_formula(&args[1], context, node_id)?;
            if !start_month.is_finite()
                || start_month.fract() != 0.0
                || !(1.0..=12.0).contains(&start_month)
            {
                return Err(eval_error(
                    node_id,
                    "fiscal_ytd() fiscal_start_month must be an integer between 1 and 12",
                ));
            }
            // Months elapsed in the fiscal year that starts in `start_month`.
            let elapsed = (current.index() + 12 - start_month as u16) % 12 + 1;
            trailing_slots(context, elapsed)?
        }
        Function::Ttm => {
            require_args("ttm", args, 1, node_id)?;
            // Twelve calendar months of model periods; `ltm_window` rejects a
            // weekly plan and owns the daily rule.
            let window =
                finstack_quant_core::dates::ltm_window(current, context.history.model_plan()?)?;
            if !window.is_complete() {
                return Ok(MISSING);
            }
            window.slots
        }
        _ => {
            return Err(eval_error(
                node_id,
                format!("Function {:?} is not a period aggregate", func),
            ));
        }
    };
    window_sum(&args[0], &slots, context, node_id)
}
