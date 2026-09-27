//! Short-rate tree models for bond valuation with embedded options.
//!
//! Implements curve-consistent short-rate trees for pricing callable/putable bonds
//! and calculating Option-Adjusted Spread (OAS). Uses industry-standard models
//! like Ho-Lee and Black-Derman-Toy.
//!
//! # Volatility Conventions
//!
//! **Critical**: The volatility parameter interpretation depends on the model type:
//!
//! | Model | Vol Type | Parameter | Formula | Typical Range |
//! |-------|----------|-----------|---------|---------------|
//! | Ho-Lee | Normal/Absolute | σ (bp/yr) | dr = θdt + σdW | 50-150 bp (0.005-0.015) |
//! | BDT | Lognormal/Relative | σ (%) | dr/r = θdt + σdW | 15-30% (0.15-0.30) |
//!
//! ## Converting Between Conventions
//!
//! Use [`convert_atm_volatility`](crate::volatility::convert_atm_volatility) to convert:
//!
//! ```
//! use finstack_quant_models::volatility::{convert_atm_volatility, VolatilityConvention};
//!
//! let normal_vol = 0.01;
//! let rate_level = 0.05;
//!
//! let lognormal_vol = convert_atm_volatility(
//!     normal_vol,
//!     VolatilityConvention::Normal,
//!     VolatilityConvention::Lognormal,
//!     rate_level,
//!     1.0,
//! )?;
//! assert!(lognormal_vol > 0.15 && lognormal_vol < 0.25);
//!
//! let back_to_normal = convert_atm_volatility(
//!     lognormal_vol,
//!     VolatilityConvention::Lognormal,
//!     VolatilityConvention::Normal,
//!     rate_level,
//!     1.0,
//! )?;
//! assert!((back_to_normal - normal_vol).abs() < 1e-10);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! ## Calibration Sources
//!
//! - **Swaption market**: ATM swaption vols are typically quoted in normal (bp)
//! - **Cap/floor market**: Often quoted in lognormal (Black vol)
//! - **Historical**: Calculate from rate time series

mod bdt;
mod black_karasinski;
mod config;
mod ho_lee;
/// State variable keys specific to short-rate trees.
pub mod short_rate_keys;
mod tree;
mod tree_model;

pub use config::TreeDiscounting;
pub use config::{
    ShortRateModel, ShortRateTreeConfig, DEFAULT_CURVE_FIT_TOLERANCE_BP, DEFAULT_NORMAL_VOL,
};
pub use tree::{ShortRateTree, TreeCalibrationResult};

#[cfg(test)]
mod tests;
