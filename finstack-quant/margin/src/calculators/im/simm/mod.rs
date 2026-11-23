//! ISDA Standard Initial Margin Model (SIMM) calculator.
//!
//! Computes an indicative historical v2.6 approximation for uncleared OTC
//! derivatives. USD-normalized sensitivities are required. Product-class,
//! subcurve and some non-IR factor dimensions are incomplete; every ImResult
//! is marked approximate. This is not current regulatory SIMM.
//!
//! # ISDA SIMM Methodology
//!
//! SIMM calculates IM based on sensitivities across risk classes:
//! - Interest Rate (IR): DV01-style currency sensitivities by tenor bucket
//! - Credit Qualifying (CQ): CS01-style currency sensitivities for corporate, sovereign and index credit
//! - Credit Non-Qualifying (CNQ): CS01-style currency sensitivities for securitization credit
//! - Equity: signed currency delta and vega sensitivities
//! - Commodity: signed currency delta and vega sensitivities
//! - FX: signed currency delta and vega sensitivities
//!
//! # Formula
//!
//! ```text
//! IM = sqrt(sum_i sum_j ρ_ij × K_i × K_j)
//! ```
//!
//! Where K_i is the risk-weighted sensitivity for bucket i.
//!
//! > **Implementation note:** [`SimmCalculator::calculate_from_sensitivities`] applies intra-bucket
//! > tenor correlations for IR delta, vega margin (IR, credit qualifying,
//! > credit non-qualifying, equity, commodity, FX), curvature
//! > risk, concentration add-ons, and the SIMM risk-class correlation matrix.
//!
//! # Conventions
//!
//! - Risk weights and correlations are stored as decimal quantities in the
//!   registry, not basis points.
//! - Rate and credit delta inputs are expected to be DV01 or CS01 style
//!   currency amounts per 1bp move before they reach this module.
//! - Tenor keys must match the registry-backed tenor labels exactly.
//! - The aggregation currency is chosen by the caller to
//!   [`SimmCalculator::calculate_from_sensitivities`].
//!
//! # References
//!
//! - ISDA SIMM: `docs/REFERENCES.md#isda-simm`
//! - BCBS-IOSCO uncleared margin framework: `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`

pub(crate) mod calculator;
mod curvature;
pub(crate) mod params;
#[cfg(test)]
mod tests;
