//! Scalar expression evaluation with DAG optimization.
//!
//! Provides the `CompiledExpr` type that evaluates expression trees using
//! optimized scalar algorithms. Supports DAG planning for shared sub-expression
//! elimination: structurally identical sub-trees are deduplicated and each DAG
//! node is evaluated exactly once per `eval()` call.
//!
//! # Evaluation Strategy
//!
//! - **Simple mode**: Direct recursive evaluation (no planning)
//! - **DAG mode**: Topological order execution with sub-expression dedup
//! - **Scratch buffers**: Reused to minimize allocations
//! - **Deterministic**: Identical results across runs

use super::{
    ast::*,
    context::SimpleContext,
    dag::{DagBuilder, ExecutionPlan},
};
use crate::collections::HashMap;
use smallvec::SmallVec;
use std::sync::{Mutex, OnceLock};
use std::vec::Vec;

/// Options controlling expression evaluation strategy.
///
/// Allows callers to override the execution plan for a single evaluation.
/// Useful for scenario analysis where different plans may be beneficial.
///
/// # Fields
///
/// - `plan`: Internal optional pre-built execution plan, exposed through
///   [`EvalOpts::has_plan`]. **Not part of the wire format** — the field is
///   `#[serde(skip)]` so a deserialized `EvalOpts` can never inject an
///   arbitrary execution plan that `eval()` would execute in place of the
///   compiled AST; plans can only be attached in-process.
/// - `cache_budget_mb`: Retained for API/serde compatibility; no-op (see below)
/// - `max_arena_bytes`: Maximum scratch arena allocation in bytes
///
/// # Serde
///
/// Deserialization is strict (`deny_unknown_fields`): unknown fields —
/// including `plan`, which older versions serialized — are rejected.
///
/// # Examples
///
/// ```rust
/// use finstack_core::expr::{CompiledExpr, Expr, SimpleContext, EvalOpts};
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
    /// Optional pre-built execution plan to follow. If not provided, the
    /// evaluator will either use the internal plan (if present) or fallback to
    /// a minimal evaluation path for the expression.
    ///
    /// Skipped by serde: the plan (with its internal `DagNode` topology) is
    /// not wire format, and a deserialized `EvalOpts` must not be able to
    /// inject a plan that `eval()` executes instead of the compiled AST.
    #[serde(skip)]
    pub(crate) plan: Option<ExecutionPlan>,
    /// No-op, retained for API and serde compatibility.
    ///
    /// The cross-evaluation result cache was removed: it keyed entries on
    /// `(dag_node_id, len)` with no input fingerprint, so re-evaluating the
    /// same expression on different same-length data returned stale results.
    /// Within a single `eval()` call each deduplicated DAG node already
    /// executes exactly once, so no per-evaluation cache is needed either.
    pub cache_budget_mb: Option<usize>,
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
            plan: None,
            cache_budget_mb: None,
            max_arena_bytes: default_max_arena_bytes(),
        }
    }
}

impl EvalOpts {
    /// Return whether an explicit execution plan is attached.
    pub fn has_plan(&self) -> bool {
        self.plan.is_some()
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
    pub fn try_new_scalar(ast: Expr) -> crate::Result<Self> {
        super::ast_walk::ensure_scalar_evaluable(&ast)?;
        Ok(Self::new(ast))
    }

    /// Construct with DAG planning enabled.
    ///
    /// The provided `meta` is stored on the execution plan and stamped into
    /// each [`EvaluationResult`] produced by [`Self::eval`].
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

    /// No-op, retained for API compatibility.
    ///
    /// The cross-evaluation result cache was removed because it keyed entries
    /// on `(dag_node_id, len)` without an input fingerprint, returning stale
    /// values when the same expression was re-evaluated on different
    /// same-length data (see ,
    /// ). Within one `eval()` call each deduplicated DAG node is
    /// already evaluated exactly once.
    pub fn with_cache(self, _budget_mb: usize) -> Self {
        self
    }

    /// Return whether this compiled expression currently has an attached cache.
    ///
    /// Always `false`: the cross-evaluation cache was removed (see
    /// [`Self::with_cache`]).
    pub fn has_cache(&self) -> bool {
        false
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
    pub fn eval(
        &self,
        ctx: &SimpleContext,
        cols: &[&[f64]],
        opts: EvalOpts,
    ) -> crate::Result<EvaluationResult> {
        // Decide on execution plan preference: opts > self > lazy-cached auto-build.
        // Use references to avoid cloning ExecutionPlan (which contains Vec<DagNode>
        // with recursive Expr trees). Only build a new owned plan when none exists.
        let owned_plan;
        let plan_to_use: &ExecutionPlan = if let Some(ref plan) = opts.plan {
            plan
        } else if let Some(ref plan) = self.plan {
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
                    // Race: another thread set it first. Use theirs (already cached).
                    // Keep our plan alive for this call as a fallback.
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

        // Compute values using the chosen strategy
        let values: Vec<f64> = {
            // Execute nodes in topological order using arena allocation
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

            // Pre-allocate arena for all node results to avoid per-node Vec allocations
            let mut arena = vec![0.0; arena_elements];
            let mut offsets: HashMap<u64, (usize, usize)> = HashMap::default();
            let mut cursor = 0;

            for node in &plan_to_use.nodes {
                // Allocate space in arena for this node's result
                let start = cursor;
                let end = cursor + len;

                // Evaluate node directly into arena slice
                // Split the arena to avoid borrow conflicts
                let (arena_deps, arena_out) = arena.split_at_mut(start);
                let out_slice = &mut arena_out[..len];
                self.eval_node_into(ctx, cols, node, arena_deps, &offsets, out_slice)?;

                offsets.insert(node.id, (start, end));
                cursor = end;
            }

            // Extract root result
            plan_to_use
                .roots
                .first()
                .and_then(|&root_id| offsets.get(&root_id))
                .map(|&(start, end)| arena[start..end].to_vec())
                .unwrap_or_default()
        };

        // Stamp the metadata carried by the execution plan (set by the caller
        // via `with_planning` or `EvalOpts.plan`); auto-built plans carry the
        // default-config snapshot. The evaluator does not record
        // timings/cache/parallel.
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
        offsets: &HashMap<u64, (usize, usize)>,
        out: &mut [f64],
    ) -> crate::Result<()> {
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
            ExprNode::CSRef { .. } => {
                return Err(crate::Error::Validation(
                    "capital-structure references require the statements evaluator".to_string(),
                ));
            }
            ExprNode::Literal(val) => {
                out.fill(*val);
            }
            ExprNode::Call(func, _args) => {
                // Get argument results from dependencies (slices from arena)
                let arg_slices: SmallVec<[&[f64]; 4]> = node
                    .dependencies
                    .iter()
                    .filter_map(|&dep_id| {
                        offsets.get(&dep_id).map(|&(start, end)| &arena[start..end])
                    })
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
                let left = offsets
                    .get(&node.dependencies[0])
                    .map(|&(start, end)| &arena[start..end])
                    .ok_or_else(|| {
                        crate::Error::Validation(format!(
                            "Binary expression node {} is missing its left dependency result",
                            node.id
                        ))
                    })?;
                let right = offsets
                    .get(&node.dependencies[1])
                    .map(|&(start, end)| &arena[start..end])
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
                let operand = offsets
                    .get(&node.dependencies[0])
                    .map(|&(start, end)| &arena[start..end])
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
                let condition = offsets
                    .get(&node.dependencies[0])
                    .map(|&(start, end)| &arena[start..end])
                    .ok_or_else(|| {
                        crate::Error::Validation(format!(
                            "If-then-else node {} is missing its condition result",
                            node.id
                        ))
                    })?;
                let then_vals = offsets
                    .get(&node.dependencies[1])
                    .map(|&(start, end)| &arena[start..end])
                    .ok_or_else(|| {
                        crate::Error::Validation(format!(
                            "If-then-else node {} is missing its then-branch result",
                            node.id
                        ))
                    })?;
                let else_vals = offsets
                    .get(&node.dependencies[2])
                    .map(|&(start, end)| &arena[start..end])
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
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::config::FinstackConfig;
    use crate::expr::{BinOp, Expr, Function, SimpleContext, UnaryOp};

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
    fn eval_allows_external_plan_override() {
        let (ctx, data) = sample_context();
        let cols: Vec<&[f64]> = data.iter().map(|v| v.as_slice()).collect();
        let expr = Expr::call(Function::Diff, vec![Expr::column("x"), Expr::literal(1.0)]);
        let meta = crate::config::results_meta(&FinstackConfig::default());
        let compiled = CompiledExpr::with_planning(expr, meta).unwrap();
        let external_plan = compiled.plan.clone();

        let result = compiled
            .eval(
                &ctx,
                &cols,
                EvalOpts {
                    plan: external_plan,
                    cache_budget_mb: None,
                    max_arena_bytes: default_max_arena_bytes(),
                },
            )
            .unwrap()
            .values;

        assert!(result[0].is_nan());
        assert!((result[1] - 0.3).abs() < 1e-12);
        assert!((result[2] - 2.5).abs() < 1e-12);
        assert!((result[3] - 1.0).abs() < 1e-12);
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
            ..EvalOpts::default()
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
        let opts = EvalOpts {
            max_arena_bytes: 0,
            ..EvalOpts::default()
        };
        let result = expr.eval(&ctx, &cols, opts);
        assert!(result.is_ok());
    }
}
