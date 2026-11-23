//! CDS Index metrics module.
//!
//! Provides metric calculators specific to `CdsIndex`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_cds_index_metrics`.
//!
//! Exposed metrics:
//! - Par spread (bp)
//! - Risky PV01
//! - CS01 (approximate)
//! - Bucketed CS01 (key-rate, per-tenor par-spread)
//! - Protection leg PV
//! - Premium leg PV
//! - Expected loss
//! - Jump to default

mod cs01;
mod dv01;
mod expected_loss;
mod jump_to_default;
mod recovery01;
mod register;
mod simple;

pub(crate) use register::register_cds_index_metrics;
