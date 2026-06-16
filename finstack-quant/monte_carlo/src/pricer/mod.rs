//! Higher-level pricing entry points built on top of [`crate::engine::McEngine`].
//!
//! Start with [`european`] for a compact GBM-only API. The `path_dependent` and
//! `lsmc` modules expose richer workflows for path-dependent contracts and
//! early-exercise problems.
//!
//! These pricers bundle process, discretization, and engine choices for common
//! use cases; the lower-level engine remains the more flexible option when you
//! need custom combinations.

pub mod basis;
pub mod european;
pub mod lsmc;
pub mod lsq;
pub mod path_dependent;

pub use european::*;
