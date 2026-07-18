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
//! - [`recovery_waterfall`][crate::credit::recovery_waterfall]: absolute-priority
//!   allocation of a distributable estate across restructuring claims.
//! - [`liability_management`][crate::credit::liability_management]: hold-versus-tender
//!   economics for distressed exchanges and issuer liability management exercises.

/// Loss Given Default: seniority recovery distributions, workout LGD,
/// downturn adjustments, and EAD computation.
pub mod lgd;

pub mod liability_management;
pub mod migration;
pub mod pd;
pub mod recovery_waterfall;
pub mod registry;
pub mod scoring;
