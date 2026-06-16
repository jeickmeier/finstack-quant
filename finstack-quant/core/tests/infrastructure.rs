//! Infrastructure module integration tests.
//!
//! This test suite verifies:
//! - Configuration extensions (FinstackConfig, ToleranceConfig)
//! - Explainability infrastructure (ExplainOpts, ExplanationTrace)
//! - ResultsMeta stamping and configuration
//!
//! # Test Organization
//!
//! - [`config`]: Configuration extensions (FinstackConfig, ToleranceConfig)
//! - [`explain`]: Explainability infrastructure (ExplainOpts, ExplanationTrace)
//! - [`metadata`]: ResultsMeta stamping and configuration

#[path = "infrastructure/config.rs"]
mod config;

#[path = "infrastructure/explain.rs"]
mod explain;

#[path = "infrastructure/metadata.rs"]
mod metadata;
