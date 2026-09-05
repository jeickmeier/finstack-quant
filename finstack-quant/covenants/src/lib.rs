//! Covenant evaluation, breach tracking, consequence application, and forward
//! compliance forecasting.
//!
//! [`CovenantEngine`] evaluates [`CovenantSpec`] values from a
//! [`metric::CovenantMetricSource`]. [`ThresholdSchedule`] supports step-down limits,
//! [`forecast_covenant_generic`] projects future compliance, and [`templates`]
//! and [`json`] provide standard packages and the serde-first binding surface.
//!
//! # Conventions
//!
//! - Callers choose each test date; [`Covenant::test_frequency`] is metadata and
//!   is not enforced by the engine.
//! - Metric sources provide calculation-ready values; the engine does not
//!   aggregate LTM or other windows. Ratios use turns (`4.5` means 4.5x).
//!   Amount metrics and thresholds must share the caller's reporting currency.
//! - Equity cures are not modeled. Use [`CovenantWaiver`] for a full waiver or
//!   amended threshold.
//!
//! [`CovenantEngine::evaluate`] returns an error for invalid engine
//! configuration, duplicate applicable covenant instance keys, missing metric
//! values, or a covenant that cannot produce a test value.
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_core::dates::{create_date, Month, Tenor};
//! use finstack_quant_covenants::{
//!     Covenant, CovenantEngine, CovenantSpec, CovenantType, HashMapMetricSource,
//! };
//!
//! # fn main() -> finstack_quant_core::Result<()> {
//! let covenant = Covenant::new(
//!     CovenantType::MaxDebtToEbitda { threshold: 4.5 },
//!     Tenor::quarterly(),
//!     "max_total_leverage", // instance label — also the report key
//! );
//! let mut engine = CovenantEngine::new();
//! engine.add_spec(CovenantSpec::with_metric(covenant, "debt_to_ebitda"));
//!
//! let metrics = HashMapMetricSource::from_pairs([("debt_to_ebitda", 3.2)]);
//! let test_date = create_date(2025, Month::March, 31)?;
//! let reports = engine.evaluate(&metrics, test_date)?;
//!
//! assert!(reports["max_total_leverage"].passed);
//! assert_eq!(reports["max_total_leverage"].threshold, Some(4.5));
//! # Ok(())
//! # }
//! ```

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

pub(crate) mod engine;
pub(crate) mod forward;
pub mod json;
pub mod metric;
pub(crate) mod report;
pub(crate) mod schedule;
pub mod templates;

pub use engine::{
    BoundKind, ConsequenceApplication, Covenant, CovenantBreach, CovenantConsequence,
    CovenantEngine, CovenantScope, CovenantSpec, CovenantType, CovenantWaiver, CovenantWindow,
    InstrumentMutator, SpringingCondition, ThresholdTest,
};
pub use forward::{
    forecast_breaches_generic, forecast_covenant_generic, CovenantForecast, CovenantForecastConfig,
    FutureBreach, ModelTimeSeries,
};
pub use json::{
    cov_lite_json, evaluate_engine_map, lbo_standard_json, project_finance_json, real_estate_json,
    validate_covenant_engine_json, validate_covenant_report_json, validate_covenant_spec_json,
};
pub use metric::{CovenantMetricId, HashMapMetricSource};
pub use report::CovenantReport;
pub use schedule::ThresholdSchedule;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
