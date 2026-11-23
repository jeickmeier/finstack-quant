//! Swaption metrics module.
//!
//! Contains per-metric calculators split into separate files for clarity and
//! maintainability. The module exposes a registration helper to wire metrics
//! into the shared `MetricRegistry`.

pub(crate) mod bermudan_greeks;
pub(crate) mod config;
mod delta;
mod gamma;
mod implied_vol;
mod register;
mod vega;

pub(crate) use bermudan_greeks::{
    BermudanDeltaCalculator, BermudanGammaCalculator, BermudanVegaCalculator,
};
pub(crate) use delta::DeltaCalculator;
pub(crate) use gamma::GammaCalculator;
pub(crate) use implied_vol::ImpliedVolCalculator;
pub(crate) use register::{register_bermudan_swaption_metrics, register_swaption_metrics};
pub(crate) use vega::VegaCalculator;
