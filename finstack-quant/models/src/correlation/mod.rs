//! Shared correlation infrastructure for credit modeling.
//!
//! This module provides reusable correlation models used across credit instruments:
//! - CDS tranche pricing
//! - Structured credit (ABS/CLO/CMBS/RMBS)
//! - Portfolio credit risk
//!
//! # Components
//!
//! - [`Copula`] / [`CopulaSpec`]: Copula models for default correlation (Gaussian, Student-t, RFL, Multi-factor)
//! - [`RecoveryModel`] / [`RecoverySpec`]: Recovery rate models (constant, market-correlated)
//! - [`LatentFactorSpec`]: Factor models for correlated behavior
//! - [`simulate_portfolio_loss`]: Monte Carlo portfolio loss distribution
//!
//! Joint probability utilities (`CorrelatedBernoulli`, `correlation_bounds`,
//! `joint_probabilities`) live in [`finstack_quant_core::math::probability`];
//! matrix helpers (`validate_correlation_matrix`, `nearest_correlation`,
//! `cholesky_correlation`) live in [`finstack_quant_core::math::linalg`].
//! [`Error`] / [`Result`] are models-owned: they wrap core matrix failures and
//! add credit-domain variants (volatilities, recovery, Student-t df).
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
//! - Gaussian copula background: `docs/REFERENCES.md#li-2000-gaussian-copula`
//!
//! - Student-t copula background: `docs/REFERENCES.md#demarta-mcneil-2005-t-copula` `docs/REFERENCES.md#mcneil-frey-embrechts-qrm`
//!
//! - Factor-model and portfolio-risk context: `docs/REFERENCES.md#meucci-risk-and-asset-allocation`
//!

mod copula;
mod error;
mod latent_factor;
mod portfolio_loss;
mod recovery;

pub use copula::{
    Copula, CopulaSpec, FactorPairIntegrand, GaussianCopula, MultiFactorCopula,
    RandomFactorLoadingCopula, StudentTCopula,
};
pub use error::{Error, Result};
pub use latent_factor::{
    LatentFactorKind, LatentFactorSpec, LatentMultiFactor, LatentSingleFactor, LatentTwoFactor,
};
pub use portfolio_loss::{
    simulate_portfolio_loss, simulate_portfolio_loss_with_recovery, CreditExposure,
    PortfolioLossConfig, PortfolioLossResult, TrancheLossStatistics, MAX_PORTFOLIO_LOSS_PATHS,
};
pub use recovery::{ConstantRecovery, CorrelatedRecovery, RecoveryModel, RecoverySpec};
