//! Commodity Asian option metrics module.
//!
//! Provides risk sensitivities for commodity Asian options:
//! - **Delta**: Forward curve sensitivity (bump-and-reprice on PriceCurve)
//! - **Vega**: Volatility sensitivity (bump-and-reprice on vol surface)
//! - **DV01**: Interest rate sensitivity (discount curve bump)
//! - **BucketedDv01**: Key-rate DV01

mod greeks;
mod register;

pub(crate) use register::register_commodity_asian_option_metrics;
