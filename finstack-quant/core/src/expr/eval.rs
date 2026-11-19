//! Scalar expression evaluation with DAG planning.
//!
//! [`CompiledExpr`] evaluates an elementwise expression tree over `&[f64]`
//! columns. Structurally identical sub-trees are deduplicated into one
//! execution plan at construction, and each plan node is evaluated exactly
//! once per [`CompiledExpr::eval`] call.
//!
//! Windowed, cumulative, positional and reducing functions (`lag`,
//! `rolling_*`, `cumsum`, `std`, `rank`, ...) share the [`Function`] AST but
//! are evaluated only by the `statements` crate's period-aware evaluator;
//! [`CompiledExpr::new`] rejects them (see [`Function::is_scalar_evaluable`]).

use super::{
    ast::*,
    context::SimpleContext,
    dag::{ExecutionPlan, Operation, build_plan},
};
use smallvec::SmallVec;

/// Compiled expression with a deduplicated execution plan.
///
/// # Thread Safety
///
/// Immutable after construction: `Send + Sync`, and concurrent [`Self::eval`]
/// calls share one instance without serializing.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::expr::{CompiledExpr, Expr, SimpleContext};
///
/// let ctx = SimpleContext::new(["x"]).expect("unique columns");
/// let x = vec![1.0, 2.0, 3.0];
/// let cols: [&[f64]; 1] = [&x];
/// let expr = CompiledExpr::new(Expr::column("x"))?;
///
/// let out = expr.eval(&ctx, &cols, cols[0].len())?;
/// assert_eq!(out, vec![1.0, 2.0, 3.0]);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct CompiledExpr {
    /// Execution plan in topological order, built at construction.
    plan: ExecutionPlan,
}

struct EvalNodeContext<'a> {
    ctx: &'a SimpleContext,
    cols: &'a [&'a [f64]],
    arena: &'a [f64],
    rows: usize,
}

impl<'a> EvalNodeContext<'a> {
    /// Result slice of a dependency. The plan emits every dependency before
    /// its dependents, so `id` always indexes an already-written arena segment.
    #[inline]
    fn dep_slice(&self, id: u64) -> &'a [f64] {
        let start = id as usize * self.rows;
        &self.arena[start..start + self.rows]
    }
}

impl TryFrom<&Expr> for CompiledExpr {
    type Error = crate::Error;

    /// Compile a shared syntax tree without a recursive clone.
    ///
    /// # Arguments
    ///
    /// * `ast` - Borrowed scalar expression tree with the same admission rules as [`Self::new`].
    fn try_from(ast: &Expr) -> crate::Result<Self> {
        Ok(Self {
            plan: super::dag::build_borrowed_plan(ast)?,
        })
    }
}

impl CompiledExpr {
    /// Compile an expression for the scalar evaluator.
    ///
    /// Builds the deduplicated execution plan up front; every evaluation
    /// reuses it.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Validation`] when `ast` calls a function that
    /// only the statements evaluator supports (every non-elementwise
    /// [`Function`]), references the capital structure (`CsRef`), or nests
    /// beyond depth 511 or exceeds 100,000 syntax nodes. Nonfinite literals,
    /// incorrect function arities and invalid round digits are rejected during
    /// compilation. [`Self::eval`] checks column names and data shapes.
    ///
    /// # Arguments
    ///
    /// * `ast` - Expression tree to compile: column references, literals,
    ///   operators, conditionals and elementwise functions only.
    pub fn new(ast: Expr) -> crate::Result<Self> {
        let plan = build_plan(ast)?;
        Ok(Self { plan })
    }

    /// Evaluate all dependencies eagerly and return one value per requested row.
    ///
    /// NaN in a data column denotes missing input. Invalid finite computations,
    /// including those in an unselected branch, fail the entire evaluation.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Exact column names in data order.
    /// * `cols` - Aligned data columns, one per context name. Every value must be finite or NaN.
    /// * `num_rows` - Required length of every column and of the output, including literal-only expressions.
    pub fn eval(
        &self,
        ctx: &SimpleContext,
        cols: &[&[f64]],
        num_rows: usize,
    ) -> crate::Result<Vec<f64>> {
        let plan = &self.plan;
        tracing::debug!(
            row_count = num_rows,
            plan_nodes = plan.nodes.len(),
            "evaluating compiled expression"
        );
        if ctx.column_count() != cols.len() {
            return Err(crate::Error::Validation(format!(
                "expected {} input columns, received {}",
                ctx.column_count(),
                cols.len()
            )));
        }
        for (index, column) in cols.iter().enumerate() {
            if column.len() != num_rows {
                return Err(crate::Error::Validation(format!(
                    "column {index} has {} rows, expected {num_rows}",
                    column.len()
                )));
            }
            if let Some(row) = column.iter().position(|value| value.is_infinite()) {
                return Err(crate::Error::Validation(format!(
                    "infinite value in column {index}, row {row}"
                )));
            }
        }
        for node in &plan.nodes {
            if let Operation::Column(name) = &node.operation
                && ctx.get_index(name).is_none()
            {
                return Err(crate::InputError::NotFound {
                    id: format!("expr column:{name}"),
                }
                .into());
            }
        }
        if num_rows == 0 {
            return Ok(Vec::new());
        }

        // The root is the final node. Allocate its output separately so the
        // returned vector owns only its rows, even for a large intermediate DAG.
        let entries = num_rows.checked_mul(plan.nodes.len()).ok_or_else(|| {
            crate::Error::Validation("expression arena size overflows usize".into())
        })?;
        let allocate = |len| -> crate::Result<Vec<f64>> {
            let mut values = Vec::new();
            values.try_reserve_exact(len).map_err(|error| {
                crate::Error::Validation(format!("could not allocate expression rows: {error}"))
            })?;
            values.resize(len, 0.0);
            Ok(values)
        };
        let mut arena = allocate(entries - num_rows)?;
        let mut result = allocate(num_rows)?;
        for node in &plan.nodes {
            let (dependencies, remaining) = arena.split_at_mut(node.id as usize * num_rows);
            let output = if node.id == plan.root {
                &mut result[..]
            } else {
                &mut remaining[..num_rows]
            };
            let env = EvalNodeContext {
                ctx,
                cols,
                arena: dependencies,
                rows: num_rows,
            };
            Self::eval_node_into(&env, node, output)?;
        }
        Ok(result)
    }

    /// Evaluate a single DAG node directly into a provided output slice (arena-based).
    fn eval_node_into(
        env: &EvalNodeContext<'_>,
        node: &super::dag::DagNode,
        out: &mut [f64],
    ) -> crate::Result<()> {
        match &node.operation {
            Operation::Column(name) => {
                let idx = env
                    .ctx
                    .get_index(name)
                    .ok_or_else(|| crate::InputError::NotFound {
                        id: format!("expr column:{name}"),
                    })?;
                out.copy_from_slice(env.cols[idx]);
            }
            Operation::Literal(bits) => {
                out.fill(f64::from_bits(*bits));
            }
            Operation::Call(func) => {
                let arg_slices: SmallVec<[&[f64]; 4]> = node
                    .dependencies
                    .iter()
                    .map(|&dep_id| env.dep_slice(dep_id))
                    .collect();
                Self::eval_function_into(node.id, *func, &arg_slices, out)?;
            }
            Operation::BinOp(op) => {
                let left = env.dep_slice(node.dependencies[0]);
                let right = env.dep_slice(node.dependencies[1]);
                Self::eval_bin_op_into(node.id, *op, left, right, out)?;
            }
            Operation::UnaryOp(op) => {
                let operand = env.dep_slice(node.dependencies[0]);
                Self::eval_unary_op_into(*op, operand, out);
            }
            Operation::IfThenElse => {
                let condition = env.dep_slice(node.dependencies[0]);
                let then_vals = env.dep_slice(node.dependencies[1]);
                let else_vals = env.dep_slice(node.dependencies[2]);
                Self::eval_if_then_else_into(condition, then_vals, else_vals, out);
            }
        }
        Ok(())
    }

    /// Evaluate a binary operation element-wise into a provided output slice.
    #[inline]
    fn eval_bin_op_into(
        node: u64,
        op: BinOp,
        left: &[f64],
        right: &[f64],
        out: &mut [f64],
    ) -> crate::Result<()> {
        for (row, result) in out.iter_mut().enumerate() {
            let l = left[row];
            let r = right[row];
            // A known false conjunction or true disjunction decides the result
            // even when the other operand is missing.
            *result = match op {
                BinOp::And if l == 0.0 || r == 0.0 => 0.0,
                BinOp::Or if (!l.is_nan() && l != 0.0) || (!r.is_nan() && r != 0.0) => 1.0,
                _ if l.is_nan() || r.is_nan() => f64::NAN,
                BinOp::Div | BinOp::Mod if r == 0.0 => {
                    return Err(crate::Error::ExpressionEvaluation {
                        node,
                        row,
                        message: format!("{op:?} by zero"),
                    });
                }
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => l / r,
                BinOp::Mod => l % r,
                BinOp::Eq => f64::from(l.partial_cmp(&r) == Some(std::cmp::Ordering::Equal)),
                BinOp::Ne => f64::from(l.partial_cmp(&r) != Some(std::cmp::Ordering::Equal)),
                BinOp::Lt => f64::from(l < r),
                BinOp::Le => f64::from(l <= r),
                BinOp::Gt => f64::from(l > r),
                BinOp::Ge => f64::from(l >= r),
                BinOp::And => 1.0,
                BinOp::Or => 0.0,
            };
            if result.is_infinite() {
                return Err(crate::Error::ExpressionEvaluation {
                    node,
                    row,
                    message: format!("{op:?} produced a nonfinite value"),
                });
            }
        }
        Ok(())
    }

    /// Evaluate a binary operation element-wise.
    #[inline]
    fn eval_unary_op_into(op: super::ast::UnaryOp, operand: &[f64], out: &mut [f64]) {
        use super::ast::UnaryOp;
        for (result, &value) in out.iter_mut().zip(operand) {
            *result = match op {
                UnaryOp::Neg => -value,
                UnaryOp::Not if value.is_nan() => f64::NAN,
                UnaryOp::Not => f64::from(value == 0.0),
            };
        }
    }

    /// Evaluate if-then-else element-wise into a provided output slice.
    #[inline]
    fn eval_if_then_else_into(
        condition: &[f64],
        then_vals: &[f64],
        else_vals: &[f64],
        out: &mut [f64],
    ) {
        for (i, out_val) in out.iter_mut().enumerate() {
            let cond = condition[i];
            let then_val = then_vals[i];
            let else_val = else_vals[i];
            *out_val = if cond.is_nan() {
                f64::NAN
            } else if cond != 0.0 {
                then_val
            } else {
                else_val
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::expr::{BinOp, Expr, SimpleContext, UnaryOp};

    fn sample_context() -> (SimpleContext, Vec<Vec<f64>>) {
        let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");
        let data = vec![vec![0.2, 0.5, 3.0, 4.0], vec![0.5, 1.5, 2.5, 3.5]];
        (ctx, data)
    }

    #[test]
    fn undefined_spread_and_missing_logical_inputs_remain_distinct() {
        let ctx = SimpleContext::new(["spread"]).unwrap();
        let spread = Expr::column("spread");
        let ratio = Expr::bin_op(
            BinOp::Div,
            spread.clone(),
            Expr::call(Function::Abs, vec![spread.clone()]),
        );
        let expr = Expr::if_then_else(
            Expr::bin_op(BinOp::Gt, ratio, Expr::literal(0.0)),
            Expr::literal(1.0),
            Expr::literal(-1.0),
        );
        let compiled = CompiledExpr::new(expr).unwrap();
        assert!(
            compiled
                .eval(&ctx, &[&[0.0]], 1)
                .unwrap_err()
                .to_string()
                .contains("by zero")
        );
        assert_eq!(
            compiled.eval(&ctx, &[&[1.0, -2.0]], 2).unwrap(),
            vec![1.0, -1.0]
        );
        for op in [
            BinOp::Eq,
            BinOp::Ne,
            BinOp::Lt,
            BinOp::Le,
            BinOp::Gt,
            BinOp::Ge,
        ] {
            let comparison =
                CompiledExpr::new(Expr::bin_op(op, spread.clone(), Expr::literal(0.0))).unwrap();
            assert!(comparison.eval(&ctx, &[&[f64::NAN]], 1).unwrap()[0].is_nan());
        }
        for (op, other, expected) in [
            (BinOp::And, 0.0, 0.0),
            (BinOp::And, 1.0, f64::NAN),
            (BinOp::Or, 1.0, 1.0),
            (BinOp::Or, 0.0, f64::NAN),
        ] {
            let expression =
                CompiledExpr::new(Expr::bin_op(op, spread.clone(), Expr::literal(other))).unwrap();
            let actual = expression.eval(&ctx, &[&[f64::NAN]], 1).unwrap()[0];
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn eval_plans_if_binop_and_unary_nodes() {
        let (ctx, data) = sample_context();
        let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

        let condition = Expr::bin_op(BinOp::Gt, Expr::column("x"), Expr::column("y"));
        let then_branch = Expr::column("x");
        let else_branch = Expr::unary_op(UnaryOp::Neg, Expr::column("y"));
        let expr = Expr::if_then_else(condition, then_branch, else_branch);

        let compiled = CompiledExpr::new(expr).expect("elementwise expression compiles");
        let result = compiled.eval(&ctx, &cols, cols[0].len()).unwrap();

        assert_eq!(result.len(), 4);
        assert!((result[0] + 0.5).abs() < 1e-12);
        assert!((result[1] + 1.5).abs() < 1e-12);
        assert!((result[2] - 3.0).abs() < 1e-12);
        assert!((result[3] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn new_rejects_statements_layer_functions_and_capital_structure_references() {
        let elementwise = Expr::call(Function::Abs, vec![Expr::column("x")]);
        assert!(CompiledExpr::new(elementwise).is_ok());

        for fun in [
            Function::RollingMean,
            Function::Lag,
            Function::CumSum,
            Function::Std,
            Function::Rank,
            Function::Ttm,
            Function::Sum,
        ] {
            let expr = Expr::call(fun, vec![Expr::column("x"), Expr::literal(2.0)]);
            let err = CompiledExpr::new(expr).expect_err("statements-layer function is rejected");
            assert!(matches!(err, crate::Error::Validation(_)), "{err}");
        }

        let nested = Expr::bin_op(
            BinOp::Add,
            Expr::column("x"),
            Expr::call(Function::Median, vec![Expr::column("x")]),
        );
        assert!(CompiledExpr::new(nested).is_err());
        assert!(CompiledExpr::new(Expr::cs_ref("debt", "total")).is_err());
    }

    #[test]
    fn nan_condition_propagates_in_if_then_else() {
        let ctx = SimpleContext::new(["c", "a", "b"]).expect("unique columns");
        let c = [f64::NAN, 1.0, 0.0];
        let a = [10.0, 10.0, 10.0];
        let b = [20.0, 20.0, 20.0];
        let cols = [c.as_slice(), a.as_slice(), b.as_slice()];

        let ite = CompiledExpr::new(Expr::if_then_else(
            Expr::column("c"),
            Expr::column("a"),
            Expr::column("b"),
        ))
        .expect("compiled");
        let res = ite.eval(&ctx, &cols, cols[0].len()).expect("eval");
        assert!(res[0].is_nan());
        assert_eq!(res[1], 10.0);
        assert_eq!(res[2], 20.0);
    }
}
