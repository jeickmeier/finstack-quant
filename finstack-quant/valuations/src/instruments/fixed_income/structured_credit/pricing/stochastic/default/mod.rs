//! Stochastic default models for structured credit.
//!
//! This module provides factor-driven default models that capture:
//! - Default correlation through copula models
//! - Intensity process dynamics (Cox process)
//! - Factor-correlated CDR
//! - Hazard curve-based defaults using market-calibrated curves
//!
//! # Models
//!
//! - **CopulaBasedDefault**: Default correlation via copula (Gaussian, Student-t)
//! - **IntensityProcessDefault**: Cox process with mean-reverting intensity
//! - **HazardCurveDefault**: Wraps HazardCurve from core for market-calibrated defaults
//!
//! # References
//!
//! - Li, D. X. (2000). "On Default Correlation: A Copula Function Approach."
//! - Duffie, D., & Singleton, K. J. (1999). "Modeling Term Structures of Defaultable Bonds."
//! - Schönbucher, P. J. (2003). "Credit Derivatives Pricing Models."

mod copula_based;
mod factor_correlated;
mod hazard_curve_adapter;
mod intensity_process;
mod per_name;
mod spec;
mod traits;

pub(crate) use copula_based::CopulaBasedDefault;
pub(crate) use factor_correlated::FactorCorrelatedDefault;
pub(crate) use hazard_curve_adapter::HazardCurveDefault;
pub(crate) use intensity_process::IntensityProcessDefault;
pub(crate) use per_name::PerNameCopulaDefault;
pub use per_name::PoolGranularity;
pub use spec::StochasticDefaultSpec;
pub(crate) use traits::{MacroCreditFactors, StochasticDefault};
