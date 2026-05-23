//! Re-export shim for the relocated correlation error type.
//!
//! `finstack_valuations::correlation::error::Error` previously lived here.
//! It now lives in [`finstack_analytics::correlation::error`] so that
//! downstream crates (e.g. `finstack-factor-model`) can consume it without
//! depending on `finstack-valuations`. This module preserves the old paths
//! via re-export.

pub use finstack_analytics::correlation::error::{Error, Result};
