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
//! use finstack_valuations::covenants::{Covenant, CovenantSpec, CovenantType};
//! use finstack_valuations::metrics::MetricId;
//! use finstack_core::dates::Tenor;
//!
//! // Define a max leverage covenant (4.5x Debt/EBITDA) with quarterly testing
//! let covenant = Covenant::new(
//!     CovenantType::MaxDebtToEBITDA { threshold: 4.5 },
//!     Tenor::quarterly(),
//! );
//!
//! // Wrap in spec with a metric for evaluation
//! let spec = CovenantSpec::with_metric(covenant, MetricId::Dv01);
//! ```
//!
//! # Breach Forecasting
//!
//! Project potential future breaches under different scenarios:
//!
//! ```ignore
//! use finstack_valuations::covenants::{
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
//! - [`crate::covenants::CovenantEngine`] for covenant evaluation
//! - [`crate::covenants::CovenantSpec`] for covenant definition
//! - [`crate::covenants::ThresholdSchedule`] for time-varying thresholds
//! - `forecast_breaches_generic` for breach forecasting

pub(crate) mod engine;
pub(crate) mod forward;
/// Covenant report types and structures
pub(crate) mod mod_types;
/// Covenant threshold schedules and interpolation
pub(crate) mod schedule;
/// Covenant package templates for common deal structures
pub mod templates;

pub use engine::{
    BoundKind, ConsequenceApplication, Covenant, CovenantBreach, CovenantConsequence,
    CovenantEngine, CovenantScope, CovenantSpec, CovenantTestSpec, CovenantType, CovenantWaiver,
    CovenantWindow, EvaluationTrigger, InstrumentMutator, SpringingCondition, ThresholdTest,
};
pub use forward::{
    forecast_breaches_generic, forecast_covenant_generic,
    CovenantForecast as GenericCovenantForecast, CovenantForecastConfig, FutureBreach,
    ModelTimeSeries,
};
pub use mod_types::CovenantReport;
pub use schedule::{threshold_for_date, ThresholdSchedule};
