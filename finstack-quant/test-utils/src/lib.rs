//! Shared test utilities for finstack-quant workspace crates.
//!
//! This crate keeps golden-test loading and comparison helpers out of
//! `finstack-quant-core`'s production library surface while preserving a common
//! framework for workspace test suites.

#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![doc(test(attr(allow(clippy::expect_used))))]

pub mod golden;

/// Error type for shared test utility failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Input data, fixture shape, or assertion validation failed.
    #[error("{0}")]
    Validation(String),
}
