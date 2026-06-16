//! Core metrics framework infrastructure.
//!
//! This module provides the foundational types for the metrics system:
//! - `MetricId`: Strongly-typed metric identifiers
//! - `MetricCalculator`: Trait for implementing metric calculations
//! - `MetricContext`: Context containing instrument and market data
//! - `MetricRegistry`: Registry managing calculators with dependency resolution and strict,
//!   fail-fast error handling
//! - Finite difference utilities for numerical derivatives

pub(crate) mod finite_difference;
pub(crate) mod ids;
pub(crate) mod registration_macro;
pub(crate) mod registry;
pub(crate) mod standard_registry;
pub(crate) mod traits;
