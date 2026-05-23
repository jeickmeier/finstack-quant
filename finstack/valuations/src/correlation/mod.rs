//! Shared correlation infrastructure for credit modeling.
//!
//! This module provides reusable correlation models used across credit instruments:
//! - CDS tranche pricing
//! - Structured credit (ABS/CLO/CMBS/RMBS)
//! - Portfolio credit risk
//!
//! # Components
//!
//! - [`crate::correlation::copula`]: Copula models for default correlation (Gaussian, Student-t, RFL, Multi-factor)
//! - [`crate::correlation::recovery`]: Recovery rate models (constant, market-correlated)
//! - [`crate::correlation::factor_model`]: Factor models for correlated behavior
//!
//! Joint probability utilities ([`crate::correlation::CorrelatedBernoulli`],
//! [`crate::correlation::correlation_bounds`],
//! [`crate::correlation::joint_probabilities`]) are re-exported from
//! [`finstack_core::math::probability`].
//!
//! # Utilities
//!
//! - [`crate::correlation::validate_correlation_matrix`]: Validate correlation matrices
//! - [`crate::correlation::cholesky_decompose`]: Cholesky decomposition for correlated factor generation
//! - [`crate::correlation::correlation_bounds`]: Fréchet-Hoeffding bounds for correlated Bernoulli
//!
//! # Conventions
//!
//! - Probabilities, correlations, and recovery rates are quoted in decimals.
//! - Flattened correlation matrices use row-major ordering.
//! - Latent-factor inputs are standard-normal or Student-t realizations, depending
//!   on the concrete model.
//!
//! # References
//!
//! - Gaussian copula background:
//!   `docs/REFERENCES.md#li-2000-gaussian-copula`
//! - Student-t copula background:
//!   `docs/REFERENCES.md#demarta-mcneil-2005-t-copula`
//! - Factor-model and portfolio-risk context:
//!   `docs/REFERENCES.md#meucci-risk-and-asset-allocation`

pub mod copula;
pub mod error;
pub mod factor_model;
pub mod nearest_correlation;
pub mod recovery;

// Re-export commonly used types
pub use copula::{
    Copula, CopulaSpec, GaussianCopula, MultiFactorCopula, RandomFactorLoadingCopula,
    StudentTCopula,
};
pub use error::{Error, Result};
pub use factor_model::{
    cholesky_decompose, FactorModelKind, FactorSpec, MultiFactorModel, SingleFactorModel,
    TwoFactorModel,
};
pub use finstack_analytics::correlation::validate_correlation_matrix;
pub use finstack_core::math::probability::{
    correlation_bounds, joint_probabilities, CorrelatedBernoulli,
};
pub use nearest_correlation::{nearest_correlation_matrix, NearestCorrelationOpts};
pub use recovery::{ConstantRecovery, CorrelatedRecovery, RecoveryModel, RecoverySpec};
