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
    build_context_for_period, eval_error, evaluate_formula, evaluate_integer_arg,
    evaluate_non_negative_integer_arg, require_args,
};
use crate::evaluator::results::EvalWarning;
use finstack_quant_core::dates::{DayCount, DayCountContext, PeriodId, PeriodKind};
use finstack_quant_core::expr::{Expr, ExprNode};
use finstack_quant_core::math::ZERO_TOLERANCE;
use time::{Date, Duration, Weekday};

/// Offset exactly in calendar periods, independent of the number of stored observations.
/// Out-of-range dates have no observation and return `None` without a linear walk.
pub(crate) fn offset_period(context: &EvaluationContext, offset: i32) -> Option<PeriodId> {
    let mut period = context.period_id;
    if offset == 0 {
        return Some(period);
    }
    if period.is_fiscal() && period.kind() == PeriodKind::Daily {
        if let Some((start, _)) = context.history.period_bounds(&period) {
            let target = start.checked_add(Duration::days(i64::from(offset)))?;
            return context
                .history
                .period_starting_on(target, PeriodKind::Daily, true);
        }
        // A standalone context can resolve a same-year backward offset from
        // the identifiers alone. Crossing a fiscal daily year requires the
        // actual model dates retained by `PeriodHistory::with_periods`.
        let index = i64::from(period.index) + i64::from(offset);
        if offset < 0 && index >= 1 {
            period.index = index as u16;
            return Some(period);
        }
        return None;
    }
    if period.is_fiscal() && period.kind() == PeriodKind::Weekly {
        // Core's fiscal weeks are seven-day blocks from the fiscal-year
        // start, followed by a shortened final block: ceil(365/7) and
        // ceil(366/7) both equal 53. Count the final block as one period;
        // subtracting seven Gregorian days across it would skip W53.
        let ordinal = i64::from(period.year) * 53 + i64::from(period.index) - 1 + i64::from(offset);
        period.year = i32::try_from(ordinal.div_euclid(53)).ok()?;
        period.index = (ordinal.rem_euclid(53) + 1) as u16;
        return Some(period);
    }
    match period.kind() {
        PeriodKind::Daily | PeriodKind::Weekly => {
            let (date, days) = if period.kind() == PeriodKind::Daily {
                (
                    Date::from_ordinal_date(period.year, period.index).ok()?,
                    i64::from(offset),
                )
            } else {
                (
                    Date::from_iso_week_date(period.year, period.index as u8, Weekday::Monday)
                        .ok()?,
                    i64::from(offset) * 7,
                )
            };
            Some(PeriodId::from_date(
                date.checked_add(Duration::days(days))?,
                period.kind(),
            ))
        }
        _ => {
            let n = i64::from(period.periods_per_year());
            let ordinal =
                i64::from(period.year) * n + i64::from(period.index) - 1 + i64::from(offset);
            period.year = i32::try_from(ordinal.div_euclid(n)).ok()?;
            period.index = (ordinal.rem_euclid(n) + 1) as u16;
            Some(period)
        }
    }
}

/// Read an expression at an earlier calendar slot without compressing gaps.
fn historical_expression_value(
    expr: &Expr,
    target_period: PeriodId,
    context: &EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    match &expr.node {
        ExprNode::Column(name) => Ok(context
            .get_historical_value(name, &target_period)
            .unwrap_or(f64::NAN)),
        ExprNode::CsRef {
            component,
            instrument_or_total,
        } => {
            if !context
                .historical_capital_structure_cashflows
                .contains_key(&target_period)
            {
                return Ok(f64::NAN);
            }
            context.get_historical_cs_value(component, instrument_or_total, &target_period)
        }
        _ => {
            if !context.history.contains_key(&target_period)
                && !context
                    .historical_capital_structure_cashflows
                    .contains_key(&target_period)
            {
                return Ok(f64::NAN);
            }
            let mut historical = build_context_for_period(target_period, context)?;
            evaluate_formula(expr, &mut historical, node_id)
        }
    }
}

pub(crate) fn eval_lag(
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    require_args("lag", args, 2, node_id)?;

    let lag_periods = evaluate_non_negative_integer_arg("lag", &args[1], context, node_id)?;

    if lag_periods == 0 {
        return evaluate_formula(&args[0], context, node_id);
    }

    let Some(target_period) = offset_period(context, -lag_periods) else {
        return Ok(f64::NAN);
    };
    historical_expression_value(&args[0], target_period, context, node_id)
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
        let v = evaluate_formula(&args[0], context, node_id)?;
        return Ok(if v.is_finite() { 0.0 } else { f64::NAN });
    }

    let Some(target_period) = offset_period(context, -lag_periods) else {
        return Ok(f64::NAN);
    };
    let current_value = evaluate_formula(&args[0], context, node_id)?;
    if current_value.is_nan() {
        return Ok(f64::NAN);
    }
    let lagged_value = historical_expression_value(&args[0], target_period, context, node_id)?;
    Ok(current_value - lagged_value)
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
        let v = evaluate_formula(&args[0], context, node_id)?;
        return Ok(if v.is_finite() { 0.0 } else { f64::NAN });
    }

    let Some(target_period) = offset_period(context, -lag_periods) else {
        return Ok(f64::NAN);
    };
    let current_value = evaluate_formula(&args[0], context, node_id)?;
    if current_value.is_nan() {
        return Ok(f64::NAN);
    }
    let lagged_value = historical_expression_value(&args[0], target_period, context, node_id)?;

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
        evaluate_formula(&args[1], context, node_id)?
    } else {
        context.period_kind.periods_per_year() as f64
    };

    if !periods_raw.is_finite() || periods_raw <= 0.0 {
        return Err(eval_error(
            node_id,
            "growth_rate() periods must be a positive integer",
        ));
    }
    // Tolerance-based integrality check: a periods count computed from other
    // nodes can carry representation noise (e.g. 3.9999999999999996), which an
    // exact `fract() != 0.0` test would spuriously reject.
    if (periods_raw - periods_raw.round()).abs() > ZERO_TOLERANCE {
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
    let periods = periods_raw.round() as i32;
    if periods == 0 {
        return Err(eval_error(
            node_id,
            "growth_rate() periods must be a positive integer",
        ));
    }

    let current_value = evaluate_formula(&args[0], context, node_id)?;
    if !current_value.is_finite() {
        return Ok(f64::NAN);
    }
    let Some(target_period) = offset_period(context, -periods) else {
        return Ok(f64::NAN);
    };
    let start_value = historical_expression_value(&args[0], target_period, context, node_id)?;
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
    // A non-positive base cannot define a compound growth rate. In
    // particular, improving losses must not be reported as negative growth.
    if !start_value.is_finite() || start_value < 0.0 {
        return Ok(f64::NAN);
    }
    let ratio = current_value / start_value;
    if !ratio.is_finite() || ratio < 0.0 {
        return Ok(f64::NAN);
    }
    let elapsed_years = match context.period_kind {
        PeriodKind::Daily | PeriodKind::Weekly => {
            let start = period_observation_date(target_period, context).ok_or_else(|| {
                eval_error(
                    node_id,
                    "growth_rate() requires explicit dates for fiscal daily/weekly periods",
                )
            })?;
            let end = period_observation_date(context.period_id, context).ok_or_else(|| {
                eval_error(
                    node_id,
                    "growth_rate() requires explicit dates for fiscal daily/weekly periods",
                )
            })?;
            DayCount::ActAct.year_fraction(start, end, DayCountContext::default())?
        }
        _ => periods as f64 / f64::from(context.period_kind.periods_per_year()),
    };
    if !elapsed_years.is_finite() || elapsed_years <= 0.0 {
        return Err(eval_error(
            node_id,
            "growth_rate() requires increasing observation dates",
        ));
    }
    let growth = ratio.powf(1.0 / elapsed_years) - 1.0;
    Ok(if growth.is_finite() { growth } else { f64::NAN })
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
        PeriodKind::Daily => Date::from_ordinal_date(period.year, period.index).ok(),
        PeriodKind::Weekly => {
            Date::from_iso_week_date(period.year, period.index as u8, Weekday::Sunday).ok()
        }
        _ => None,
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
        return evaluate_formula(&args[0], context, node_id);
    }

    // Positive shift == backward (lag-like); negative shift is forward-looking
    // and returns NaN to prevent peeking into the future.
    if shift_periods < 0 {
        return Ok(f64::NAN);
    }

    let Some(target_period) = offset_period(context, -shift_periods) else {
        return Ok(f64::NAN);
    };
    historical_expression_value(&args[0], target_period, context, node_id)
}
