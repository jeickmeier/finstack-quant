//! Agency CMO risk metrics.
//!
//! CMO-specific metrics including tranche-level static Z-spread, duration,
//! and scenario analysis. A true Monte Carlo OAS (stochastic rates +
//! rate-dependent prepayment, as in `mbs_passthrough::metrics::mc_oas`) is
//! deferred; the spread metric exposed here is an honest static Z-spread.

mod register;
pub(crate) mod zspread;

pub(crate) use register::register_cmo_metrics;
pub(crate) use zspread::calculate_tranche_zspread;
