//! Metric registry and computation engine.
//!
//! Manages metric calculators with dependency resolution, caching, and batch
//! computation. The registry handles which metrics apply to which instrument
//! types and ensures efficient computation ordering.

pub(crate) mod engine;
#[cfg(test)]
mod tests;

pub(crate) use engine::validate_measure;
pub(crate) use engine::{MetricAggregation, MetricRegistry, MetricRegistryError};
