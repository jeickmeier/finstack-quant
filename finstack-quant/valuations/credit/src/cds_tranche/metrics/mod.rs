//! CDS Tranche metrics module.
//!
//! Provides metric calculators specific to `CdsTranche`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_cds_tranche_metrics`.
//!
//! Exposed metrics:
//! - Spread DV01 (per 1bp change in running coupon)
//! - Expected loss (currency units)
//! - Jump-to-default (currency units)
//! - CS01 (parallel par-spread rebootstrap)
//! - Correlation01 (per 1% correlation change)
//! - Recovery01 (per 1% recovery change)

mod cs01;
mod dv01;
mod recovery01;
mod register;

pub(crate) use register::register_cds_tranche_metrics;
