//! CDS Option metrics module.
//!
//! Provides metric calculators specific to `CdsOption`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_cds_option_metrics`.
//!
//! Exposed metrics:
//! - Delta, Gamma, Vega, Theta
//! - CS01 (quoted CDS spread sensitivity)
//! - DV01 (swap-curve quote sensitivity; canonical IR sensitivity)
//! - ParSpread (Bloomberg CDSO ATM forward spread in bp)
//! - Implied Volatility

pub(crate) mod delta;
mod dv01;
pub(crate) mod gamma;
mod recovery01;
pub(crate) mod register;

pub(crate) use register::register_cds_option_metrics;
