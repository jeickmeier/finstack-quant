//! Scalar expression evaluation with DAG optimization.
//!
//! Provides the `CompiledExpr` type that evaluates expression trees using
//! optimized scalar algorithms. Supports DAG planning for shared sub-expression
//! elimination: structurally identical sub-trees are deduplicated and each DAG
//! node is evaluated exactly once per `eval()` call.
//!
//! # Evaluation Strategy
//!
//! - **Planning**: Eager compilation or a lazily cached DAG plan
//! - **DAG mode**: Topological order execution with sub-expression dedup
//! - **Scratch buffers**: Reused to minimize allocations
//! - **Deterministic**: Identical results across runs

use super::{
    ast::*,
    context::SimpleContext,
    dag::{DagBuilder, ExecutionPlan},
};
use smallvec::SmallVec;
use std::sync::{Mutex, OnceLock};
use std::vec::Vec;

/// Options controlling expression evaluation strategy.
///
/// Limits scratch arena allocation while the compiled expression owns its plan.
/// Deserialization rejects unknown fields, including execution-plan overrides.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::expr::{CompiledExpr, Expr, SimpleContext, EvalOpts};
///
/// let ctx = SimpleContext::new(["x"]).expect("unique columns");
/// let x = vec![1.0, 2.0, 3.0];
/// let cols: [&[f64]; 1] = [&x];
/// let expr = CompiledExpr::new(Expr::column("x"));
///
/// let out = expr.eval(&ctx, &cols, EvalOpts::default()).expect("column lookup should succeed");
/// assert_eq!(out.values, vec![1.0, 2.0, 3.0]);
/// ```
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalOpts {
    /// Maximum arena allocation in bytes. Defaults to 1 GB.
    /// Set to 0 to disable the check.
    #[serde(default = "default_max_arena_bytes")]
    pub max_arena_bytes: usize,
}

fn default_max_arena_bytes() -> usize {
    1_073_741_824
}

impl Default for EvalOpts {
    fn default() -> Self {
        Self {
            max_arena_bytes: default_max_arena_bytes(),
        }
    }
}

/// Compiled expression with optimized evaluation.
///
/// Wraps an expression AST with optional DAG planning for efficient evaluation
/// of complex formulas. Used extensively in financial statement models where
/// hundreds of interdependent formulas must be evaluated.
///
/// # Components
///
/// - **AST**: Expression tree to evaluate
/// - **Plan**: Optional execution plan (topological order)
/// - **Scratch buffers**: Reused temporary storage to minimize allocations
///
/// # Evaluation Modes
///
/// - **Simple**: Direct recursive evaluation (fast for simple expressions)
/// - **DAG-optimized**: Shared sub-expression elimination (best for complex graphs)
///
/// # Thread Safety
///
/// `CompiledExpr` is both `Send` and `Sync`. Internal scratch buffers are
/// protected by `Mutex`. For parallel evaluation, either share a
/// single instance (concurrent `eval()` calls will serialize on the scratch
/// `Mutex`) or clone for independent scratch buffers per thread.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CompiledExpr {
    /// Underlying expression AST.
    pub ast: Expr,
    /// Optional execution plan for complex expressions.
    pub(crate) plan: Option<ExecutionPlan>,
    /// Small scratch arena to reuse temporary buffers within hot paths.
    #[serde(skip, default = "default_scratch")]
    pub(super) scratch: Mutex<ScratchArena>,
    /// Lazily-built fallback plan, populated on first `eval()` when `plan` is None.
    /// Prevents rebuilding the DAG on every call for expressions created via `new()`.
    #[serde(skip)]
    lazy_plan: OnceLock<ExecutionPlan>,
}

fn default_scratch() -> Mutex<ScratchArena> {
    Mutex::new(ScratchArena::default())
}

/// Tiny reusable scratch buffers for hot evaluation paths.
#[derive(Default, Debug)]
pub(super) struct ScratchArena {
    /// Generic temporary buffer for algorithms (e.g., median, sorts).
    pub(super) tmp: Vec<f64>,
    /// Window buffer for rolling operations that need a writable copy.
    pub(super) window: Vec<f64>,
    /// Node-result arena reused across `eval()` calls to avoid reallocating
    /// `len * node_count` f64s every evaluation.
    pub(super) arena: Vec<f64>,
    /// Per-node result offsets indexed by DAG node id, reused across `eval()`
    /// calls. Replaces a per-call `HashMap<u64, (usize, usize)>`; node ids are
    /// dense, so direct indexing avoids hashing on every dependency lookup.
    pub(super) offsets: Vec<Option<(usize, usize)>>,
}

impl Clone for CompiledExpr {
    fn clone(&self) -> Self {
        Self {
            ast: self.ast.clone(),
            plan: self.plan.clone(),
            // Fresh scratch and lazy_plan for clones; per-instance reuse only.
            scratch: Mutex::new(ScratchArena::default()),
            lazy_plan: OnceLock::new(),
        }
    }
}

impl CompiledExpr {
    /// Construct a new compiled expression from an AST.
    ///
    /// Accepts any [`Expr`], including statements-layer functions (`Ttm`,
    /// `Ytd`, etc.); those will fail at `eval()` time with a typed validation
    /// error. Callers that know they are operating on a scalar evaluator may
    /// prefer [`Self::try_new_scalar`] to fail fast at compile time.
    ///
    /// # Arguments
    ///
    /// * `ast` - Expression tree to compile. Statements-layer functions such as
    ///   `Ttm` are accepted here and fail at [`Self::eval`].
    pub fn new(ast: Expr) -> Self {
        Self {
            ast,
            plan: None,
            scratch: Mutex::new(ScratchArena::default()),
            lazy_plan: OnceLock::new(),
        }
    }

    /// Construct a compiled expression and reject statements-layer functions
    /// up front.
    ///
    /// Use this when you know the expression must be evaluable by the core
    /// scalar evaluator (i.e., not under the `statements` crate). Period-aware
    /// functions like `Ttm`/`Ytd`/`GrowthRate` etc. return a typed validation
    /// error instead of being silently accepted and rejected at eval time.
    ///
    /// # Errors
    ///
    /// Returns an error when `ast` contains a statements-layer or other
    /// period-aware operation that the scalar evaluator cannot execute. It
    /// does not validate column names or data shapes; those are checked by
    /// [`Self::eval`].
    ///
    /// # Arguments
    ///
    /// * `ast` - Expression tree that must be evaluable by the scalar engine;
    ///   period-aware functions are rejected immediately.
    pub fn try_new_scalar(ast: Expr) -> crate::Result<Self> {
        super::ast_walk::ensure_scalar_evaluable(&ast)?;
        Ok(Self::new(ast))
    }

    /// Construct with DAG planning enabled.
    ///
    /// The provided `meta` is stored on the execution plan and stamped into
    /// each [`EvaluationResult`] produced by [`Self::eval`].
    ///
    /// The plan deduplicates structurally identical subexpressions and
    /// evaluates each planned node once per call. Use [`Self::new`] when this
    /// up-front planning cost is not appropriate; it builds and caches the
    /// equivalent plan lazily on the first evaluation.
    ///
    /// # Arguments
    ///
    /// * `ast` - Owned expression tree to plan, deduplicating structurally
    ///   identical subexpressions before evaluation.
    /// * `meta` - Result metadata stored in the execution plan and copied into
    ///   each evaluation result.
    ///
    /// # Errors
    ///
    /// Returns an error if the expression graph cannot be converted into a
    /// valid execution plan, for example because planning detects an invalid
    /// dependency structure or more than 512 nested expression levels.
    pub fn with_planning(ast: Expr, meta: crate::config::ResultsMeta) -> crate::Result<Self> {
        let mut builder = DagBuilder::new();
        let plan = builder.build_plan(vec![ast.clone()], meta)?;

        Ok(Self {
            ast,
            plan: Some(plan),
            scratch: Mutex::new(ScratchArena::default()),
            lazy_plan: OnceLock::new(),
        })
    }

    /// Return whether this compiled expression has a pre-built execution plan.
    pub fn has_plan(&self) -> bool {
        self.plan.is_some()
    }

    /// Unified evaluation entrypoint returning values with execution metadata.
    ///
    /// Uses scalar implementations for all functions, with optional DAG planning
    /// for complex expressions.
    ///
    /// # Column length handling
    ///
    /// The output length is the length of the **first** column in `cols`.
    /// Columns shorter than that are NaN-padded at the tail; columns longer
    /// than that are truncated. Missing tail values therefore propagate as
    /// NaN rather than being silently zero-filled.
    ///
    /// The context maps column names to positions in `cols`. A plan attached
    /// via [`Self::with_planning`] is reused; otherwise a plan is built once
    /// and cached. Auto-built plans carry a default-config metadata snapshot.
    /// Evaluation does not add timing or parallelism data. Every call
    /// recomputes values from the supplied columns.
    ///
    /// # Errors
    ///
    /// Returns an error when planning fails, a referenced column is absent
    /// from `ctx` or `cols`, an expression operation is invalid for scalar
    /// evaluation, a planned dependency is unavailable, or the scratch arena
    /// would overflow or exceed `opts.max_arena_bytes`. A zero arena limit
    /// disables only the configured size limit, not arithmetic-overflow
    /// protection.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Column-name to index map used to resolve `Expr` column references.
    /// * `cols` - Column arrays aligned with `ctx`; output length is the first
    ///   column's length.
    /// * `opts` - Arena size limit; `max_arena_bytes = 0` disables the configured cap.
    pub fn eval(
        &self,
        ctx: &SimpleContext,
        cols: &[&[f64]],
        opts: EvalOpts,
    ) -> crate::Result<EvaluationResult> {
        let owned_plan;
        let plan_to_use: &ExecutionPlan = if let Some(ref plan) = self.plan {
            plan
        } else if let Some(plan) = self.lazy_plan.get() {
            plan
        } else {
            let mut builder = DagBuilder::new();
            let meta = crate::config::results_meta(&crate::config::FinstackConfig::default());
            let plan = builder.build_plan(vec![self.ast.clone()], meta)?;
            // Try to cache for future calls; if a racing thread beat us, use theirs.
            match self.lazy_plan.set(plan) {
                Ok(()) => self.lazy_plan.get().ok_or_else(|| {
                    crate::Error::Internal(
                        "expression lazy plan missing immediately after OnceLock::set".to_string(),
                    )
                })?,
                Err(plan) => {
                    owned_plan = plan;
                    self.lazy_plan.get().unwrap_or(&owned_plan)
                }
            }
        };

        tracing::debug!(
            row_count = cols.first().map(|c| c.len()).unwrap_or(0),
            plan_nodes = plan_to_use.nodes.len(),
            "evaluating compiled expression"
        );

        let values: Vec<f64> = {
            let len = cols.first().map(|c| c.len()).unwrap_or(0);
            let node_count = plan_to_use.nodes.len();
            let arena_elements = len.checked_mul(node_count).ok_or_else(|| {
                crate::Error::from(crate::InputError::TooLarge {
                    what: "expression arena".into(),
                    requested_bytes: usize::MAX,
                    limit_bytes: opts.max_arena_bytes,
                })
            })?;
            let arena_bytes = arena_elements.saturating_mul(std::mem::size_of::<f64>());
            if opts.max_arena_bytes > 0 && arena_bytes > opts.max_arena_bytes {
                return Err(crate::InputError::TooLarge {
                    what: "expression arena".into(),
                    requested_bytes: arena_bytes,
                    limit_bytes: opts.max_arena_bytes,
                }
                .into());
            }

            // DAG node ids are dense (assigned 0..next_id during plan build), so
            // index offsets by id directly instead of hashing.
            let id_capacity = plan_to_use
                .nodes
                .iter()
                .map(|n| n.id as usize)
                .max()
                .map_or(0, |m| m + 1);

            // Borrow the pooled arena/offsets buffers (retaining capacity across
            // calls) and release the scratch lock immediately so nested
            // median/rolling ops can re-lock the same scratch without deadlock.
            let (mut arena, mut offsets) = {
                let mut guard = self
                    .scratch
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                (
                    std::mem::take(&mut guard.arena),
                    std::mem::take(&mut guard.offsets),
                )
            };
            // Every node overwrites its slice, so retain a larger arena and
            // avoid clearing and re-zeroing it on each call.
            if arena.len() < arena_elements {
                arena.resize(arena_elements, 0.0);
            }
            // Reset offsets so a failed prior evaluation cannot leak stale data.
            offsets.clear();
            offsets.resize(id_capacity, None);

            let mut cursor = 0;
            let eval_result: crate::Result<Vec<f64>> = (|| {
                for node in &plan_to_use.nodes {
                    let start = cursor;
                    let end = cursor + len;

                    // Split the arena to avoid borrow conflicts
                    let (arena_deps, arena_out) = arena.split_at_mut(start);
                    let out_slice = &mut arena_out[..len];
                    self.eval_node_into(ctx, cols, node, arena_deps, &offsets, out_slice)?;

                    offsets[node.id as usize] = Some((start, end));
                    cursor = end;
                }

                Ok(plan_to_use
                    .roots
                    .first()
                    .and_then(|&root_id| offsets.get(root_id as usize).copied().flatten())
                    .map(|(start, end)| arena[start..end].to_vec())
                    .unwrap_or_default())
            })();

            // Return the buffers to the pool for reuse, even on error.
            {
                let mut guard = self
                    .scratch
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard.arena = arena;
                guard.offsets = offsets;
            }

            eval_result?
        };

        let meta = plan_to_use.meta.clone();

        Ok(EvaluationResult {
            values,
            metadata: meta,
        })
    }

    /// Evaluate a single DAG node directly into a provided output slice (arena-based).
    fn eval_node_into(
        &self,
        ctx: &SimpleContext,
        cols: &[&[f64]],
        node: &super::dag::DagNode,
        arena: &[f64],
        offsets: &[Option<(usize, usize)>],
        out: &mut [f64],
    ) -> crate::Result<()> {
        // Look up a dependency's arena offset by (dense) node id.
        let dep_offset = |id: u64| offsets.get(id as usize).copied().flatten();
        match &node.expr.node {
            ExprNode::Column(name) => {
                let Some(idx) = ctx.index_of(name) else {
                    return Err(crate::error::InputError::NotFound {
                        id: format!("expr column:{name}"),
                    }
                    .into());
                };
                let Some(col_data) = cols.get(idx) else {
                    return Err(crate::Error::Validation(format!(
                        "Expression context resolved column '{name}' to index {idx}, but only {} data columns were provided",
                        cols.len()
                    )));
                };
                let len = out.len().min(col_data.len());
                out[..len].copy_from_slice(&col_data[..len]);
                out[len..].fill(f64::NAN);
            }
            ExprNode::CsRef { .. } => {
                return Err(crate::Error::Validation(
                    "capital-structure references require the statements evaluator".to_string(),
                ));
            }
            ExprNode::Literal(val) => {
                out.fill(*val);
            }
            ExprNode::Call(func, _args) => {
                let arg_slices: SmallVec<[&[f64]; 4]> = node
                    .dependencies
                    .iter()
                    .filter_map(|&dep_id| dep_offset(dep_id).map(|(start, end)| &arena[start..end]))
                    .collect();

                if arg_slices.len() != node.dependencies.len() {
                    return Err(crate::Error::Validation(format!(
                        "Expression DAG node {} is missing {} dependency results",
                        node.id,
                        node.dependencies.len() - arg_slices.len()
                    )));
                }
                self.eval_function_into(*func, &arg_slices, ctx, cols, out)?;
            }
            ExprNode::BinOp { op, .. } => {
                // Binary operations should have exactly 2 dependencies
                if node.dependencies.len() < 2 {
                    return Err(crate::Error::Validation(format!(
                        "Binary expression node {} is missing operands",
                        node.id
                    )));
                }
                let left = dep_offset(node.dependencies[0])
                    .map(|(start, end)| &arena[start..end])
                    .ok_or_else(|| {
                        crate::Error::Validation(format!(
                            "Binary expression node {} is missing its left dependency result",
                            node.id
                        ))
                    })?;
                let right = dep_offset(node.dependencies[1])
                    .map(|(start, end)| &arena[start..end])
                    .ok_or_else(|| {
                        crate::Error::Validation(format!(
                            "Binary expression node {} is missing its right dependency result",
                            node.id
                        ))
                    })?;
                Self::eval_bin_op_into(*op, left, right, out);
            }
            ExprNode::UnaryOp { op, .. } => {
                // Unary operations should have exactly 1 dependency
                if node.dependencies.is_empty() {
                    return Err(crate::Error::Validation(format!(
                        "Unary expression node {} is missing its operand",
                        node.id
                    )));
                }
                let operand = dep_offset(node.dependencies[0])
                    .map(|(start, end)| &arena[start..end])
                    .ok_or_else(|| {
                        crate::Error::Validation(format!(
                            "Unary expression node {} is missing its operand result",
                            node.id
                        ))
                    })?;
                Self::eval_unary_op_into(*op, operand, out);
            }
            ExprNode::IfThenElse { .. } => {
                // If-then-else should have exactly 3 dependencies
                if node.dependencies.len() < 3 {
                    return Err(crate::Error::Validation(format!(
                        "If-then-else expression node {} is missing one or more branch dependencies",
                        node.id
                    )));
                }
                let condition = dep_offset(node.dependencies[0])
                    .map(|(start, end)| &arena[start..end])
                    .ok_or_else(|| {
                        crate::Error::Validation(format!(
                            "If-then-else node {} is missing its condition result",
                            node.id
                        ))
                    })?;
                let then_vals = dep_offset(node.dependencies[1])
                    .map(|(start, end)| &arena[start..end])
                    .ok_or_else(|| {
                        crate::Error::Validation(format!(
                            "If-then-else node {} is missing its then-branch result",
                            node.id
                        ))
                    })?;
                let else_vals = dep_offset(node.dependencies[2])
                    .map(|(start, end)| &arena[start..end])
                    .ok_or_else(|| {
                        crate::Error::Validation(format!(
                            "If-then-else node {} is missing its else-branch result",
                            node.id
                        ))
                    })?;
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

    /// Evaluate a function with given argument results (slices from arena).
    fn eval_function_into(
        &self,
        fun: Function,
        arg_slices: &[&[f64]],
        _ctx: &SimpleContext,
        _cols: &[&[f64]],
        out: &mut [f64],
    ) -> crate::Result<()> {
        match fun {
            Function::Lag => self.eval_lag_into(arg_slices, out),
            Function::Lead => self.eval_lead_into(arg_slices, out),
            Function::Diff => self.eval_diff_into(arg_slices, out),
            Function::PctChange => self.eval_pct_change_into(arg_slices, out),
            Function::RollingMean => self.eval_rolling_mean_into(arg_slices, out),
            Function::RollingSum => self.eval_rolling_sum_into(arg_slices, out),
            Function::RollingStd => self.eval_rolling_std_into(arg_slices, out),
            Function::RollingVar => self.eval_rolling_var_into(arg_slices, out),
            Function::RollingMedian => self.eval_rolling_median_into(arg_slices, out),
            Function::Shift => self.eval_shift_into(arg_slices, out),
            Function::RollingMin => self.eval_rolling_min_into(arg_slices, out),
            Function::RollingMax => self.eval_rolling_max_into(arg_slices, out),
            Function::RollingCount => self.eval_rolling_count_into(arg_slices, out),
            _ => {
                let result = self.eval_function_core(fun, arg_slices, _ctx, _cols)?;
                let copy_len = out.len().min(result.len());
                out[..copy_len].copy_from_slice(&result[..copy_len]);
                if copy_len < out.len() {
                    out[copy_len..].fill(f64::NAN);
                }
                Ok(())
            }
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
    fn eval_auto_builds_plan_for_if_binop_and_unary_nodes() {
        let (ctx, data) = sample_context();
        let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();

        let condition = Expr::bin_op(BinOp::Gt, Expr::column("x"), Expr::column("y"));
        let then_branch = Expr::column("x");
        let else_branch = Expr::unary_op(UnaryOp::Neg, Expr::column("y"));
        let expr = Expr::if_then_else(condition, then_branch, else_branch);

        let compiled = CompiledExpr::new(expr);
        let result = compiled
            .eval(&ctx, &cols, EvalOpts::default())
            .unwrap()
            .values;

        assert_eq!(result.len(), 4);
        assert!((result[0] + 0.5).abs() < 1e-12);
        assert!((result[1] + 1.5).abs() < 1e-12);
        assert!((result[2] - 3.0).abs() < 1e-12);
        assert!((result[3] - 4.0).abs() < 1e-12);
    }

    #[test]
    fn arena_rejects_oversized_allocation() {
        let ast = Expr::bin_op(BinOp::Add, Expr::column("x"), Expr::column("y"));
        let expr = CompiledExpr::new(ast);

        let col: Vec<f64> = vec![1.0; 1000];
        let cols: Vec<&[f64]> = vec![&col, &col];
        let ctx = SimpleContext::new(["x", "y"]).expect("unique columns");

        let opts = EvalOpts {
            max_arena_bytes: 100,
        };
        let result = expr.eval(&ctx, &cols, opts);
        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(
            err_str.contains("too large") || err_str.contains("TooLarge"),
            "Expected TooLarge error, got: {err_str}"
        );
    }

    #[test]
    fn arena_accepts_normal_allocation() {
        let ast = Expr::column("x");
        let expr = CompiledExpr::new(ast);
        let col = vec![1.0, 2.0, 3.0];
        let cols: Vec<&[f64]> = vec![&col];
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let opts = EvalOpts::default();
        let result = expr.eval(&ctx, &cols, opts);
        assert!(result.is_ok());
    }

    #[test]
    fn arena_check_disabled_when_zero() {
        let ast = Expr::column("x");
        let expr = CompiledExpr::new(ast);
        let col = vec![1.0, 2.0, 3.0];
        let cols: Vec<&[f64]> = vec![&col];
        let ctx = SimpleContext::new(["x"]).expect("unique columns");
        let opts = EvalOpts { max_arena_bytes: 0 };
        let result = expr.eval(&ctx, &cols, opts);
        assert!(result.is_ok());
    }
}
