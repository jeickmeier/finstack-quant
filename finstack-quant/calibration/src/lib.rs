//! Calibration framework — the canonical path to build a `MarketContext` from
//! raw market quotes.
//!
//! # Building a MarketContext from quotes
//!
//! The supported workflow is JSON-in / `MarketContext`-out:
//!
//! ```rust
//! use finstack_quant_calibration::api::{engine, schema::CalibrationEnvelope};
//! use finstack_quant_core::market_data::context::MarketContext;
//!
//! # let envelope_json = r#"{"schema":"finstack_quant.calibration/1","plan":{"id":"empty","description":null,"quote_sets":{},"steps":[],"settings":{}}}"#;
//! let envelope: CalibrationEnvelope =
//!     serde_json::from_str(envelope_json).expect("parse envelope");
//! let result = engine::execute(&envelope).expect("calibration succeeded");
//! let market = MarketContext::try_from(result.result.final_market)
//!     .expect("rehydrate market");
//! // `market` is now ready for valuations, attribution, scenarios, portfolio analysis.
//! # let _ = market;
//! ```
//!
//! Python and JavaScript users get the same surface: `finstack_quant.calibration.calibrate(json).market`
//! returns a `MarketContext`; the `CalibrationResult` wrapper additionally exposes per-step
//! residuals and a plan-level report next to the context, so users can verify their curves
//! actually fit.
//!
//! See `finstack-quant/calibration/examples/market_bootstrap/` for canonical envelope JSON examples
//! covering discount curves, hazard curves layered on snapshot inputs in `market_data`,
//! and FX matrices supplied as snapshot data.
//!
//! # Two-track envelope structure
//!
//! A `CalibrationEnvelope` carries quotes in two complementary places:
//!
//! - **Track A — bootstrapping (`plan.quote_sets` + `plan.steps`).** Quotes that drive a
//!   solver — rates, CDS, swaptions, vols, tranche spreads, etc. Each `step` reads its
//!   `quote_set` and produces a curve or surface added to the in-progress context.
//!   Step kinds: `discount`, `forward`, `hazard`, `inflation`, `vol_surface`,
//!   `swaption_vol`, `base_correlation`, `student_t`, `hull_white`, `cap_floor_hull_white`,
//!   `svi_surface`, `xccy_basis`, `parametric`.
//! - **Track B — snapshot data (`market_data` entries).** FX matrices, bond prices, equity
//!   spot prices, and dividend schedules are not bootstrapped today — they are supplied
//!   as materialized state via `fx_spot`, `price`, and `dividend_schedule` entries in
//!   `market_data` (with pre-built calibrated objects optionally supplied via
//!   `prior_market`).
//!
//! Both tracks are valid in the same envelope; the engine merges `market_data` and
//! `prior_market` into the working context before running steps.
//!
//! # When to use `MarketContext::try_from(MarketContextState)` directly
//!
//! `MarketContext::try_from(state)` (paired with `serde_json::from_str::<MarketContextState>`)
//! is the materialized-snapshot deserializer — it rehydrates a *previously-saved*
//! `MarketContext`. It does **not** build one from quotes. Use the calibration path
//! (above) for quote-driven construction; reserve direct deserialization for replaying
//! an already-calibrated context (e.g., from a saved snapshot, a downstream consumer,
//! or a regression-test fixture).
//!
//! # Documentation Rules For Calibration APIs
//!
//! Calibration docs should make three things explicit:
//!
//! - **Which quotes and conventions are assumed**: quote style, day count, curve
//!   time basis, interpolation, and market-standard construction choices should be
//!   stated near the public API that uses them.
//! - **Which tolerance is being discussed**: solver convergence tolerances and
//!   post-solve validation tolerances are distinct and should not be conflated.
//! - **Which canonical source applies**: model-heavy and convention-heavy APIs
//!   should include `# References` sections pointing to `docs/REFERENCES.md`.
//!
//! # Features
//! - **Plan-Driven API**: Uses `"finstack_quant.calibration/1"` schema for structured calibration plans.
//! - **Flexible Solvers**: Supports both sequential bootstrapping and global optimization (Newton/LM).
//! - **Market Standards**: Implements post-2008 multi-curve frameworks and strict pricing conventions.
//!
//! # See Also
//! - `api` for the plan schema and engine.
//! - `solver` for the underlying numerical solvers.
//! - [`crate::quotes`] for market data representation.
//!
//! # References
//!
//! - Multi-curve discounting and construction: `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - Curve interpolation: `docs/REFERENCES.md#hagan-west-monotone-convex`
//! - Core rates/derivatives background: `docs/REFERENCES.md#hull-options-futures`

#![forbid(unsafe_code)]
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

/// Plan-driven calibration API (schema + execution engine).
pub mod api;
/// Quote-to-instrument construction and date resolution.
pub(crate) mod build;
/// Hull-White one-factor model calibration to European swaptions.
pub mod hull_white;
/// Bermudan LMM loading-scale calibration.
pub mod lmm;
pub use lmm::{calibrate_bermudan_lmm_base_vol, calibrate_bermudan_lmm_base_vol_from_json};
/// Checked-in JSON Schema artifacts owned by this crate.
#[cfg(feature = "json-schema")]
pub mod json_schema;
/// Prepared quotes for calibration.
pub(crate) mod prepared;
/// Raw market quote data-transfer objects.
pub mod quotes;
/// Solver utilities and implementations used by calibration.
pub(crate) mod solver;
/// Calibration targets mapping API steps to domain execution.
pub(crate) mod targets;

// Shared infrastructure
mod config;
mod report;
pub(crate) mod step_runtime;
/// Curve and surface validators, preflight checks, and rate-bound policy.
pub mod validation;

/// Quote-space replay and batch-local recalibration provider.
///
/// Quote shocks use the valuations-owned `QuoteBump` contract. Direct curve
/// and surface shocks remain core `BumpSpec`/`Bumpable` operations.
pub mod recalibration;

/// Shared constants (tolerances, magic numbers).
pub(crate) mod constants;

#[cfg(test)]
mod hazard_curve_tests;
#[cfg(test)]
mod repricing_tests;
#[cfg(test)]
mod test_support;

// These types form the supported public API for calibration configuration.
// They are used by wasm/py bindings and external consumers.

/// Configuration types for calibration.
pub use config::{
    CalibrationConfig, CalibrationMethod, DiscountCurveSolveConfig, ForwardCurveSolveConfig,
    HazardCurveSolveConfig, InflationCurveSolveConfig, MarketFreshnessPolicy, MarketQuoteSide,
    RatesStepConventions, ResidualWeightingScheme, VolSurfaceSolveConfig,
};

/// Solver configuration (Brent/Newton).
pub use solver::SolverConfig;

/// Validation types for curves and surfaces.
pub use validation::{RateBounds, RateBoundsPolicy, ValidationConfig, ValidationMode};

/// Calibration diagnostics and results.
pub use report::{CalibrationDiagnostics, CalibrationReport, QuoteQuality};

// Internal/advanced re-exports (not part of typical usage)
#[doc(hidden)]
pub use config::CALIBRATION_CONFIG_KEY;

/// Calibration methodology identifiers recorded in audit reports.
pub mod versions;
