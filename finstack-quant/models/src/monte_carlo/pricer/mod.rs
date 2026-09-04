//! Higher-level pricing entry points built on top of [`crate::monte_carlo::engine::McEngine`].
//!
//! Start with [`european`] for a compact GBM-only API and [`heston`] for the
//! canonical Heston European entry points shared with the host bindings. The
//! `path_dependent` and `lsmc` modules expose richer workflows for
//! path-dependent contracts and early-exercise problems. GBM Asian and
//! American convenience entry points used by host bindings live on
//! [`path_dependent::PathDependentPricer`] and [`lsmc::LsmcPricer`].
//!
//! These pricers bundle process, discretization, and engine choices for common
//! use cases; the lower-level engine remains the more flexible option when you
//! need custom combinations.

pub mod basis;
pub mod european;
pub mod heston;
pub mod lsmc;
pub mod lsq;
pub mod path_dependent;
pub mod polynomial;

pub use european::EuropeanPricer;
