#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::new_without_default)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
// Allow expect() in doc tests (they are test code)
#![doc(test(attr(allow(clippy::expect_used))))]

//! Finstack Scenarios — Lightweight deterministic scenario capability.
//!
//! This crate provides a minimal, deterministic API for applying shocks to market data
//! and financial statement forecasts. It enables what-if analysis and stress testing
//! without requiring a full DSL parser.
//!
//! This is the cross-domain scenario surface. [`ScenarioSpec`] can mutate a
//! supplied [`ExecutionContext`] across market data, instruments, rate bindings,
//! and statement forecast nodes. A statement model is optional: market-only and
//! instrument-only callers can pass `None`, while statement operations return a
//! typed error if no model is supplied. Statement-local named scenario sets live
//! in `finstack-statements-analytics`; those evaluate scalar model overrides and
//! do not apply market or instrument shocks.
//!
//! # API Layers
//!
//! Most callers start with:
//! - [`ScenarioSpec`] and [`OperationSpec`] to describe shocks and time rolls
//! - [`ScenarioEngine`] to apply a spec deterministically
//! - [`ExecutionContext`] to supply market data, statements, instruments, and calendars
//! - [`templates`] for reusable historical stress scenarios
//!
//! # Quick Start
//!
//! ```ignore
//! use finstack_scenarios::{ScenarioSpec, OperationSpec, CurveKind, ScenarioEngine, ExecutionContext};
//! use finstack_core::market_data::context::MarketContext;
//! use finstack_statements::FinancialModelSpec;
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut market = MarketContext::new();
//! let mut model = FinancialModelSpec::new("test", vec![]);
//! let as_of = date!(2025-01-01);
//!
//! let scenario = ScenarioSpec {
//!     id: "stress_test".into(),
//!     name: Some("Q1 Stress Test".into()),
//!     description: None,
//!     operations: vec![
//!         OperationSpec::CurveParallelBp {
//!             curve_kind: CurveKind::Discount,
//!             curve_id: "USD_SOFR".into(),
//!             discount_curve_id: None,
//!             bp: 50.0,
//!         },
//!     ],
//!     priority: 0,
//!     resolution_mode: Default::default(),
//! };
//!
//! let engine = ScenarioEngine::default();
//! let mut ctx = ExecutionContext {
//!     market: &mut market,
//!     model: Some(&mut model),
//!     instruments: None,
//!     rate_bindings: None,
//!     calendar: None,
//!     as_of,
//! };
//!
//! let report = engine.apply(&scenario, &mut ctx)?;
//! println!("Applied {} operations", report.operations_applied);
//! # Ok(())
//! # }
//! ```

/// Adaptations for scenario execution across domains.
pub(crate) mod adapters;
/// Scenario execution engine and context.
pub mod engine;
/// Error types for scenario evaluation.
pub mod error;
/// Horizon total return analysis.
pub mod horizon;
/// Scenario specification types and enums.
pub mod spec;
/// Historical stress test template types and builders.
pub mod templates;
/// Utility helpers for scenario operations.
pub(crate) mod utils;
/// Structured warning enum surfaced via `ApplicationReport.warnings`.
pub mod warning;

pub use adapters::time_roll::apply_time_roll_forward;
pub use adapters::{ArbitrageViolation, RollForwardReport};
pub use engine::{ApplicationEnvelope, ApplicationReport, ExecutionContext, ScenarioEngine};
pub use error::{Error, Result};
pub use horizon::{HorizonAnalysis, HorizonResult};
pub use spec::{
    Compounding, CurveKind, HierarchyTarget, InstrumentType, NodeId, OperationSpec,
    RateBindingSpec, ScenarioSpec, TenorMatchMode, TimeRollMode, VolSurfaceKind,
};
pub use templates::{
    AssetClass, RegisteredTemplate, ScenarioSpecBuilder, Severity, TemplateMetadata,
    TemplateRegistry,
};
pub use utils::{
    calculate_interpolation_weights, parse_period_to_days, parse_tenor_to_years,
    InterpolationResult,
};
pub use warning::Warning;
