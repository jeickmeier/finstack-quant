//! Credit risk modeling primitives.
//!
//! - [`migration`][crate::credit::migration]: Credit migration modeling
//!   (JLT / CreditMetrics-style).
//! - [`lgd`][crate::credit::lgd]: Loss Given Default models (seniority,
//!   workout, downturn, EAD).
//! - [`scoring`][crate::credit::scoring]: academic credit scoring models
//!   (Altman, Ohlson, Zmijewski).
//! - [`pd`][crate::credit::pd]: PD calibration, term structures, and master
//!   scale mapping.

/// Loss Given Default: seniority recovery distributions, workout LGD,
/// downturn adjustments, and EAD computation.
pub mod lgd;

pub mod migration;
pub mod pd;
pub mod registry;
pub mod scoring;
