//! Evaluate compiled formulas against an evaluation context.
//!
//! Arithmetic operators are handled locally for performance and separation of
//! concerns, while statistical/time-series functions delegate to the shared
//! `finstack-quant-core` helpers.
//!
//! # Numerical Behavior
//!
//! ## NaN Handling
//! - Division by zero → NaN (with log warning)
//! - Missing historical values in lag/shift → NaN
//! - Insufficient data for variance (< 2 values) → NaN
//! - pct_change with near-zero denominator → NaN (with log warning)
//!
//! ## Overflow Protection
//! - Compound growth (`growth_pct`) errors on overflow
//! - Growth rates > 100% produce warnings
//!
//! ## Precision
//! - Equality comparisons use [`finstack_quant_core::math::ZERO_TOLERANCE`]
//! - Suitable for rate comparisons (0.01 bp precision)
//! - Monetary comparisons should use the `Money` type for currency safety

use crate::error::{Error, Result};
use crate::evaluator::context::EvaluationContext;
use crate::evaluator::formula_helpers::{collect_historical_values_sorted, is_truthy};
use crate::evaluator::results::EvalWarning;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::expr::{Expr, ExprNode, Function};
use finstack_quant_core::math::ZERO_TOLERANCE;
use std::collections::BTreeMap;
use std::rc::Rc;

pub(crate) use crate::evaluator::formula_helpers::{
    collect_all_historical_values, collect_period_range_values, collect_rolling_window_values,
};

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

/// Convert boolean to f64 (1.0 for true, 0.0 for false).
#[inline]
fn bool_to_f64(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
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
    if value.fract().abs() > ZERO_TOLERANCE {
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
    if value.fract().abs() > ZERO_TOLERANCE {
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
) -> Result<EvaluationContext> {
    // Share the full columnar history. The period_id on the new context
    // determines what is "current"; aggregate functions filter by ordering.
    let mut period_context = EvaluationContext::new_with_history(
        target_period,
        std::sync::Arc::clone(&context.history),
        std::sync::Arc::clone(&context.historical_capital_structure_cashflows),
    );
    period_context.period_kind = context.period_kind;
    period_context.node_value_types = std::sync::Arc::clone(&context.node_value_types);
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
    } else if let Some(row) = context.history.row(&target_period) {
        period_context.current_values = row.to_vec();
    }

    Ok(period_context)
}

/// Collect expression values over all available periods in chronological order.
///
/// **Performance note:** For complex expressions (not simple Column or Literal),
/// this rebuilds an evaluation context and re-evaluates the expression for each
/// historical period, giving O(P) evaluations. If the expression itself contains
/// aggregate functions that also walk history, the total cost is O(P²). Consider
/// caching results by `(expr_hash, period_id)` if this becomes a bottleneck.
pub(crate) fn collect_expression_values_sorted(
    expr: &Expr,
    context: &EvaluationContext,
    node_id: Option<&str>,
) -> Result<Rc<BTreeMap<PeriodId, f64>>> {
    match &expr.node {
        ExprNode::Column(name) => return collect_historical_values_sorted(name, context),
        ExprNode::Literal(value) => {
            let mut values = BTreeMap::new();
            for period in context.history.keys() {
                if *period >= context.period_id {
                    continue;
                }
                values.insert(*period, *value);
            }
            values.insert(context.period_id, *value);
            return Ok(Rc::new(values));
        }
        _ => {}
    }

    let periods: Vec<PeriodId> = context
        .history
        .keys()
        .filter(|period| **period < context.period_id)
        .copied()
        .chain(std::iter::once(context.period_id))
        .collect();

    let mut values = BTreeMap::new();
    for period in periods {
        let mut period_context = build_context_for_period(period, context)?;
        let value = evaluate_formula(expr, &mut period_context, node_id)?;
        values.insert(period, value);
    }

    Ok(Rc::new(values))
}

/// Returns `true` if the expression tree contains any time-series or
/// aggregate functions that depend on historical values (lag, rolling,
/// cumulative, etc.). Point-wise arithmetic on columns and literals is
/// safe to evaluate period-by-period without full history.
fn has_aggregate(expr: &Expr) -> bool {
    match &expr.node {
        ExprNode::Column(_) | ExprNode::CsRef { .. } | ExprNode::Literal(_) => false,
        ExprNode::Call(func, args) => {
            matches!(
                func,
                Function::Lag
                    | Function::Lead
                    | Function::Diff
                    | Function::PctChange
                    | Function::CumSum
                    | Function::CumProd
                    | Function::CumMin
                    | Function::CumMax
                    | Function::RollingMean
                    | Function::RollingSum
                    | Function::RollingStd
                    | Function::RollingVar
                    | Function::RollingMedian
                    | Function::RollingMin
                    | Function::RollingMax
                    | Function::RollingCount
                    | Function::EwmMean
                    | Function::EwmStd
                    | Function::EwmVar
                    | Function::Std
                    | Function::Var
                    | Function::Median
                    | Function::Rank
                    | Function::Quantile
                    | Function::Shift
                    | Function::Ttm
                    | Function::Ytd
                    | Function::Qtd
                    | Function::FiscalYtd
                    | Function::GrowthRate
            ) || args.iter().any(has_aggregate)
        }
        ExprNode::BinOp { left, right, .. } => has_aggregate(left) || has_aggregate(right),
        ExprNode::UnaryOp { operand, .. } => has_aggregate(operand),
        ExprNode::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => has_aggregate(condition) || has_aggregate(then_expr) || has_aggregate(else_expr),
    }
}

/// Collect expression values for a rolling window in chronological order.
///
/// Uses an optimized reverse-walk when the expression contains no aggregate
/// functions, evaluating only the last `window_size` periods instead of all.
pub(crate) fn collect_expression_window_values(
    expr: &Expr,
    context: &EvaluationContext,
    window_size: usize,
    node_id: Option<&str>,
) -> Result<Vec<f64>> {
    if window_size == 0 {
        return Ok(Vec::new());
    }

    match &expr.node {
        ExprNode::Column(name) => {
            return collect_rolling_window_values(name, context, window_size);
        }
        ExprNode::Literal(value) => {
            let visible_historical = context
                .history
                .keys()
                .filter(|period| **period < context.period_id)
                .count();
            let total = visible_historical + 1;
            return Ok(vec![*value; window_size.min(total)]);
        }
        _ => {}
    }

    if !has_aggregate(expr) {
        let mut periods: Vec<PeriodId> = context
            .history
            .keys()
            .filter(|period| **period < context.period_id)
            .copied()
            .chain(std::iter::once(context.period_id))
            .collect();
        periods.sort_unstable();

        let mut values = Vec::with_capacity(window_size);
        for period in periods.iter().rev().take(window_size) {
            let mut period_context = build_context_for_period(*period, context)?;
            let value = evaluate_formula(expr, &mut period_context, node_id)?;
            values.push(value);
        }
        values.reverse();
        return Ok(values);
    }

    let sorted = collect_expression_values_sorted(expr, context, node_id)?;
    let skip_count = sorted.len().saturating_sub(window_size);
    Ok(sorted.values().skip(skip_count).copied().collect())
}

/// Evaluate a compiled expression.
///
/// Handles both basic arithmetic operations (evaluated directly) and
/// advanced financial/statistical functions (delegated to specialized handlers).
/// Evaluation reads the current period and historical values from `context`;
/// `node_id`, when supplied, is incorporated into diagnostics so formula
/// failures can be traced back to the owning statement node.
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
/// Returns an evaluation error when an expression needs a missing node or
/// capital-structure value, a function receives invalid arguments (for
/// example, a non-integral lag window), a historical lookup is unavailable, or
/// a delegated function cannot evaluate. IEEE non-finite arithmetic may still
/// return `NaN` with a warning rather than an error where the DSL defines that
/// as a propagating numerical result.
/// Recursively evaluate an expression.
pub fn evaluate_formula(
    expr: &Expr,
    context: &mut EvaluationContext,
    node_id: Option<&str>,
) -> Result<f64> {
    use finstack_quant_core::expr::{BinOp, ExprNode, UnaryOp};

    match &expr.node {
        ExprNode::Literal(val) => Ok(*val),
        ExprNode::CsRef {
            component,
            instrument_or_total,
        } => map_err_with_node(
            context.get_cs_value(component, instrument_or_total),
            node_id,
        ),
        ExprNode::Column(name) => map_err_with_node(context.get_value(name), node_id),
        ExprNode::Call(func, args) => {
            crate::evaluator::formula_dispatch::evaluate_function(func, args, context, node_id)
        }
        ExprNode::BinOp { op, left, right } => {
            // Note: Binary operations are evaluated directly here rather than
            // through the Function enum. This is intentional - see module docs.
            let left_val = evaluate_formula(left, context, node_id)?;

            // Short-circuit logical operators before touching the right-hand
            // side. DSL boolean semantics (`is_truthy`) treat non-finite and
            // zero as false, so an AND whose left is false cannot become true
            // and an OR whose left is true cannot become false. Skipping the
            // right side avoids triggering its side effects (division-by-zero
            // warnings, lookup errors, etc.) whenever the result is already
            // determined.
            if matches!(op, BinOp::And) && !is_truthy(left_val) {
                return Ok(bool_to_f64(false));
            }
            if matches!(op, BinOp::Or) && is_truthy(left_val) {
                return Ok(bool_to_f64(true));
            }

            let right_val = evaluate_formula(right, context, node_id)?;

            let result = match op {
                // Arithmetic operations - evaluated directly for performance
                BinOp::Add => left_val + right_val,
                BinOp::Sub => left_val - right_val,
                BinOp::Mul => left_val * right_val,
                BinOp::Div => {
                    // Division by zero yields NaN rather than an error so a
                    // single bad cell does not abort the whole evaluation.
                    // NaN then propagates through every downstream formula
                    // (NaN + x = NaN, comparisons are false). Callers that
                    // need to surface this must run `NonFiniteCheck` — it is
                    // included in the standard `three_statement_checks` and
                    // `credit_underwriting_checks` suites — or inspect the
                    // `DivisionByZero` warning pushed below.
                    if right_val == 0.0 {
                        tracing::warn!(
                            "Division by zero in formula evaluation (period: {:?})",
                            context.period_id
                        );
                        if let Some(id) = node_id {
                            context.push_warning(EvalWarning::DivisionByZero {
                                node_id: id.to_string(),
                                period: context.period_id,
                            });
                        }
                        f64::NAN
                    } else {
                        left_val / right_val
                    }
                }
                BinOp::Mod => {
                    if right_val == 0.0 {
                        tracing::warn!(
                            "Modulo by zero in formula evaluation (period: {:?})",
                            context.period_id
                        );
                        if let Some(id) = node_id {
                            context.push_warning(EvalWarning::DivisionByZero {
                                node_id: id.to_string(),
                                period: context.period_id,
                            });
                        }
                        f64::NAN
                    } else {
                        left_val % right_val
                    }
                }

                // Comparison operations (use approximate equality for == and !=)
                BinOp::Eq => bool_to_f64((left_val - right_val).abs() <= ZERO_TOLERANCE),
                BinOp::Ne => bool_to_f64((left_val - right_val).abs() > ZERO_TOLERANCE),
                BinOp::Lt => bool_to_f64(left_val < right_val),
                BinOp::Le => bool_to_f64(left_val <= right_val),
                BinOp::Gt => bool_to_f64(left_val > right_val),
                BinOp::Ge => bool_to_f64(left_val >= right_val),

                // Logical operations
                BinOp::And => bool_to_f64(is_truthy(left_val) && is_truthy(right_val)),
                BinOp::Or => bool_to_f64(is_truthy(left_val) || is_truthy(right_val)),
            };
            Ok(result)
        }
        ExprNode::UnaryOp { op, operand } => {
            let val = evaluate_formula(operand, context, node_id)?;
            let result = match op {
                UnaryOp::Neg => -val,
                UnaryOp::Not => bool_to_f64(!is_truthy(val)),
            };
            Ok(result)
        }
        ExprNode::IfThenElse {
            condition,
            then_expr,
            else_expr,
        } => {
            let cond_val = evaluate_formula(condition, context, node_id)?;
            if is_truthy(cond_val) {
                evaluate_formula(then_expr, context, node_id)
            } else {
                evaluate_formula(else_expr, context, node_id)
            }
        }
    }
}

// `evaluate_function` lives in [`crate::evaluator::formula_dispatch`].
// Local tests below use the dispatch module's re-export to keep call sites
// concise while still exercising the same code path used by `evaluate_formula`.
#[cfg(test)]
use crate::evaluator::formula_dispatch::evaluate_function;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capital_structure::{CapitalStructureCashflows, CashflowBreakdown};
    use crate::evaluator::PeriodHistory;
    use crate::types::NodeId;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::expr::{Expr, Function};
    use finstack_quant_core::math::kahan_sum;
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

        let mut historical = IndexMap::new();
        for (period, value) in historical_values {
            let mut values = IndexMap::new();
            values.insert(node_id.to_string(), value);
            historical.insert(period, values);
        }

        let mut context = EvaluationContext::new(
            current_period,
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

        let historical =
            build_context_for_period(q1, &context).expect("historical context should build");

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

        let precise = finstack_quant_core::math::mean_or_nan(&values);
        let reference = kahan_sum(values.iter().copied()) / values.len() as f64;
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
        // pd.Series([1, nan, 2, 3]).ewm(alpha=0.5, adjust=False).var(bias=False):
        // absolute-position weights w = [(1−α)³, α(1−α), α] = [1/8, 1/4, 1/2];
        // ŵ = [1/7, 2/7, 4/7]; mean = 17/7; biased var = 26/49; Σŵ² = 3/7;
        // correction = 7/4 ⇒ 26/49 · 7/4 = 13/14.
        // (Two-observation cases cannot discriminate: unbiased var of two
        // points is d²/2 under any weighting.)
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

        assert!((value - 13.0 / 14.0).abs() < 1e-12, "got {value}");
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
        let v = eval(
            Function::Pow,
            &[Expr::literal(-1.0), Expr::literal(0.5)],
            &mut context,
        );
        assert!(v.is_nan());

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

        // ln / exp / log10 / sqrt (IEEE semantics)
        assert_eq!(eval(Function::Ln, &[Expr::literal(1.0)], &mut context), 0.0);
        assert!(eval(Function::Ln, &[Expr::literal(-1.0)], &mut context).is_nan());
        let v = eval(Function::Exp, &[Expr::literal(1.0)], &mut context);
        assert!((v - std::f64::consts::E).abs() < 1e-12);
        let v = eval(Function::Log10, &[Expr::literal(100.0)], &mut context);
        assert!((v - 2.0).abs() < 1e-12);
        assert_eq!(
            eval(Function::Sqrt, &[Expr::literal(4.0)], &mut context),
            2.0
        );
        assert!(eval(Function::Sqrt, &[Expr::literal(-4.0)], &mut context).is_nan());

        // clamp: inclusive bounds, NaN-safe, inverted range → NaN (no panic)
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
        assert!(eval(Function::Clamp, &clamp_args(5.0, 10.0, 0.0), &mut context).is_nan());

        // is_missing: non-finite (NaN or ±inf) → 1, finite → 0
        assert_eq!(
            eval(Function::IsMissing, &[Expr::literal(1.0)], &mut context),
            0.0
        );
        let div_by_zero = Expr::bin_op(
            finstack_quant_core::expr::BinOp::Div,
            Expr::literal(1.0),
            Expr::literal(0.0),
        );
        assert_eq!(eval(Function::IsMissing, &[div_by_zero], &mut context), 1.0);
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
        let reference = kahan_sum([1e16, 1.0, -1e16]);
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
            &[Expr::column("series")],
            &mut context,
            Some("series"),
        )
        .expect("growth_rate evaluation");

        assert!((value - 0.10).abs() < 1e-6, "value={value}");

        let explicit = evaluate_function(
            &Function::GrowthRate,
            &[Expr::column("series"), Expr::literal(2.0)],
            &mut context,
            Some("series"),
        )
        .expect("explicit periods");

        // Between Q1 2025 and Q1 2025 minus 2 quarters (Q3 2024)
        // Values: 146.41 vs 121 → CAGR over 2 periods ≈ 10%
        assert!((explicit - 0.10).abs() < 1e-6, "explicit={explicit}");
    }

    #[test]
    fn annualize_uses_period_kind_when_periods_missing() {
        let period = PeriodId::month(2025, 3).expect("valid period fixture");
        let mut context = EvaluationContext::new(
            period,
            std::sync::Arc::new(IndexMap::new()),
            std::sync::Arc::new(IndexMap::new()),
        );

        let default_factor = evaluate_function(
            &Function::Annualize,
            &[Expr::literal(2.5)],
            &mut context,
            Some("annualize"),
        )
        .expect("annualize default");

        assert!((default_factor - 30.0).abs() < 1e-9);

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

        let sign_nan = evaluate_function(
            &Function::Sign,
            &[Expr::literal(f64::NAN)],
            &mut context,
            Some("sign"),
        )
        .expect("sign nan");
        assert!(sign_nan.is_nan());
    }

    #[test]
    fn nan_conditions_are_falsey_in_formula_logic() {
        let period = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let mut context = EvaluationContext::new(
            period,
            std::sync::Arc::new(IndexMap::new()),
            std::sync::Arc::new(IndexMap::new()),
        );

        let if_expr = crate::dsl::parse_and_compile("if(0 / 0, 1, 2)").expect("compile if expr");
        let if_value =
            evaluate_formula(&if_expr, &mut context, Some("if_nan")).expect("evaluate if expr");
        assert_eq!(if_value, 2.0);

        let and_expr = crate::dsl::parse_and_compile("(0 / 0) and 1").expect("compile and expr");
        let and_value =
            evaluate_formula(&and_expr, &mut context, Some("and_nan")).expect("evaluate and expr");
        assert_eq!(and_value, 0.0);

        let not_expr = crate::dsl::parse_and_compile("not (0 / 0)").expect("compile not expr");
        let not_value =
            evaluate_formula(&not_expr, &mut context, Some("not_nan")).expect("evaluate not expr");
        assert_eq!(not_value, 1.0);
    }

    #[test]
    fn collect_historical_values_sorted_supports_cs_references() {
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let mut context = EvaluationContext::new(
            p2,
            std::sync::Arc::new(IndexMap::new()),
            std::sync::Arc::new(IndexMap::new()),
        );
        let mut hist_cs = IndexMap::new();
        hist_cs.insert(p1, build_cs_snapshot(p1, 100.0, 5.0));
        context.historical_capital_structure_cashflows = std::sync::Arc::new(hist_cs);
        context.capital_structure_cashflows = Some(build_cs_snapshot(p2, 90.0, 4.0));

        let values = collect_historical_values_sorted("__cs__debt_balance__total", &context)
            .expect("cs history");
        assert_eq!(values.get(&p1), Some(&100.0));
        assert_eq!(values.get(&p2), Some(&90.0));
    }

    #[test]
    fn lag_supports_cs_references() {
        let p1 = PeriodId::quarter(2025, 1).expect("valid period fixture");
        let p2 = PeriodId::quarter(2025, 2).expect("valid period fixture");
        let mut context = EvaluationContext::new(
            p2,
            std::sync::Arc::new(IndexMap::new()),
            std::sync::Arc::new(IndexMap::new()),
        );
        let mut hist_cs = IndexMap::new();
        hist_cs.insert(p1, build_cs_snapshot(p1, 100.0, 5.0));
        context.historical_capital_structure_cashflows = std::sync::Arc::new(hist_cs);
        context.capital_structure_cashflows = Some(build_cs_snapshot(p2, 90.0, 4.0));

        let value = evaluate_function(
            &Function::Lag,
            &[
                Expr::column("__cs__interest_expense__total"),
                Expr::literal(1.0),
            ],
            &mut context,
            Some("lag_cs"),
        )
        .expect("lag over cs should succeed");
        assert_eq!(value, 5.0);
    }
}
