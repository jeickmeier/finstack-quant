//! Builder API for financial models using type-state pattern.

mod model_builder;

pub(crate) use model_builder::validate_node_id;
pub use model_builder::{MixedNodeBuilder, ModelBuilder, NeedPeriods, Ready};
