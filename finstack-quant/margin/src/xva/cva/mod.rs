//! Credit and Funding Valuation Adjustments (CVA, DVA, FVA).
//!
//! Implements unilateral CVA, DVA (debit valuation adjustment), FVA (funding
//! valuation adjustment), and bilateral XVA for OTC derivative portfolios.
//!
//! # Mathematical Framework
//!
//! **Unilateral CVA** — expected loss from counterparty default:
//!
//! ```text
//! CVA = (1 - R) × Σᵢ EPE_mid(tᵢ) × [S(tᵢ₋₁) - S(tᵢ)] × DF_mid(tᵢ)
//! ```
//!
//! **DVA** — expected gain from own default (mirror of CVA using ENE):
//!
//! ```text
//! DVA = (1 - R_own) × Σᵢ ENE_mid(tᵢ) × [S_own(tᵢ₋₁) - S_own(tᵢ)] × DF_mid(tᵢ)
//! ```
//!
//! [`crate::xva::cva::compute_bilateral_xva`] and the unilateral
//! [`crate::xva::cva::compute_dva`] integrate the supplied ENE as is. Posted
//! initial margin is not deducted; a collateral-aware exposure profile must
//! already reflect it.
//!
//! **FVA** — funding cost/benefit on uncollateralized exposure:
//!
//! ```text
//! FVA = Σᵢ [EPE_mid(tᵢ) × s_f⁺ - ENE_mid(tᵢ) × s_f⁻] × DF_mid(tᵢ) × Δtᵢ
//! ```
//!
//! **MVA** — funding cost of posted initial margin (see [`crate::xva::mva`]):
//!
//! ```text
//! MVA = Σᵢ s_im × IM_mid(tᵢ) × DF_mid(tᵢ) × S_joint_mid(tᵢ) × Δtᵢ
//! ```
//!
//! **Total XVA** = CVA − DVA + FVA + MVA
//!
//! where `EPE_mid` and `DF_mid` are averaged over consecutive time points
//! for O(Δt²) convergence.
//!
//! Notation:
//! - `R`, `R_own` = recovery rates (counterparty, own)
//! - `EPE(t)` = expected positive exposure at time `t`
//! - `ENE(t)` = expected negative exposure at time `t`
//! - `S(t)`, `S_own(t)` = survival probabilities (counterparty, own)
//! - `DF(t)` = risk-free discount factor at time `t`
//! - `s_f⁺`, `s_f⁻` = funding spread (cost) and funding benefit spread
//!
//! # References
//!
//! - Gregory XVA Challenge: `docs/REFERENCES.md#gregory-xva-challenge`
//! - Green XVA: `docs/REFERENCES.md#green-xva`
//! - BCBS 279 SA-CCR: `docs/REFERENCES.md#bcbs-279-saccr`

mod calculator;
#[cfg(test)]
mod tests;

pub use calculator::{compute_bilateral_xva, compute_cva, compute_dva, compute_fva};
