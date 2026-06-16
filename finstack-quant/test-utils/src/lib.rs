//! Shared test utilities for finstack-quant workspace crates.
//!
//! This crate keeps golden-test loading and comparison helpers out of
//! `finstack-quant-core`'s production library surface while preserving a common
//! framework for workspace test suites.

#![forbid(unsafe_code)]

use std::fmt;

pub mod golden;

/// Error type for shared test utility failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Input data, fixture shape, or assertion validation failed.
    Validation(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Validation(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}
