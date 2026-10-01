//! Expression engine with DAG planning and scalar evaluation.
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
//!   domain-specific rolling estimator; `quantile` here is a global reducer.
//! - rolling_min / rolling_max / rolling_count
//! - ewm_std / ewm_var
//!
//! Evaluation supports:
//! - DAG planning with shared sub-expression detection (each deduplicated
//!   node is evaluated exactly once per `eval()` call)
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
//! `std` and `var` use population estimators (denominator `n`, `ddof=0`),
//! excluding NaNs. They broadcast NaN when fewer than two observations remain.
//! Rolling variance and standard deviation also use `ddof=0`, but require a
//! complete window with no NaNs. EWM variance/std use normalized population
//! moments without sample-bias correction and start at zero for one observation.
//! Rolling sum/mean propagate NaNs within the window and recover after they
//! expire; a single sign of infinity yields that infinity, while both signs
//! yield NaN until one sign leaves the window.
//!
//! # Quick example
//!
//! ```rust
//! use finstack_quant_core::expr::{Expr, Function, CompiledExpr, SimpleContext, EvalOpts};
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
//!
//! // Population variance of [1, 2, 3, 4, 5] is 2, broadcast to every row.
//! let variance = CompiledExpr::new(Expr::call(Function::Var, vec![Expr::column("x")]));
//! assert_eq!(variance.eval(&context, &cols, EvalOpts::default())?.values, vec![2.0; 5]);
//! # Ok::<(), finstack_quant_core::Error>(())
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
//! Exponential weights are normalized over the observed history when `adjust`
//! is enabled; the recursive form seeds from the first non-NaN observation.
//! Missing observations emit NaN without advancing either weighting mode.

mod ast;
mod ast_walk;
mod context;
mod dag;
mod eval;
mod eval_functions;

pub use ast::{BinOp, EvaluationResult, Expr, ExprNode, Function, UnaryOp};
pub use context::SimpleContext;
pub use eval::{CompiledExpr, EvalOpts};
