//! Stochastic recovery models for credit portfolio pricing.
//!
//! This module provides recovery-rate models used alongside latent-factor
//! default models. Recovery is expressed in decimals, so `0.40` means a 40%
//! recovery rate and `0.60` LGD.
//!
//! # Constant vs Stochastic Recovery
//!
//! - **Constant**: Simplest model, uses fixed recovery (e.g., 40%)
//! - **Stochastic**: Recovery varies with market conditions
//!
//! # Stochastic Recovery Models
//!
//! ## Market-correlated logistic-normal recovery
//!
//! ```text
//! R(z,e) = sigmoid(logit(m) + sigma/[m(1-m)] * (rho*z + sqrt(1-rho²)*e))
//! ```
//!
//! `m` is the zero-total-shock recovery location, not the unconditional mean.
//! `sigma` is a local shock scale, not the realized recovery standard deviation.
//! Both Gaussian factors are independent standard normals. Conditional recovery
//! integrates over `e`; expected recovery integrates over both factors. Locations
//! zero and one and a zero shock scale give exact constant recoveries. Low `z`
//! is stress, so positive `rho` lowers recovery during stress.
//!
//! # References
//!
//! - Default/recovery empirical evidence: `docs/REFERENCES.md#altman-et-al-2005-recovery`
//!
//! - Stochastic recovery model context: `docs/REFERENCES.md#andersen-sidenius-2005-rfl`
//!

pub(crate) mod constant;
pub(crate) mod correlated;
pub(crate) mod spec;
#[cfg(test)]
mod tests;

use super::error::Result;
pub(crate) use constant::ConstantRecovery;
pub(crate) use correlated::CorrelatedRecovery;
pub(crate) use spec::RecoveryModel;
#[cfg(test)]
pub(super) use spec::RecoverySpec;
use spec::{validate_correlated_inputs, validate_finite_recovery_input, validate_recovery_input};
