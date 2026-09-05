#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! Finstack Quant Scenarios — Lightweight deterministic scenario capability.
//!
//! Apply shocks to market data and financial statement forecasts. This is the
//! cross-domain scenario surface: [`ScenarioSpec`] mutates a supplied
//! [`ExecutionContext`] across market data, instruments, rate bindings, and
//! statement forecast nodes. A statement model is optional: market-only and
//! instrument-only callers can pass `None`, while statement operations return a
//! typed error if no model is supplied. Statement-local named scenario sets live
//! in `finstack-quant-statements-analytics`; those evaluate scalar model overrides and
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
//! ```
//! use finstack_quant_scenarios::{ScenarioSpec, OperationSpec, CurveKind, ScenarioEngine, ExecutionContext};
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_statements::FinancialModelSpec;
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let as_of = date!(2025-01-01);
//!
//! // The scenario bumps USD_SOFR, so that curve has to be in the context.
//! let mut market = MarketContext::new().insert(
//!     DiscountCurve::builder("USD_SOFR")
//!         .base_date(as_of)
//!         .knots([(0.0, 1.0), (5.0, 0.80)])
//!         .build()?,
//! );
//! let mut model = FinancialModelSpec::new("test", vec![]);
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
//!     hazard_bump_mode: Default::default(),
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
//! println!("Applied {} effects", report.operations_applied);
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - Day-count and business-day conventions: `docs/REFERENCES.md#isda-2006-definitions`
//! - Period notation: `docs/REFERENCES.md#iso-8601`

/// Adaptations for scenario execution across domains.
pub(crate) mod adapters;
/// Scenario execution engine and context.
pub mod engine;
/// Versioned persistence envelope for scenario specifications.
pub mod envelope;
/// Error types for scenario evaluation.
pub mod error;
/// Horizon total return analysis.
pub mod horizon;
/// JSON Schema generation helpers for scenario contracts.
#[cfg(feature = "json-schema")]
pub mod schema;
/// Scenario specification types and enums.
pub mod spec;
/// Historical stress test template types and builders.
pub mod templates;
/// Utility helpers for scenario operations.
pub(crate) mod utils;
/// Structured warning enum surfaced via `ApplicationReport.warnings`.
pub mod warning;

pub use adapters::time_roll::apply_time_roll_forward;
pub use adapters::vol::ArbitrageViolation;
pub use engine::{
    ApplicationEnvelope, ApplicationReport, ExecutionContext, RollForwardReport, ScenarioEngine,
};
pub use envelope::ScenarioEnvelope;
pub use error::{Error, Result};
pub use horizon::{HorizonAnalysis, HorizonResult};
pub use spec::{
    Compounding, CurveKind, HazardBumpMode, HierarchyTarget, InstrumentType, NodeId, OperationSpec,
    RateBindingSpec, ScenarioSpec, TenorMatchMode, TimeRollMode,
};
pub use templates::{AssetClass, Severity, TemplateMetadata, TemplateRegistry};
pub use warning::Warning;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
