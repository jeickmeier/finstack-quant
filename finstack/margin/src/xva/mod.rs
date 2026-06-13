//! XVA (valuation adjustments) types and deterministic exposure engines.
//!
//! Public surface: [`types`] (configuration, exposure profiles, results),
//! [`exposure::compute_exposure_profile`], [`cva::compute_cva`],
//! [`cva::compute_dva`], [`cva::compute_fva`], [`cva::compute_bilateral_xva`],
//! and the deterministic netting helpers.
//!
//! # Conventions
//!
//! - Exposure times are year fractions.
//! - Deterministic exposure assumes constant curves on the roll-forward grid.
//! - Close-out netting follows an ISDA master-agreement view.
//! - CSA terms reduce exposure; MPOR gap risk is not modeled in the
//!   deterministic engine.
//!
//! # Stochastic exposure
//!
//! Pathwise exposure with quantile-based PFE uses `finstack-monte-carlo` behind
//! a crate-private helper. Defaults
//! are in `data/margin/xva_defaults.v1.json`.
//!
//! # References
//!
//! - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
//! - Green XVA: `docs/REFERENCES.md#green-xva`
//! - ISDA 2002 Master Agreement: `docs/REFERENCES.md#isda-2002-master-agreement`
//! - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`

/// CVA, DVA, FVA, and bilateral-XVA integration formulas.
pub mod cva;
/// Deterministic and stochastic exposure engines.
pub mod exposure;
/// Netting and collateral-reduction helpers.
pub mod netting;
/// Minimal trait surface for XVA-compatible instruments.
pub mod traits;
/// Shared XVA configuration and result container types.
pub mod types;
