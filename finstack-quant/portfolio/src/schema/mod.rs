//! Schema registry for the portfolio crate.
//!
//! The registry lives in the library rather than the generator binary so the
//! generator, the contract tests and the language bindings all render from one
//! definition. Render an entry with
//! [`SchemaArtifact::generate`](finstack_quant_core::schema::SchemaArtifact::generate).

mod artifacts;
mod examples;
mod margin_examples;

use artifacts::example_portfolio;
pub use artifacts::{ARTIFACTS, package_materialization_schema};
