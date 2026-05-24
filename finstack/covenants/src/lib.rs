//! Covenant evaluation, testing, and breach forecasting.
//!
//! This module provides infrastructure for defining, testing, and forecasting
//! financial covenants commonly found in credit agreements, loan documents,
//! and structured product indentures.
//!
//! # Features
//!
//! - **Covenant Engine**: Rule-based evaluation of covenant compliance
//! - **Threshold Schedules**: Time-varying covenant levels
//! - **Breach Detection**: Identify current covenant violations
//! - **Forward Forecasting**: Project future breaches under scenarios
//! - **Consequence Modeling**: Trigger actions on breach (springing liens, etc.)
//!
//! # Covenant Types
//!
//! Common financial covenants supported:
//! - **Leverage Ratios**: Debt/EBITDA, Net Debt/EBITDA
//! - **Coverage Ratios**: Interest coverage, fixed charge coverage
//! - **Liquidity Tests**: Minimum cash, current ratio
//! - **Capital Covenants**: Maximum capex, minimum equity
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_covenants::{Covenant, CovenantMetricId, CovenantSpec, CovenantType};
//! use finstack_core::dates::Tenor;
//!
//! // Define a max leverage covenant (4.5x Debt/EBITDA) with quarterly testing
//! let covenant = Covenant::new(
//!     CovenantType::MaxDebtToEBITDA { threshold: 4.5 },
//!     Tenor::quarterly(),
//! );
//!
//! // Wrap in spec with a metric for evaluation
//! let spec = CovenantSpec::with_metric(covenant, CovenantMetricId::from("debt_to_ebitda"));
//! ```
//!
//! # Breach Forecasting
//!
//! Project potential future breaches under different scenarios:
//!
//! ```ignore
//! use finstack_covenants::{
//!     forecast_breaches_generic, CovenantForecastConfig
//! };
//!
//! // Configure forecasting
//! let config = CovenantForecastConfig::default();
//!
//! // Forecast breaches over forecast horizon
//! // let breaches = forecast_breaches_generic(&instrument, &covenants, &scenarios, config);
//! ```
//!
//! # See Also
//!
//! - [`CovenantEngine`] for covenant evaluation
//! - [`CovenantSpec`] for covenant definition
//! - [`ThresholdSchedule`] for time-varying thresholds
//! - `forecast_breaches_generic` for breach forecasting

pub(crate) mod engine;
pub(crate) mod forward;
pub mod json;
pub mod metric;
/// Covenant report types and structures
pub(crate) mod report;
/// Covenant threshold schedules and interpolation
pub(crate) mod schedule;
/// Covenant package templates for common deal structures
pub mod templates;

pub use engine::{
    BoundKind, ConsequenceApplication, Covenant, CovenantBreach, CovenantConsequence,
    CovenantEngine, CovenantEvalCtx, CovenantScope, CovenantSpec, CovenantTestSpec, CovenantType,
    CovenantWaiver, CovenantWindow, EvaluationTrigger, InstrumentMutator, SpringingCondition,
    ThresholdTest,
};
pub use forward::{
    forecast_breaches_generic, forecast_covenant_generic, CovenantForecast, CovenantForecastConfig,
    FutureBreach, ModelTimeSeries,
};
pub use json::{
    cov_lite_json, evaluate_engine_json, lbo_standard_json, project_finance_json, real_estate_json,
    validate_covenant_engine_json, validate_covenant_report_json, validate_covenant_spec_json,
};
pub use metric::{CovenantMetricId, CovenantMetricSource, HashMapMetricSource};
pub use report::CovenantReport;
pub use schedule::{threshold_for_date, ThresholdSchedule};
