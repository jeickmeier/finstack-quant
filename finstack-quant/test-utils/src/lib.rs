//! Shared test utilities for finstack-quant workspace crates.
//!
//! This crate keeps golden-test loading and comparison helpers out of
//! `finstack-quant-core`'s production library surface while preserving a common
//! framework for workspace test suites.

#![forbid(unsafe_code)]

pub mod golden;
