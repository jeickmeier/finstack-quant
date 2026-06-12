#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! # Finstack Statements Analytics
//!
//! Higher-level analysis, reporting, and extension implementations that build
//! on the core [`finstack_statements`] evaluation engine.
//!
//! This crate provides:
//!
//! - **Analysis** — sensitivity, scenario sets, variance, DCF, goal seek,
//!   covenants, backtesting, Monte Carlo, and introspection
//! - **Extensions** — concrete analytics extensions (corkscrew, credit
//!   scorecard) called directly via inherent methods
//! - **Templates** — real estate, roll-forward, and vintage model builders

/// Analysis tools for financial statement models.
pub mod analysis;

/// Concrete extension implementations (corkscrew, credit scorecard).
pub mod extensions;

/// Templates for common financial model structures.
pub mod templates;
