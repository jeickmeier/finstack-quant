//! Expression engine: a shared AST and a scalar DAG evaluator.
//!
//! [`Expr`](crate::expr::Expr)/[`ExprNode`](crate::expr::ExprNode) build the
//! tree and [`Function`](crate::expr::Function) enumerates every operator the
//! `statements` DSL and the valuations composite instrument share. Only the
//! elementwise subset evaluates here:
//! - abs / sign / pow / round / floor / ceil / ln / exp / log10 / sqrt /
//!   clamp / is_missing
//! - arithmetic, comparison and logical operators, `if … then … else`
//!
//! Windowed, cumulative, positional and reducing functions (`lag`, `diff`,
//! `rolling_*`, `cumsum`, `ewm_*`, `std`, `median`, `rank`, `quantile`, …)
//! and the period-aware functions (`ttm`, `ytd`, `growth_rate`, …) are
//! evaluated by the `statements` crate's period evaluator;
//! [`CompiledExpr::new`](crate::expr::CompiledExpr::new) rejects them
//! ([`Function::is_scalar_evaluable`](crate::expr::Function::is_scalar_evaluable)).
//!
//! # Execution model
//!
//! Expressions operate over column-oriented numeric arrays. A
//! [`SimpleContext`](crate::expr::SimpleContext) maps column names to column
//! positions, [`CompiledExpr`](crate::expr::CompiledExpr) plans the expression
//! once (structurally identical sub-trees are evaluated exactly once per call)
//! and evaluation returns an
//! [`EvaluationResult`](crate::expr::EvaluationResult) containing both values
//! and metadata describing the run. Results are deterministic for the same
//! inputs.
//!
//! # Quick example
//!
//! ```rust
//! use finstack_quant_core::expr::{BinOp, CompiledExpr, Expr, Function, SimpleContext};
//!
//! // abs(x - 3)
//! let expr = Expr::call(
//!     Function::Abs,
//!     vec![Expr::bin_op(BinOp::Sub, Expr::column("x"), Expr::literal(3.0))],
//! );
//! let compiled = CompiledExpr::new(expr)?;
//! let context = SimpleContext::new(["x"]).expect("unique columns");
//! let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
//! let cols = [data.as_slice()];
//! let result = compiled.eval(&context, &cols)?;
//! assert_eq!(result.values, vec![2.0, 1.0, 0.0, 1.0, 2.0]);
//!
//! // Windowed functions belong to the statements evaluator.
//! let rolling = Expr::call(
//!     Function::RollingMean,
//!     vec![Expr::column("x"), Expr::literal(3.0)],
//! );
//! assert!(CompiledExpr::new(rolling).is_err());
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

mod ast;
mod context;
mod dag;
mod eval;
mod eval_functions;

pub use ast::{BinOp, EvaluationResult, Expr, ExprNode, Function, UnaryOp};
pub use context::SimpleContext;
pub use eval::CompiledExpr;
