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
    dag::{build_plan, ExecutionPlan, Operation},
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
/// let out = expr.eval(&ctx, &cols)?;
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
    offsets: &'a [(usize, usize)],
}

impl<'a> EvalNodeContext<'a> {
    /// Result slice of a dependency. The plan emits every dependency before
    /// its dependents, so `id` always indexes an already-written arena segment.
    #[inline]
    fn dep_slice(&self, id: u64) -> &'a [f64] {
        let (start, end) = self.offsets[id as usize];
        &self.arena[start..end]
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
    /// deeper than the plan limit. Column names and data shapes are checked by
    /// [`Self::eval`].
    ///
    /// # Arguments
    ///
    /// * `ast` - Expression tree to compile: column references, literals,
    ///   operators, conditionals and elementwise functions only.
    pub fn new(ast: Expr) -> crate::Result<Self> {
        let plan = build_plan(ast)?;
        Ok(Self { plan })
    }

    /// Evaluate the expression over `cols`.
    ///
    /// # Column length handling
    ///
    /// The output length is the length of the **first** column in `cols`.
    /// Columns shorter than that are NaN-padded at the tail; columns longer
    /// than that are truncated. Missing tail values therefore propagate as
    /// NaN rather than being silently zero-filled.
    ///
    /// The context maps column names to positions in `cols`. Every call
    /// recomputes values from the supplied columns.
    ///
    /// # Errors
    ///
    /// Returns an error when a referenced column is absent from `ctx` or
    /// `cols`.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Column-name to index map used to resolve `Expr` column references.
    /// * `cols` - Column arrays aligned with `ctx`; output length is the first
    ///   column's length.
    pub fn eval(&self, ctx: &SimpleContext, cols: &[&[f64]]) -> crate::Result<Vec<f64>> {
        let plan = &self.plan;
        tracing::debug!(
            row_count = cols.first().map(|c| c.len()).unwrap_or(0),
            plan_nodes = plan.nodes.len(),
            "evaluating compiled expression"
        );

        let len = cols.first().map(|c| c.len()).unwrap_or(0);
        let arena_elements = len.checked_mul(plan.nodes.len()).ok_or_else(|| {
            crate::Error::Validation("expression arena size overflows usize".to_string())
        })?;
        // DAG node ids are dense positions in `plan.nodes`, so offsets are
        // indexed by id directly.
        let mut arena = vec![0.0; arena_elements];
        let mut offsets = vec![(0, 0); plan.nodes.len()];

        let mut cursor = 0;
        for node in &plan.nodes {
            let start = cursor;
            let end = cursor + len;

            // Split the arena to avoid borrow conflicts
            let (arena_deps, arena_out) = arena.split_at_mut(start);
            let out_slice = &mut arena_out[..len];
            let env = EvalNodeContext {
                ctx,
                cols,
                arena: arena_deps,
                offsets: &offsets,
            };
            Self::eval_node_into(&env, node, out_slice)?;

            offsets[node.id as usize] = (start, end);
            cursor = end;
        }

        let (start, end) = offsets[plan.root as usize];
        Ok(arena[start..end].to_vec())
    }

    /// Evaluate a single DAG node directly into a provided output slice (arena-based).
    fn eval_node_into(
        env: &EvalNodeContext<'_>,
        node: &super::dag::DagNode,
        out: &mut [f64],
    ) -> crate::Result<()> {
        match &node.operation {
            Operation::Column(name) => {
                let Some(idx) = env.ctx.index_of(name) else {
                    return Err(crate::InputError::NotFound {
                        id: format!("expr column:{name}"),
                    }
                    .into());
                };
                let Some(col_data) = env.cols.get(idx) else {
                    return Err(crate::Error::Validation(format!(
                        "Expression context resolved column '{name}' to index {idx}, but only {} data columns were provided",
                        env.cols.len()
                    )));
                };
                let len = out.len().min(col_data.len());
                out[..len].copy_from_slice(&col_data[..len]);
                out[len..].fill(f64::NAN);
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
                Self::eval_function_into(*func, &arg_slices, out)?;
            }
            Operation::BinOp(op) => {
                let left = env.dep_slice(node.dependencies[0]);
                let right = env.dep_slice(node.dependencies[1]);
                Self::eval_bin_op_into(*op, left, right, out);
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
    fn eval_bin_op_into(op: super::ast::BinOp, left: &[f64], right: &[f64], out: &mut [f64]) {
        use super::ast::BinOp;
        let len = out.len();

        for (i, out_val) in out.iter_mut().enumerate().take(len) {
            let (Some(&l), Some(&r)) = (left.get(i), right.get(i)) else {
                *out_val = f64::NAN;
                continue;
            };

            *out_val = match op {
                // Arithmetic
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => {
                    if r == 0.0 {
                        f64::NAN
                    } else {
                        l / r
                    }
                }
                BinOp::Mod => l % r,

                // Comparison (return 1.0 for true, 0.0 for false)
                // Exact equality semantics for expression-language operators.
                #[allow(clippy::float_cmp)]
                BinOp::Eq => {
                    if l == r {
                        1.0
                    } else {
                        0.0
                    }
                }
                #[allow(clippy::float_cmp)]
                BinOp::Ne => {
                    if l != r {
                        1.0
                    } else {
                        0.0
                    }
                }
                BinOp::Lt => {
                    if l < r {
                        1.0
                    } else {
                        0.0
                    }
                }
                BinOp::Le => {
                    if l <= r {
                        1.0
                    } else {
                        0.0
                    }
                }
                BinOp::Gt => {
                    if l > r {
                        1.0
                    } else {
                        0.0
                    }
                }
                BinOp::Ge => {
                    if l >= r {
                        1.0
                    } else {
                        0.0
                    }
                }

                // Logical (treat non-zero as true)
                BinOp::And => {
                    if l != 0.0 && r != 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                BinOp::Or => {
                    if l != 0.0 || r != 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
            };
        }
    }

    /// Evaluate a binary operation element-wise.
    #[inline]
    fn eval_unary_op_into(op: super::ast::UnaryOp, operand: &[f64], out: &mut [f64]) {
        use super::ast::UnaryOp;
        let len = out.len().min(operand.len());
        for i in 0..len {
            out[i] = match op {
                UnaryOp::Neg => -operand[i],
                UnaryOp::Not => {
                    if operand[i] == 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
            };
        }
        out[len..].fill(f64::NAN);
    }

    /// Evaluate if-then-else element-wise into a provided output slice.
    #[inline]
    fn eval_if_then_else_into(
        condition: &[f64],
        then_vals: &[f64],
        else_vals: &[f64],
        out: &mut [f64],
    ) {
        let len = out.len();
        for (i, out_val) in out.iter_mut().enumerate().take(len) {
            let (Some(&cond), Some(&then_val), Some(&else_val)) =
                (condition.get(i), then_vals.get(i), else_vals.get(i))
            else {
                *out_val = f64::NAN;
                continue;
            };
            *out_val = if cond != 0.0 { then_val } else { else_val };
        }
    }

    /// Evaluate a function directly into `out`.
    ///
    /// Every elementwise arm writes the whole output slice, NaN-filling a
    /// short result. Any other function was rejected by [`Self::new`].
    fn eval_function_into(
        fun: Function,
        arg_slices: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        match fun {
            Function::Abs => Self::eval_unary_into(arg_slices, out, f64::abs),
            Function::Sign => Self::eval_unary_into(arg_slices, out, Self::sign_or_zero),
            Function::Pow => Self::eval_pow_into(arg_slices, out),
            Function::Round => Self::eval_round_into(arg_slices, out)?,
            Function::Floor => Self::eval_unary_into(arg_slices, out, f64::floor),
            Function::Ceil => Self::eval_unary_into(arg_slices, out, f64::ceil),
            Function::Ln => Self::eval_unary_into(arg_slices, out, f64::ln),
            Function::Exp => Self::eval_unary_into(arg_slices, out, f64::exp),
            Function::Log10 => Self::eval_unary_into(arg_slices, out, f64::log10),
            Function::Sqrt => Self::eval_unary_into(arg_slices, out, f64::sqrt),
            Function::Clamp => Self::eval_clamp_into(arg_slices, out),
            Function::IsMissing => {
                Self::eval_unary_into(
                    arg_slices,
                    out,
                    |value| {
                        if value.is_finite() {
                            0.0
                        } else {
                            1.0
                        }
                    },
                );
            }
            _ => {
                debug_assert!(
                    !fun.is_scalar_evaluable(),
                    "Function::is_scalar_evaluable disagrees with eval dispatch for {fun:?}"
                );
                return Err(crate::Error::Validation(format!(
                    "Expression function '{fun}' is a statements-layer function; evaluate it via the statements crate instead of core::expr::eval"
                )));
            }
        }
        Ok(())
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
    fn eval_plans_if_binop_and_unary_nodes() {
        let (ctx, data) = sample_context();
        let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

        let condition = Expr::bin_op(BinOp::Gt, Expr::column("x"), Expr::column("y"));
        let then_branch = Expr::column("x");
        let else_branch = Expr::unary_op(UnaryOp::Neg, Expr::column("y"));
        let expr = Expr::if_then_else(condition, then_branch, else_branch);

        let compiled = CompiledExpr::new(expr).expect("elementwise expression compiles");
        let result = compiled.eval(&ctx, &cols).unwrap();

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
}
