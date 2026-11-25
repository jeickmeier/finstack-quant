//! Inflation cap/floor metrics module.
//!
//! Provides metric calculators specific to `InflationCapFloor`.
//! Metrics are registered via `register_inflation_cap_floor_metrics`.
//!
//! # Available Metrics
//!
//! - **Inflation01**: Sensitivity to 1bp parallel shift in inflation curve
//! - **Gamma**: Second derivative (convexity) with respect to inflation rate
//! - **Vega**: Sensitivity to 1% absolute shift in volatility surface
//! - **Dv01**: Sensitivity to 1bp shift in discount curve
//! - **BucketedDv01**: Key-rate sensitivities to discount curve
//! - **Theta**: Time decay (1-day roll)

mod register;

pub(crate) use register::register_inflation_cap_floor_metrics;
