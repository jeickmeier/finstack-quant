//! Expression engine test suite.
//!
//! Organized by concern:
//! - `ast`: AST construction and structural equality
//! - `context`: `SimpleContext` column resolution
//! - `eval`: Core evaluation infrastructure
//! - `functions`: Function-specific behavior tests
//! - `serde`: Serialization/deserialization tests

#[path = "expr/ast.rs"]
mod ast;

#[path = "expr/context.rs"]
mod context;

#[path = "expr/eval.rs"]
mod eval;

#[path = "expr/functions.rs"]
mod functions;

#[path = "expr/serde.rs"]
mod expr_serde;
