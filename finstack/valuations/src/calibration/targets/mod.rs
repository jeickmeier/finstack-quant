//! Calibration targets for different instrument types.
//!
//! This module provides targets that bridge between the plan-driven calibration API
//! and the domain-specific optimization logic for various financial instruments.
//!
//! # Features
//! - **Standardized Interface**: All targets implement common traits like [`BootstrapTarget`]
//!   or [`GlobalSolveTarget`].
//! - **Broad Asset Coverage**: Support for Interest Rates (IR), Credit, Inflation, and Volatility.
//! - **Automatic Convention Resolution**: Adapters handle the mapping from high-level
//!   market quotes to concrete pricing inputs.
//!
//! # See Also
//! - [`crate::calibration::api`] for the schema that drives these targets.

/// Base correlation curve bootstrapping from CDS tranche quotes.
pub(crate) mod base_correlation;
/// Discount curve bootstrapping from rate quotes.
pub(crate) mod discount;
/// Forward curve bootstrapping from rate quotes.
pub(crate) mod forward;
/// Hazard curve bootstrapping from CDS quotes.
pub(crate) mod hazard;
/// Inflation curve bootstrapping from inflation swap quotes.
pub(crate) mod inflation;
/// LMM/BGM factor-loading calibration to the swaption volatility surface.
pub(crate) mod lmm;
/// Nelson-Siegel / Nelson-Siegel-Svensson parametric curve calibration.
pub(crate) mod parametric;
/// Student-t copula degrees of freedom calibration.
pub(crate) mod student_t;
/// SVI volatility surface calibration.
pub(crate) mod svi;
/// Swaption volatility surface calibration.
pub(crate) mod swaption;
/// Shared utility functions for calibration targets.
pub(crate) mod util;
/// Option volatility surface calibration.
pub(crate) mod vol;
/// Cross-currency basis curve bootstrapping.
pub(crate) mod xccy_basis;
