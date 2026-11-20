//! XVA (valuation adjustments) types and aggregation formulas.
//!
//! Public surface: [`types`] (configuration, exposure profiles, results),
//! `cva::compute_cva`, `cva::compute_dva`, `cva::compute_fva`,
//! `cva::compute_bilateral_xva`, `mva::compute_mva`,
//! and `mva::im_profile_from_simm`.
//!
//! # Conventions
//!
//! - Exposure times are year fractions.
//! - Adjustments are positive when they cost the desk and compose as
//!   `total_xva = CVA − DVA + FVA + MVA`. `cva::compute_bilateral_xva`
//!   uses the supplied exposure basis for credit legs and expected posted IM
//!   for MVA when `types::FundingConfig::im_profile` is set.
//!
//! # Exposure profiles
//!
//! [`crate::xva::types::ExposureProfile`] is a container: callers supply the EPE / ENE grid
//! from their own simulation or valuation stack, and the formulas here turn it
//! into CVA / DVA / FVA / MVA.
//!
//! # References
//!
//! - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
//! - Green XVA: `docs/REFERENCES.md#green-xva`
//! - ISDA 2002 Master Agreement: `docs/REFERENCES.md#isda-2002-master-agreement`
//! - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`

pub mod cva;
/// CVA, DVA, FVA, MVA aggregation, and bilateral-XVA integration formulas.
mod dated;
/// Margin Valuation Adjustment (MVA) on SIMM-based IM profiles.
pub mod mva;
/// Shared XVA configuration and result container types.
pub mod types;
