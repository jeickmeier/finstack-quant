//! Metric registry for financial statement definitions.
//!
//! The registry allows you to define reusable financial metrics that can be
//! shared across models. Metrics are organized into namespaces (e.g., `fin`)
//! and referenced using qualified identifiers (e.g., `fin.gross_profit`).
//!
//! # Namespace Behavior
//!
//! ## Qualified References
//!
//! Metrics are stored with fully-qualified IDs like `"fin.gross_profit"`.
//! In formulas, reference them using the qualified form:
//!
//! ```ignore
//! use finstack_statements::prelude::*;
//! use finstack_statements::builder::Ready;
//!
//! # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! # let mut model: ModelBuilder<Ready> = unimplemented!("build a ModelBuilder<Ready>");
//! let registry = Registry::with_builtins()?;
//! model.add_metric_from_registry("fin.gross_profit", &registry)?;
//! model.compute("margin", "fin.gross_profit / revenue")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Namespace Resolution
//!
//! When a metric formula references another identifier:
//! 1. Check if it exists as a qualified metric ID (e.g., `fin.ebitda`)
//! 2. Check if it exists as a node ID in the model
//! 3. If neither, report an unknown identifier error
//!
//! ## Shadowing Warning
//!
//! **User-defined nodes can shadow registry metrics if they use the same qualified ID.**
//!
//! For example:
//! - Registry defines `fin.gross_profit`
//! - User creates a node with `node_id = "fin.gross_profit"`
//! - The user node will shadow the registry metric
//!
//! ### Best Practices
//!
//! 1. **Use custom namespaces** for your metrics (e.g., `"custom.my_metric"`)
//! 2. **Don't create nodes** with IDs that match `namespace.metric_id` patterns
//! 3. **Prefix user nodes** with clear identifiers (e.g., `"model_revenue"` not `"revenue"`)
//! 4. **Document dependencies** when mixing registry and custom metrics
//!
//! # Example: Safe Custom Metric Definition
//!
//! ```ignore
//! use finstack_statements::prelude::*;
//! use finstack_statements::builder::Ready;
//!
//! # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! # let mut model: ModelBuilder<Ready> = unimplemented!("build a ModelBuilder<Ready>");
//! // Load standard metrics under "fin" namespace
//! model.with_builtin_metrics()?;
//!
//! // Define custom metrics under "custom" namespace (no collision)
//! # let values: &[(PeriodId, AmountOrScalar)] = &[];
//! model.value("revenue", values);
//! model.compute("custom.my_margin", "fin.gross_profit / revenue")?;
//! //                                  ^^^^^^^^^^^^^    ^^^^^^^
//! //                                  registry metric  user node
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`crate::registry::Registry`] - Main registry type
//! - [`ModelBuilder::with_builtin_metrics`](crate::builder::ModelBuilder::with_builtin_metrics)
//! - Metric files in `finstack/statements/data/metrics/`

pub(crate) mod aliases;
pub(crate) mod builtins;
pub mod dynamic;
pub(crate) mod schema;
pub(crate) mod validation;

pub(crate) use aliases::AliasRegistry;
pub use dynamic::Registry;
pub use schema::{MetricDefinition, MetricRegistry, UnitType};
