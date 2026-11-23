//! WASM bindings for the `finstack-quant-statements` crate.
//!
//! Data crosses the boundary as JSON strings or plain objects; the stateful
//! pieces are classes:
//! - `Evaluator` evaluates a `FinancialModelSpec` (optionally with a market
//!   or under Monte Carlo) and can carry a check suite.
//! - `ModelBuilder` / `MixedNodeBuilder` assemble a model step by step, and
//!   `Registry` holds reusable metric definitions.
//!
//! Everything else is a function: spec validation (`validate*Json`), DSL
//! formula parsing, and the free-function twins of the Python result and
//! spec methods (`statementResult*`, `capitalStructureCashflows*`,
//! `forecastSpec*`, `adjustment*`, `normalize`).

mod builder;
mod exports;
mod handles;
mod results;
mod specs;
mod validation;

pub use builder::{JsMixedNodeBuilder, JsModelBuilder};
pub use exports::*;
pub use handles::{JsEvaluator, JsRegistry};
pub use validation::*;
pub(crate) use validation::{load_model, load_model_text};
