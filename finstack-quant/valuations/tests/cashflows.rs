//! Cashflows integration test entrypoint.
//!
//! Run with:
//! `cargo nextest run -p finstack-quant-valuations --test valuations cashflows::`

#[path = "cashflows/mod.rs"]
#[allow(clippy::module_inception)]
mod cashflows;
