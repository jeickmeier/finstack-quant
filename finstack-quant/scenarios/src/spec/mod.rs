//! Serializable scenario specifications (serde-stable).
//!
//! This module defines the data contract that callers use to describe market,
//! instrument, statement, and time-roll shocks. Types here are intentionally
//! lightweight and focus on configuration rather than behaviour so they can be
//! shared across binaries, configuration files, and JSON/YAML APIs.
//!
//! Start with [`ScenarioSpec`] and [`OperationSpec`]. Use [`RateBindingSpec`]
//! when statement nodes should follow curve-driven rates, and use
//! [`HierarchyTarget`](finstack_quant_core::market_data::hierarchy::HierarchyTarget) together with [`ResolutionMode`](finstack_quant_core::market_data::hierarchy::ResolutionMode)
//! for hierarchy-expanded shocks.
//!
//! # Conventions
//!
//! - Percent shocks use percentage-point inputs (`5.0 = +5%`).
//! - Curve and spread shocks quoted in `bp` use additive basis points
//!   (1 bp = 1e-4). Commodity shocks quoted in `pct` scale forward prices.
//! - Vol-index `Pts` are index points (`1.0` on 18.5 → 19.5).
//! - Correlation / base-corr `Pts` are decimal correlation (`0.02` = +0.02).
//! - Time-roll operations distinguish business-day-aware, calendar-day, and
//!   approximate day-count semantics via [`TimeRollMode`].
//! - Hierarchy-targeted operations expand to direct market-data identifiers at
//!   execution time; [`ScenarioSpec::resolution_mode`] determines whether
//!   overlapping hierarchy matches accumulate or collapse to the most specific
//!   match.

pub(crate) mod operations;
pub(crate) mod scenario;
#[cfg(test)]
mod tests;
pub(crate) mod types;
pub(crate) mod validation;

pub(crate) use operations::OperationSpec;
pub(crate) use scenario::ScenarioSpec;
#[cfg(test)]
pub(crate) use types::TimeRollMode;
pub(crate) use types::{CurveKind, RateBindingSpec, TenorMatchMode};
