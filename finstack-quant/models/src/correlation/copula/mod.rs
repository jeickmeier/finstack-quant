//! Copula models for portfolio default correlation.
//!
//! Provides a trait-based copula abstraction enabling pluggable correlation
//! models with the one-factor Gaussian copula as the default.
//!
//! # Supported Models
//!
//! - **Gaussian**: Standard one-factor Gaussian copula (market default)
//! - **Student-t**: Fat-tailed copula capturing tail dependence
//! - **Random Factor Loading (RFL)**: Stochastic correlation model
//! - **Multi-Factor**: Sector-based correlation structure
//!
//! # References
//!
//! - Gaussian copula background: `docs/REFERENCES.md#li-2000-gaussian-copula`
//!
//! - Random recovery and random-factor-loading extensions: `docs/REFERENCES.md#andersen-sidenius-2005-rfl`
//!
//! - Analytical CDO valuation context: `docs/REFERENCES.md#hull-white-2004-cdo`
//!

pub(crate) mod gaussian;
pub(crate) mod multi_factor;
pub(crate) mod random_factor_loading;
pub(crate) mod spec;
pub(crate) mod student_t;
#[cfg(test)]
mod tests;

pub(crate) use gaussian::GaussianCopula;
pub(crate) use multi_factor::MultiFactorCopula;
pub(crate) use random_factor_loading::RandomFactorLoadingCopula;
#[cfg(test)]
pub(super) use spec::CopulaSpec;
pub(crate) use spec::DEFAULT_QUADRATURE_ORDER;
pub(crate) use spec::{Copula, FactorPairIntegrand};
use spec::{default_quadrature, select_quadrature, validate_mixing_uniform};
pub(crate) use student_t::StudentTCopula;
