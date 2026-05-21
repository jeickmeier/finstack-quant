//! XVA (valuation adjustments) types and crate-internal exposure engines.
//!
//! Public surface: [`types`] (configuration, exposure profiles, results) for
//! bindings and downstream integration. Exposure roll-forward, netting,
//! collateral reduction, and CVA/DVA/FVA formulas live in sibling modules and
//! are exercised from this crate's tests.
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
//! Pathwise exposure with quantile-based PFE uses `finstack-monte-carlo` via
//! `exposure::compute_stochastic_exposure_profile` (crate-internal). Defaults
//! are in `data/margin/xva_defaults.v1.json`.
//!
//! # References
//!
//! - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
//! - Green XVA: `docs/REFERENCES.md#green-xva`
//! - ISDA 2002 Master Agreement: `docs/REFERENCES.md#isda-2002-master-agreement`
//! - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`

/// CVA, DVA, FVA, and bilateral-XVA integration formulas.
#[cfg(test)]
pub(crate) mod cva;
/// Deterministic and stochastic exposure engines.
#[cfg(test)]
pub(crate) mod exposure;
/// Netting and collateral-reduction helpers.
#[cfg(test)]
pub(crate) mod netting;
/// Minimal trait surface for XVA-compatible instruments.
#[cfg(test)]
pub(crate) mod traits;
/// Shared XVA configuration and result container types.
pub mod types;
