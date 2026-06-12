//! Barrier-monitoring helpers for Monte Carlo pricing.
//!
//! These modules are used by barrier payoffs and other first-passage-style
//! diagnostics. `bridge` models continuous barrier hits between monitoring
//! dates, while `corrections` provides continuity adjustments that reduce the
//! bias of discretely monitored simulations.

pub mod bridge;
pub mod corrections;
