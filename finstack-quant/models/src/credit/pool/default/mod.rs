//! Stochastic default models for structured credit.
//!
//! This module provides factor-driven default models that capture:
//! - Default correlation through copula models
//! - Intensity process dynamics (Cox process)
//! - Factor-correlated CDR
//!
//! # Models
//!
//! - **CopulaBasedDefault**: Default correlation via copula (Gaussian, Student-t)
//! - **IntensityProcessDefault**: Cox process with mean-reverting intensity
//!
//! # References
//!
//! - Li, D. X. (2000). "On Default Correlation: A Copula Function Approach." `docs/REFERENCES.md#li-2000-gaussian-copula`
//! - Duffie, D., & Singleton, K. J. (1999). "Modeling Term Structures of Defaultable Bonds." `docs/REFERENCES.md#duffie-singleton-1999`
//! - Schönbucher, P. J. (2003). "Credit Derivatives Pricing Models."

mod copula_based;
mod factor_correlated;
mod intensity_process;
mod per_name;
mod spec;
mod traits;

pub(crate) use copula_based::CopulaBasedDefault;
pub(crate) use factor_correlated::FactorCorrelatedDefault;
pub(crate) use intensity_process::IntensityProcessDefault;
pub use per_name::PerNameCopulaDefault;
pub use per_name::PoolGranularity;
pub use spec::StochasticDefaultSpec;
pub use traits::StochasticDefault;
