//! Interest rate option metrics module.
//!
//! Provides metric calculators specific to `CapFloor`. The calculators compose
//! with the shared metrics framework and are registered via
//! `register_cap_floor_metrics`.
//!
//! Exposed metrics:
//! - Delta
//! - Gamma
//! - Vega (market normal-vol quote sensitivity)
//! - HwSigmaVega (direct Hull-White short-rate sigma sensitivity)
//! - Theta
//! - Rho
//! - ImpliedVol (placeholder)

mod common;
mod dv01;
mod implied_vol;
mod register;
mod theta;
mod vega;

pub(crate) use register::register_cap_floor_metrics;
