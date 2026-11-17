//! Evaluate compiled formulas against an evaluation context.
//!
//! [`evaluate_value`] is the typed evaluator: checked scalar arithmetic,
//! exact money, Boolean controls and unavailable values (see
//! [`crate::evaluator::value`]). History kernels (`lag`, rolling windows,
//! cumulative and exponentially weighted statistics) still read the float
//! view of their operands through [`evaluate_formula`], where an unavailable
//! value is `NaN`.

use crate::error::{Error, Result};
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::value::{self, At, Value};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::expr::Expr;
use std::collections::BTreeSet;

fn annotate_error(err: Error, node_id: Option<&str>) -> Error {
    match (node_id, err) {
        (Some(id), Error::Eval(msg)) => {
            if msg.starts_with("[node ") {
                Error::Eval(msg)
            } else {
                Error::Eval(format!("[node {}] {}", id, msg))
            }
        }
        (_, other) => other,
    }
}

pub(crate) fn eval_error(node_id: Option<&str>, msg: impl Into<String>) -> Error {
    annotate_error(Error::eval(msg), node_id)
}

pub(crate) fn map_err_with_node<T, E>(
    res: std::result::Result<T, E>,
    node_id: Option<&str>,
) -> Result<T>
where
    E: Into<Error>,
{
    res.map_err(|err| annotate_error(err.into(), node_id))
}

/// Validate that a function has exactly the expected number of arguments.
#[inline]
pub(crate) fn require_args(
    func_name: &str,
    args: &[Expr],
    expected: usize,
    node_id: Option<&str>,
) -> Result<()> {
    if args.len() != expected {
        return Err(eval_error(
            node_id,
            format!(
                "{}() requires exactly {} argument{}",
                func_name,
                expected,
                if expected == 1 { "" } else { "s" }
            ),
        ));
    }
    Ok(())
}

/// Validate that a function has at least the minimum number of arguments.
#[inline]
pub(crate) fn require_min_args(
    func_name: &str,
    args: &[Expr],
    min: usize,
    node_id: Option<&str>,
) -> Result<()> {
    if args.len() < min {
        return Err(eval_error(
            node_id,
            format!(
                "{}() requires at least {} argument{}",
                func_name,
                min,
                if min == 1 { "" } else { "s" }
            ),
        ));
    }
    Ok(())
}

#[inline]
pub(crate) fn evaluate_non_negative_integer_arg(
    func_name: &str,
    expr: &Expr,
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<i32> {
    let value = evaluate_formula(expr, context, node_id)?;
    if !value.is_finite() {
        return Err(eval_error(
            node_id,
            format!("{func_name}() requires a finite integer argument"),
        ));
    }
    if value.fract() != 0.0 {
        return Err(eval_error(
            node_id,
            format!("{func_name}() requires an integer argument"),
        ));
    }
    if value < 0.0 || value > i32::MAX as f64 {
        return Err(eval_error(
            node_id,
            format!("{func_name}() argument must be a non-negative integer within i32 range"),
        ));
    }

    Ok(value as i32)
}

#[inline]
pub(crate) fn evaluate_integer_arg(
    func_name: &str,
    expr: &Expr,
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<i32> {
    let value = evaluate_formula(expr, context, node_id)?;
    if !value.is_finite() {
        return Err(eval_error(
            node_id,
            format!("{func_name}() requires a finite integer argument"),
        ));
    }
    if value.fract() != 0.0 {
        return Err(eval_error(
            node_id,
            format!("{func_name}() requires an integer argument"),
        ));
    }
    if value < i32::MIN as f64 || value > i32::MAX as f64 {
        return Err(eval_error(
            node_id,
            format!("{func_name}() argument value is out of i32 range"),
        ));
    }
    Ok(value as i32)
}

/// Build a period-specific evaluation context so an expression can be
/// re-evaluated historically with the correct current/historical split.
pub(crate) fn build_context_for_period(
    target_period: PeriodId,
    context: &EvaluationContext,
) -> EvaluationContext {
    // Share the full columnar history. The period_id on the new context
    // determines what is "current"; aggregate functions filter by ordering.
    let mut period_context = EvaluationContext::new_with_history(
        target_period,
        std::sync::Arc::clone(&context.history),
        std::sync::Arc::clone(&context.historical_capital_structure_cashflows),
    );
    period_context.capital_structure_cashflows = if target_period == context.period_id {
        context.capital_structure_cashflows.clone()
    } else {
        context
            .historical_capital_structure_cashflows
            .get(&target_period)
            .cloned()
    };

    if target_period == context.period_id {
        // Same period: the column layout is identical (both share the same
        // `node_to_column` via `Arc`), so copy the evaluated column vector
        // directly. This avoids round-tripping every node through a
        // `String`-keyed `IndexMap` (one heap allocation per node) and
        // re-hashing each name via `set_value`. The temporary context is only
        // used to re-evaluate an expression and read back a single value, so
        // skipping `set_value`'s warning bookkeeping is intentional.
        period_context.current_values = context.current_values.clone();
        period_context.current_cells = context.current_cells.clone();
    } else if let Some(row) = context.history.row(&target_period) {
        period_context.current_values = row.to_vec();
        if let Some(cells) = context.history.cell_row(&target_period) {
            period_context.current_cells = cells.to_vec();
        }
    }

    period_context
}

/// Visible statement and capital-structure dates for historical expressions.
/// Standalone contexts may contain cashflow snapshots without statement rows.
pub(super) fn expression_periods(context: &EvaluationContext) -> BTreeSet<PeriodId> {
    context
        .history
        .keys()
        .chain(context.historical_capital_structure_cashflows.keys())
        .filter(|period| **period < context.period_id)
        .copied()
        .chain(std::iter::once(context.period_id))
        .collect()
}

/// Evaluate a compiled expression to its float view.
///
/// The float view is the typed result of the formula with an unavailable
/// value as `NaN`, a Boolean as `1.0` or `0.0` and money as its nearest
/// float. History kernels and callers that have not moved to typed values
/// read it; the evaluator itself publishes typed cells.
///
/// # Arguments
///
/// * `expr` - Compiled DSL expression to evaluate for the context's current
///   model period.
/// * `context` - Mutable evaluation context providing current, historical, and
///   capital-structure values; diagnostics may be recorded while evaluating.
/// * `node_id` - Optional owning statement-node identifier included in
///   diagnostics; `None` is appropriate for standalone expressions.
///
/// # Errors
///
/// Returns an evaluation error naming the node and period when an expression
/// needs an unknown node or capital-structure value, a function receives
/// invalid arguments, a control is not Boolean, units or currencies are
/// mixed, a value is outside a function's domain, a division or remainder is
/// by zero, or a result overflows.
pub fn evaluate_formula(
    expr: &Expr,
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    evaluate_value(expr, context, node_id).map(Value::float)
}

/// Evaluate a compiled expression to a typed value.
///
/// `if`, `and` and `or` evaluate only the operands they need. `and` and
/// `or` use strong three-valued logic: a false operand decides `and` and a
/// true operand decides `or` even when the other is unavailable; otherwise an
/// unavailable operand makes the result unavailable. An unavailable `if`
/// condition makes the result unavailable.
pub(crate) fn evaluate_value(
    expr: &Expr,
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<Value> {
    use finstack_quant_core::expr::{BinOp, ExprNode, UnaryOp};

    let at = At {
        node_id,
        period: context.period_id,
    };
    match &expr.node {
        ExprNode::Literal(val) => Ok(Value::Scalar(*val)),
        ExprNode::CsRef {
            component,
            instrument_or_total,
        } => map_err_with_node(
            context.get_cs_value(component, instrument_or_total),
            node_id,
        )
        .map(Value::from_float),
        ExprNode::Column(name) => map_err_with_node(context.get_operand(name), node_id),
        ExprNode::Call(func, args) => {
            crate::evaluator::formula_dispatch::evaluate_function(func, args, context, node_id)
        }
        ExprNode::BinOp { op, left, right } => match op {
            BinOp::And | BinOp::Or => {
                let what = if *op == BinOp::And { "and" } else { "or" };
                // `and` is decided by a false operand, `or` by a true one.
                let deciding = *op == BinOp::Or;
                let left_val = evaluate_value(left, context, node_id)?;
                let left_flag = value::control(left_val, what, at)?;
                if left_flag == Some(deciding) {
                    return Ok(Value::Bool(deciding));
                }
                // The right operand is needed. With an unavailable left it
                // can still decide the result; otherwise the result is
                // unavailable.
                let right_val = evaluate_value(right, context, node_id)?;
                Ok(match (left_flag, value::control(right_val, what, at)?) {
                    (_, Some(flag)) if flag == deciding => Value::Bool(deciding),
                    (Some(_), Some(flag)) => Value::Bool(flag),
                    (None, _) => left_val,
                    (Some(_), None) => right_val,
                })
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let left_val = evaluate_value(left, context, node_id)?;
                let right_val = evaluate_value(right, context, node_id)?;
                value::compare(*op, left_val, right_val, at)
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                let left_val = evaluate_value(left, context, node_id)?;
                let right_val = evaluate_value(right, context, node_id)?;
                value::arithmetic(*op, left_val, right_val, at)
            }
        },
        ExprNode::UnaryOp { op, operand } => {
            let val = evaluate_value(operand, context, node_id)?;
            match op {
                UnaryOp::Neg => value::negate(val, at),
                UnaryOp::Not => Ok(match value::control(val, "not", at)? {
                    Some(flag) => Value::Bool(!flag),
                    None => val,
                }),
            }
        }
        ExprNode::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => {
            let cond_val = evaluate_value(condition, context, node_id)?;
            match value::control(cond_val, "if condition", at)? {
                Some(true) => evaluate_value(then_expr, context, node_id),
                Some(false) => evaluate_value(else_expr, context, node_id),
                None => Ok(cond_val),
            }
        }
    }
}

/// Float view of one function call, for the kernel tests below.
#[cfg(test)]
fn evaluate_function(
    func: &finstack_quant_core::expr::Function,
    args: &[Expr],
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    crate::evaluator::formula_dispatch::evaluate_function(func, args, context, node_id)
        .map(Value::float)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
    use crate::evaluator::PeriodHistory;
    use crate::types::NodeId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::expr::{Expr, Function};
    use finstack_quant_core::math::summation::kahan_sum;
    use finstack_quant_core::money::Money;
    use indexmap::IndexMap;

    fn build_context_with_history(
        current_period: PeriodId,
        node_id: &str,
        historical_values: Vec<(PeriodId, f64)>,
        current_value: f64,
    ) -> EvaluationContext {
        let mut node_to_column = IndexMap::new();
        node_to_column.insert(crate::types::NodeId::new(node_id), 0);

        let first = historical_values
            .iter()
            .map(|(period, _)| *period)
            .min()
            .map_or(current_period, |first| first.min(current_period));
        let plan =
            finstack_quant_core::dates::build_periods(&format!("{first}..{current_period}"), None)
                .expect("fixture plan");
        let mut historical = IndexMap::new();
        for (period, value) in historical_values {
            let mut values = IndexMap::new();
            values.insert(node_id.to_string(), value);
            historical.insert(period, values);
        }

        let mut context = EvaluationContext::new(
            current_period,
            &plan,
            std::sync::Arc::new(node_to_column),
            std::sync::Arc::new(historical),
        );
        context
            .set_value(node_id, current_value)
            .expect("set node value");
        context
    }

    #[test]
    fn historical_context_rebuild_copies_the_matching_columnar_row() {
        let q1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let q2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let columns = std::sync::Arc::new(IndexMap::from_iter([
            (NodeId::new("cogs"), 0),
            (NodeId::new("revenue"), 1),
        ]));
        let mut history = PeriodHistory::new(std::sync::Arc::clone(&columns));
        history.push_row(q1, vec![Some(40.0), Some(100.0)]);
        let context = EvaluationContext::new_with_history(
            q2,
            std::sync::Arc::new(history),
            std::sync::Arc::new(IndexMap::new()),
        );

        let historical = build_context_for_period(q1, &context);

        assert_eq!(historical.current_values, vec![Some(40.0), Some(100.0)]);
        assert_eq!(historical.get_value("cogs").expect("cogs"), 40.0);
        assert_eq!(historical.get_value("revenue").expect("revenue"), 100.0);
    }

    fn build_cs_snapshot(
        period: PeriodId,
        debt_balance: f64,
        interest: f64,
    ) -> CapitalStructureCashflows {
        let mut snapshot = CapitalStructureCashflows::new();
        let breakdown = CashflowBreakdown {
            interest_expense_cash: Money::new(interest, Currency::USD)
                .expect("valid money fixture"),
            interest_income_cash: Some(Money::from((0_i64, Currency::USD))),
            interest_expense_pik: Money::from((0_i64, Currency::USD)),
            principal_payment: Money::from((0_i64, Currency::USD)),
            fees: Money::from((0_i64, Currency::USD)),
            debt_balance: Money::new(debt_balance, Currency::USD).expect("valid money fixture"),
            accrued_interest: Money::from((0_i64, Currency::USD)),
            opening_balance: None,
            scheduled_principal: None,
            mandatory_prepayment: None,
            sweep_prepayment: None,
            voluntary_prepayment: None,
            interest_arrears: None,
        };
        let mut totals = IndexMap::new();
        totals.insert(period, breakdown);
        snapshot.totals = totals.clone();
        snapshot.totals_by_currency.insert(Currency::USD, totals);
        snapshot.reporting_currency = Some(Currency::USD);
        snapshot
    }

    #[test]
    fn calculate_mean_matches_kahan_reference() {
        let mut values = vec![1e16];
        values.extend(std::iter::repeat_n(1.0, 256));

        let precise = finstack_quant_core::math::stats::mean(&values)
            .unwrap()
            .unwrap();
        let reference = kahan_sum(values.iter().copied()).unwrap() / values.len() as f64;
        let naive = values.iter().sum::<f64>() / values.len() as f64;

        assert!((precise - reference).abs() < 1e-12);
        assert!(
            (naive - reference).abs() > 1e-6,
            "Expected naive mean to deviate from reference"
        );
    }

    #[test]
    fn ewm_var_defaults_to_bias_correction() {
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

        let mut context = build_context_with_history(p2, "series", vec![(p1, 1.0)], 2.0);
        let value_default = evaluate_function(
            &Function::EwmVar,
            &[Expr::column("series"), Expr::literal(0.5)],
            &mut context,
            Some("ewm_var"),
        )
        .expect("default ewm_var");

        let mut context_no_adjust = build_context_with_history(p2, "series", vec![(p1, 1.0)], 2.0);
        let value_no_adjust = evaluate_function(
            &Function::EwmVar,
            &[
                Expr::column("series"),
                Expr::literal(0.5),
                Expr::literal(0.0),
            ],
            &mut context_no_adjust,
            Some("ewm_var"),
        )
        .expect("ewm_var without adjust");

        // pandas reference: pd.Series([1, 2]).ewm(alpha=0.5, adjust=False).var()
        // Normalized recursion weights ŵ = [0.5, 0.5] ⇒ Σŵ² = 0.5,
        // correction = 1 / (1 − 0.5) = 2; biased var = 0.25 ⇒ 0.25 * 2 = 0.5
        // (bias=False). bias=True leaves the biased 0.25.
        assert!((value_default - 0.5).abs() < 1e-9);
        assert!((value_no_adjust - 0.25).abs() < 1e-9);
        assert!(value_default > value_no_adjust);
    }

    #[test]
    fn ewm_var_matches_pandas_adjust_false_bias_false() {
        // pandas reference:
        // pd.Series([1, 2, 3]).ewm(alpha=0.5, adjust=False).var(bias=False) → 1.1
        // Hand check: ŵ = [0.25, 0.25, 0.5], mean = 2.25, biased var = 0.6875,
        // Σŵ² = 0.375, correction = 1 / (1 − 0.375) = 1.6, 0.6875 * 1.6 = 1.1.
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let p3 = PeriodId::quarter(2025, 3).expect("valid period fixture");

        let mut context = build_context_with_history(p3, "series", vec![(p1, 1.0), (p2, 2.0)], 3.0);
        let value = evaluate_function(
            &Function::EwmVar,
            &[Expr::column("series"), Expr::literal(0.5)],
            &mut context,
            Some("ewm_var"),
        )
        .expect("ewm_var");

        assert!((value - 1.1).abs() < 1e-9, "got {value}");
    }

    #[test]
    fn ewm_preserves_current_overlay_and_excludes_future_rows() {
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let p3 = PeriodId::quarter(2025, 3).expect("valid period fixture");
        let mut context = build_context_with_history(
            p2,
            "series",
            vec![(p1, 1.0), (p2, 999.0), (p3, 999.0)],
            3.0,
        );
        let args = [Expr::column("series"), Expr::literal(0.5)];
        let value = evaluate_function(&Function::EwmMean, &args, &mut context, None)
            .expect("current overlay");
        assert_eq!(value, 2.0);
    }

    #[test]
    fn ewm_mean_decays_across_nan_gaps() {
        // pandas `ignore_na=False` (the default): a NaN observation is skipped
        // but the decay weight still advances across the gap, so the weights
        // for [1, NaN, 3] with adjust=False are (1−α)² and α:
        // pd.Series([1, nan, 3]).ewm(alpha=0.5, adjust=False).mean().iloc[-1]
        //   = (0.25·1 + 0.5·3) / 0.75 = 7/3.
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let p3 = PeriodId::quarter(2025, 3).expect("valid period fixture");

        let mut context =
            build_context_with_history(p3, "series", vec![(p1, 1.0), (p2, f64::NAN)], 3.0);
        let value = evaluate_function(
            &Function::EwmMean,
            &[Expr::column("series"), Expr::literal(0.5)],
            &mut context,
            Some("ewm_mean"),
        )
        .expect("ewm_mean");

        assert!((value - 7.0 / 3.0).abs() < 1e-12, "got {value}");
    }

    #[test]
    fn ewm_var_decays_across_nan_gaps() {
        // Recursive normalization gives weights [1/6, 1/3, 1/2] after
        // the fourth slot. Unbiased variance is 10/11.
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let p3 = PeriodId::quarter(2025, 3).expect("valid period fixture");
        let p4 = PeriodId::quarter(2025, 4).expect("valid period fixture");

        let mut context = build_context_with_history(
            p4,
            "series",
            vec![(p1, 1.0), (p2, f64::NAN), (p3, 2.0)],
            3.0,
        );
        let value = evaluate_function(
            &Function::EwmVar,
            &[Expr::column("series"), Expr::literal(0.5)],
            &mut context,
            Some("ewm_var"),
        )
        .expect("ewm_var");

        assert!((value - 10.0 / 11.0).abs() < 1e-12, "got {value}");
    }

    #[test]
    fn ewm_rejects_alpha_zero() {
        // pandas requires 0 < alpha <= 1: alpha = 0 freezes the mean at the
        // oldest value and zeroes the variance bias-correction denominator.
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

        for func in [Function::EwmMean, Function::EwmVar, Function::EwmStd] {
            let mut context = build_context_with_history(p2, "series", vec![(p1, 1.0)], 2.0);
            let err = evaluate_function(
                &func,
                &[Expr::column("series"), Expr::literal(0.0)],
                &mut context,
                Some("ewm"),
            )
            .expect_err("alpha = 0 must be rejected");
            assert!(err.to_string().contains("alpha"), "got: {err}");
        }
    }

    #[test]
    fn ewm_accepts_arbitrary_expressions() {
        // ewm is linear in its input, so ewm_mean(2 * series) must equal
        // 2 * ewm_mean(series) — and, more importantly, an expression argument
        // must evaluate at all rather than erroring with "requires a column
        // reference".
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let p3 = PeriodId::quarter(2025, 3).expect("valid period fixture");

        let mut context = build_context_with_history(p3, "series", vec![(p1, 1.0), (p2, 2.0)], 3.0);
        let column_mean = evaluate_function(
            &Function::EwmMean,
            &[Expr::column("series"), Expr::literal(0.5)],
            &mut context,
            Some("ewm_mean"),
        )
        .expect("column ewm_mean");

        let mut context = build_context_with_history(p3, "series", vec![(p1, 1.0), (p2, 2.0)], 3.0);
        let doubled = Expr::bin_op(
            finstack_quant_core::expr::BinOp::Mul,
            Expr::column("series"),
            Expr::literal(2.0),
        );
        let expr_mean = evaluate_function(
            &Function::EwmMean,
            &[doubled.clone(), Expr::literal(0.5)],
            &mut context,
            Some("ewm_mean"),
        )
        .expect("expression ewm_mean");
        assert!(
            (expr_mean - 2.0 * column_mean).abs() < 1e-12,
            "got {expr_mean}"
        );

        // ewm_std scales linearly with the input too: std(2x) = 2·std(x).
        let mut context = build_context_with_history(p3, "series", vec![(p1, 1.0), (p2, 2.0)], 3.0);
        let column_std = evaluate_function(
            &Function::EwmStd,
            &[Expr::column("series"), Expr::literal(0.5)],
            &mut context,
            Some("ewm_std"),
        )
        .expect("column ewm_std");
        let mut context = build_context_with_history(p3, "series", vec![(p1, 1.0), (p2, 2.0)], 3.0);
        let expr_std = evaluate_function(
            &Function::EwmStd,
            &[doubled, Expr::literal(0.5)],
            &mut context,
            Some("ewm_std"),
        )
        .expect("expression ewm_std");
        assert!(
            (expr_std - 2.0 * column_std).abs() < 1e-12,
            "got {expr_std}"
        );
    }

    #[test]
    fn elementwise_math_functions_match_shared_semantics() {
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let mut context = build_context_with_history(p2, "x", vec![(p1, 1.0)], 2.5);

        let eval = |func: Function, args: &[Expr], context: &mut EvaluationContext| {
            evaluate_function(&func, args, context, Some("elementwise")).expect("should evaluate")
        };

        // pow
        let v = eval(
            Function::Pow,
            &[Expr::column("x"), Expr::literal(2.0)],
            &mut context,
        );
        assert!((v - 6.25).abs() < 1e-12);
        let fails = |func: Function, args: &[Expr], context: &mut EvaluationContext| {
            evaluate_function(&func, args, context, Some("elementwise")).is_err()
        };
        assert!(fails(
            Function::Pow,
            &[Expr::literal(-1.0), Expr::literal(0.5)],
            &mut context,
        ));

        // round: ties away from zero, optional digits (negative allowed)
        let v = eval(Function::Round, &[Expr::column("x")], &mut context);
        assert_eq!(v, 3.0);
        let v = eval(
            Function::Round,
            &[Expr::literal(-2.5), Expr::literal(0.0)],
            &mut context,
        );
        assert_eq!(v, -3.0);
        let v = eval(
            Function::Round,
            &[Expr::literal(2.34567), Expr::literal(2.0)],
            &mut context,
        );
        assert!((v - 2.35).abs() < 1e-12);
        let v = eval(
            Function::Round,
            &[Expr::literal(1250.0), Expr::literal(-2.0)],
            &mut context,
        );
        assert_eq!(v, 1300.0);
        // Fractional digits are rejected with an error (statements-layer
        // convention for integer arguments, like lag/shift).
        let err = evaluate_function(
            &Function::Round,
            &[Expr::literal(1.0), Expr::literal(1.5)],
            &mut context,
            Some("elementwise"),
        )
        .expect_err("fractional digits must error");
        assert!(err.to_string().contains("integer"), "got: {err}");

        // floor / ceil
        assert_eq!(
            eval(Function::Floor, &[Expr::literal(1.7)], &mut context),
            1.0
        );
        assert_eq!(
            eval(Function::Ceil, &[Expr::literal(1.2)], &mut context),
            2.0
        );

        // ln / exp / log10 / sqrt: a value outside the domain is an error
        assert_eq!(eval(Function::Ln, &[Expr::literal(1.0)], &mut context), 0.0);
        assert!(fails(Function::Ln, &[Expr::literal(-1.0)], &mut context));
        let v = eval(Function::Exp, &[Expr::literal(1.0)], &mut context);
        assert!((v - std::f64::consts::E).abs() < 1e-12);
        let v = eval(Function::Log10, &[Expr::literal(100.0)], &mut context);
        assert!((v - 2.0).abs() < 1e-12);
        assert_eq!(
            eval(Function::Sqrt, &[Expr::literal(4.0)], &mut context),
            2.0
        );
        assert!(fails(Function::Sqrt, &[Expr::literal(-4.0)], &mut context));

        // clamp: inclusive bounds; an inverted range is an error
        let clamp_args =
            |x: f64, lo: f64, hi: f64| [Expr::literal(x), Expr::literal(lo), Expr::literal(hi)];
        assert_eq!(
            eval(Function::Clamp, &clamp_args(5.0, 0.0, 10.0), &mut context),
            5.0
        );
        assert_eq!(
            eval(Function::Clamp, &clamp_args(-5.0, 0.0, 10.0), &mut context),
            0.0
        );
        assert_eq!(
            eval(Function::Clamp, &clamp_args(15.0, 0.0, 10.0), &mut context),
            10.0
        );
        assert!(fails(
            Function::Clamp,
            &clamp_args(5.0, 10.0, 0.0),
            &mut context
        ));

        // is_missing: true only for an unavailable operand
        assert_eq!(
            eval(Function::IsMissing, &[Expr::literal(1.0)], &mut context),
            0.0
        );
        let before_history = Expr::call(Function::Lag, vec![Expr::column("x"), Expr::literal(5.0)]);
        assert_eq!(
            eval(Function::IsMissing, &[before_history], &mut context),
            1.0
        );
    }

    #[test]
    fn rolling_mean_returns_nan_until_window_full() {
        // pandas parity: rolling(window=4) uses min_periods=window by default,
        // so a 3-observation history yields NaN — not a silent 3-point mean
        // presented as a 4-period statistic.
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let p3 = PeriodId::quarter(2025, 3).expect("valid period fixture");

        let mut context =
            build_context_with_history(p3, "series", vec![(p1, 10.0), (p2, 20.0)], 30.0);
        let value = evaluate_function(
            &Function::RollingMean,
            &[Expr::column("series"), Expr::literal(4.0)],
            &mut context,
            Some("rolling_mean"),
        )
        .expect("rolling_mean");

        assert!(value.is_nan(), "partial window must be NaN, got {value}");
    }

    #[test]
    fn rolling_mean_honors_explicit_min_periods() {
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let p3 = PeriodId::quarter(2025, 3).expect("valid period fixture");

        let mut context =
            build_context_with_history(p3, "series", vec![(p1, 10.0), (p2, 20.0)], 30.0);
        let value = evaluate_function(
            &Function::RollingMean,
            &[
                Expr::column("series"),
                Expr::literal(4.0),
                Expr::literal(2.0),
            ],
            &mut context,
            Some("rolling_mean"),
        )
        .expect("rolling_mean with min_periods");

        assert!((value - 20.0).abs() < 1e-12, "got {value}");
    }

    #[test]
    fn rolling_mean_nan_in_window_yields_nan_by_default() {
        // A NaN inside a full window reduces the finite-observation count
        // below min_periods (= window by default), matching pandas.
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let p3 = PeriodId::quarter(2025, 3).expect("valid period fixture");

        let mut context =
            build_context_with_history(p3, "series", vec![(p1, 1.0), (p2, f64::NAN)], 3.0);
        let value = evaluate_function(
            &Function::RollingMean,
            &[Expr::column("series"), Expr::literal(3.0)],
            &mut context,
            Some("rolling_mean"),
        )
        .expect("rolling_mean");

        assert!(
            value.is_nan(),
            "NaN in a full window must be NaN, got {value}"
        );
    }

    /// Characterization: `rolling_count` follows the same `min_periods`
    /// gating as the other rolling aggregates (a behavior change from the
    /// old "always return the count"). A partial window is NaN by default;
    /// `min_periods=1` restores counting over whatever is available.
    /// Matches modern pandas `rolling(window).count()` (min_periods applies
    /// since pandas 1.0).
    #[test]
    fn rolling_count_respects_min_periods_default() {
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let p3 = PeriodId::quarter(2025, 3).expect("valid period fixture");

        // 3 observations (one NaN → 2 finite), window 4: partial window.
        let mut context =
            build_context_with_history(p3, "series", vec![(p1, 10.0), (p2, f64::NAN)], 30.0);
        let value = evaluate_function(
            &Function::RollingCount,
            &[Expr::column("series"), Expr::literal(4.0)],
            &mut context,
            Some("rolling_count"),
        )
        .expect("rolling_count");
        assert!(
            value.is_nan(),
            "partial window must be NaN by default, got {value}"
        );

        // Explicit min_periods=1: counts the finite observations (2).
        let mut context =
            build_context_with_history(p3, "series", vec![(p1, 10.0), (p2, f64::NAN)], 30.0);
        let value = evaluate_function(
            &Function::RollingCount,
            &[
                Expr::column("series"),
                Expr::literal(4.0),
                Expr::literal(1.0),
            ],
            &mut context,
            Some("rolling_count"),
        )
        .expect("rolling_count with min_periods");
        assert!(
            (value - 2.0).abs() < 1e-12,
            "min_periods=1 must count finite observations, got {value}"
        );
    }

    #[test]
    fn rolling_min_periods_larger_than_window_rejected() {
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");

        let mut context = build_context_with_history(p2, "series", vec![(p1, 1.0)], 2.0);
        let err = evaluate_function(
            &Function::RollingMean,
            &[
                Expr::column("series"),
                Expr::literal(2.0),
                Expr::literal(3.0),
            ],
            &mut context,
            Some("rolling_mean"),
        )
        .expect_err("min_periods > window must be rejected");
        assert!(err.to_string().contains("min_periods"), "got: {err}");
    }

    #[test]
    fn sum_function_handles_large_cancellations() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let mut context = EvaluationContext::new(
            period,
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            std::sync::Arc::new(IndexMap::new()),
            std::sync::Arc::new(IndexMap::new()),
        );
        let args = vec![
            Expr::literal(1e16),
            Expr::literal(1.0),
            Expr::literal(-1e16),
        ];
        let sum_value = evaluate_function(&Function::Sum, &args, &mut context, Some("sum_test"))
            .expect("sum evaluation should succeed");
        let reference =
            finstack_quant_core::math::summation::neumaier_sum([1e16, 1.0, -1e16]).unwrap();
        assert!((reference - 1.0).abs() < f64::EPSILON);
        assert!(
            (sum_value - reference).abs() < 1e-12,
            "sum_value={sum_value}, reference={reference}"
        );
    }

    #[test]
    fn growth_rate_defaults_to_period_frequency() {
        let history = vec![
            (
                PeriodId::quarter(2024, 1).expect("valid period fixture"),
                100.0,
            ),
            (
                PeriodId::quarter(2024, 2).expect("valid period fixture"),
                110.0,
            ),
            (
                PeriodId::quarter(2024, 3).expect("valid period fixture"),
                121.0,
            ),
            (
                PeriodId::quarter(2024, 4).expect("valid period fixture"),
                133.1,
            ),
        ];
        let current_period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let mut context = build_context_with_history(current_period, "series", history, 146.41);

        let value = evaluate_function(
            &Function::GrowthRate,
            &[Expr::column("series"), Expr::literal(4.0)],
            &mut context,
            Some("series"),
        )
        .expect("growth_rate evaluation");

        assert!((value - 0.4641).abs() < 1e-6, "value={value}");
        assert!(
            evaluate_function(
                &Function::GrowthRate,
                &[Expr::column("series")],
                &mut context,
                Some("series"),
            )
            .is_err(),
            "growth_rate() has no default lookback"
        );

        let explicit = evaluate_function(
            &Function::GrowthRate,
            &[Expr::column("series"), Expr::literal(2.0)],
            &mut context,
            Some("series"),
        )
        .expect("explicit periods");

        // Between Q1 2025 and Q1 2025 minus 2 quarters (Q3 2024)
        // Values: 146.41 vs 121 over two quarters, annualized: 1.21^2 - 1.
        assert!((explicit - 0.4641).abs() < 1e-6, "explicit={explicit}");
    }

    #[test]
    fn annualize_requires_an_explicit_factor() {
        let period = PeriodId::month(2025, 3).expect("valid period fixture");
        let mut context = EvaluationContext::new(
            period,
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            std::sync::Arc::new(IndexMap::new()),
            std::sync::Arc::new(IndexMap::new()),
        );

        assert!(
            evaluate_function(
                &Function::Annualize,
                &[Expr::literal(2.5)],
                &mut context,
                Some("annualize"),
            )
            .is_err(),
            "annualize() infers no factor from the period kind"
        );

        let override_factor = evaluate_function(
            &Function::Annualize,
            &[Expr::literal(2.5), Expr::literal(4.0)],
            &mut context,
            Some("annualize"),
        )
        .expect("annualize override");

        assert!((override_factor - 10.0).abs() < 1e-9);
    }

    #[test]
    fn ttm_requires_a_full_trailing_window() {
        let current_period = PeriodId::quarter(2025, 3).expect("valid period fixture");
        let history = vec![
            (
                PeriodId::quarter(2025, 1).expect("valid period fixture"),
                10.0,
            ),
            (
                PeriodId::quarter(2025, 2).expect("valid period fixture"),
                20.0,
            ),
        ];
        let mut context = build_context_with_history(current_period, "ebitda", history, 30.0);

        let value = evaluate_function(
            &Function::Ttm,
            &[Expr::column("ebitda")],
            &mut context,
            Some("ttm"),
        )
        .expect("ttm evaluation");

        assert!(value.is_nan(), "partial TTM should be NaN, got {value}");
    }

    #[test]
    fn abs_and_sign_helpers_cover_edge_cases() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let mut context = EvaluationContext::new(
            period,
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            std::sync::Arc::new(IndexMap::new()),
            std::sync::Arc::new(IndexMap::new()),
        );

        let abs_val = evaluate_function(
            &Function::Abs,
            &[Expr::literal(-42.0)],
            &mut context,
            Some("abs"),
        )
        .expect("abs eval");
        assert_eq!(abs_val, 42.0);

        let sign_pos = evaluate_function(
            &Function::Sign,
            &[Expr::literal(3.5)],
            &mut context,
            Some("sign"),
        )
        .expect("sign positive");
        assert_eq!(sign_pos, 1.0);

        let sign_neg = evaluate_function(
            &Function::Sign,
            &[Expr::literal(-3.5)],
            &mut context,
            Some("sign"),
        )
        .expect("sign negative");
        assert_eq!(sign_neg, -1.0);

        let sign_zero = evaluate_function(
            &Function::Sign,
            &[Expr::literal(0.0)],
            &mut context,
            Some("sign"),
        )
        .expect("sign zero");
        assert_eq!(sign_zero, 0.0);
    }

    #[test]
    fn collect_historical_values_sorted_supports_cs_references() {
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let mut context = EvaluationContext::new(
            p2,
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            std::sync::Arc::new(IndexMap::new()),
            std::sync::Arc::new(IndexMap::new()),
        );
        let mut hist_cs = IndexMap::new();
        hist_cs.insert(p1, build_cs_snapshot(p1, 100.0, 5.0));
        context.historical_capital_structure_cashflows = std::sync::Arc::new(hist_cs);
        context.capital_structure_cashflows = Some(build_cs_snapshot(p2, 90.0, 4.0));

        let expression = crate::dsl::parse_and_compile("cs.debt_balance.total")
            .expect("capital-structure reference should compile");
        // The full-history sum reads the past snapshot and the current one.
        let total = evaluate_function(&Function::CumSum, &[expression], &mut context, None)
            .expect("cs history");
        assert_eq!(total, 190.0);
    }

    #[test]
    fn lag_supports_cs_references() {
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let mut context = EvaluationContext::new(
            p2,
            &finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                Vec::new(),
            )
            .expect("empty plan"),
            std::sync::Arc::new(IndexMap::new()),
            std::sync::Arc::new(IndexMap::new()),
        );
        let mut hist_cs = IndexMap::new();
        hist_cs.insert(p1, build_cs_snapshot(p1, 100.0, 5.0));
        context.historical_capital_structure_cashflows = std::sync::Arc::new(hist_cs);
        context.capital_structure_cashflows = Some(build_cs_snapshot(p2, 90.0, 4.0));

        let expression = crate::dsl::parse_and_compile("lag(cs.interest_expense.total, 1)")
            .expect("capital-structure lag should compile");
        let value = evaluate_formula(&expression, &mut context, Some("lag_cs"))
            .expect("lag over cs should succeed");
        assert_eq!(value, 5.0);
    }
}
