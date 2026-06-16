//! Cross-currency swap (XCCY swap) instrument.
//!
//! Provides a market-standard, explicit multi-currency leg model:
//! - Separate domestic/foreign legs with their own projection + discount curves
//! - Explicit calendars and business-day conventions (no implicit fallbacks by default)
//! - Explicit FX conversion requirements (PV is reported in a chosen reporting currency)
//!
//! This module is intended to be used both for standalone pricing and as the
//! underlying instrument for XCCY basis calibration.

/// XCCY swap metrics.
pub(crate) mod metrics;
pub(crate) mod pricing_mtm;
mod types;

pub use types::{LegSide, NotionalExchange, ResettingSide, XccySwap, XccySwapLeg};
