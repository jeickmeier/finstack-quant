//! Expression engine with DAG planning, caching, and scalar evaluation.
//!
//! Supported functions:
//! - lag(expr, n) / lead(expr, n)
//! - diff(expr, n) / pct_change(expr, n)
//! - cumsum / cumprod / cummin / cummax
//! - rolling_mean / rolling_sum (row windows)
//! - rolling_std / rolling_var / rolling_median
//! - ewm_mean(expr, alpha, adjust)
//! - std / var / median
//! - shift / rank / quantile (reducer over entire series; broadcasts scalar)
//!   - For rolling/windowed quantiles, use `rolling_median` or implement a
//!     domain-specific rolling estimator; `quantile` here is a global reducer.
//! - rolling_min / rolling_max / rolling_count
//! - ewm_std / ewm_var
//!
//! Evaluation supports:
//! - DAG planning with shared sub-expression detection
//! - Optional caching for intermediate results
//! - Scalar implementations over `&[f64]` inputs
//! - Deterministic execution
//! - Metadata stamping for results
//!
//! # Execution model
//!
//! Expressions operate over column-oriented numeric arrays. A
//! [`crate::expr::SimpleContext`] maps column names to column positions,
//! [`crate::expr::CompiledExpr`] plans the expression, and evaluation returns an
//! [`crate::expr::EvaluationResult`] containing both values and
//! metadata describing the run.
//!
//! For the higher-level architecture split between this vector engine and the
//! statements period-aware evaluator, see `book/src/architecture/analytics/expressions.md`.
//!
//! Windowed functions in this module use row-count windows rather than
//! calendar-time windows. Reducers such as `quantile` broadcast a single scalar
//! back across the output vector unless the function name explicitly says
//! `rolling_*`.
//!
//! # Quick example
//!
//! ```rust
//! use finstack_core::expr::{Expr, Function, CompiledExpr, SimpleContext, EvalOpts};
//!
//! // Create expression: rolling_mean(x, 3)
//! let expr = Expr::call(
//!     Function::RollingMean,
//!     vec![Expr::column("x"), Expr::literal(3.0)]
//! );
//!
//! // Compile and evaluate
//! let compiled = CompiledExpr::new(expr);
//! let context = SimpleContext::new(["x"]).expect("unique columns");
//! let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
//! let cols = [data.as_slice()];
//! let result = compiled.eval(&context, &cols, EvalOpts::default())?;
//! assert_eq!(result.values.len(), 5);
//! # Ok::<(), finstack_core::Error>(())
//! ```
//!
//! # Execution Strategy
//!
//! All functions are implemented as scalar operations over column slices:
//! 1. Intermediate buffers are reused during evaluation.
//! 2. Rolling functions use row-count windows.
//! 3. Results are deterministic for the same inputs and evaluation options.
//! 4. The module does not depend on external DataFrame libraries.
//!
//! # References
//!
//! - Exponential-weighted semantics are intended to be compatible with common
//!   pandas-style usage when parameters match.

mod ast;
mod ast_walk;
pub(crate) mod cache;
mod context;
mod dag;
mod eval;
mod eval_functions;

// Public API - simplified surface for end users
pub use ast::{BinOp, EvaluationResult, Expr, ExprNode, Function, UnaryOp};
pub use context::SimpleContext;
pub use eval::{CompiledExpr, EvalOpts};

// Polars Series no longer part of public API surface here
