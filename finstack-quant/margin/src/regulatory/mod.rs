//! Regulatory capital frameworks.
//!
//! This module provides implementations of standardized regulatory capital
//! calculations that complement the bilateral margin (SIMM) and XVA frameworks
//! already present in `finstack-quant-margin`.
//!
//! # Frameworks
//!
//! - **FRTB SBA** (Fundamental Review of the Trading Book, Sensitivity-Based
//!   Approach): Standardized market risk capital charge per BCBS d457. Computes
//!   delta, vega, curvature, default risk, and residual risk add-on components.
//!
//! - **SA-CCR** (Standardized Approach for Counterparty Credit Risk): Exposure at
//!   Default computation per BCBS 279. Replaces CEM/SM for computing EAD on
//!   derivative portfolios.

pub mod frtb;
pub mod sa_ccr;

pub use frtb::{
    CorrelationScenario, DrcAssetType, DrcPosition, DrcSector, DrcSeniority, FrtbRiskClass,
    FrtbSbaEngine, FrtbSbaEngineBuilder, FrtbSbaResult, FrtbSensitivities, RraoPosition,
};
pub use sa_ccr::{
    EadResult, SaCcrAssetClass, SaCcrEngine, SaCcrEngineBuilder, SaCcrNettingSetConfig,
    SaCcrOptionType, SaCcrTrade,
};
