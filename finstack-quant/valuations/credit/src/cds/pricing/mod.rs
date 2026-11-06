//! Core CDS pricing engine and helpers.
//!
//! Provides deterministic valuation for Credit Default Swaps (CDS) with
//! support for ISDA-style premium schedules, accrual-on-default, and
//! ISDA Standard Model integration for the protection leg.
//!
//! The engine exposes present value calculations for the protection and
//! premium legs, par spread, risky annuity, and PV01/CS01. Heavy numerics are
//! kept here to isolate pricing policy from instrument data shapes.
//!
//! # Par Spread Calculation
//!
//! The par spread is the spread at which the CDS has zero initial value (i.e.,
//! protection leg PV equals premium leg PV). It is calculated as:
//!
//! ```text
//! Par Spread = Protection_PV / Denominator
//! ```
//!
//! The denominator is selected by the instrument's `valuation_convention`:
//!
//! | Convention | Denominator |
//! |------------|-------------|
//! | `BloombergCdswClean` (default) | Premium-leg annuity including accrual-on-default, minus the accrued fraction |
//! | `BloombergCdswCleanFullPremium` | Premium-leg annuity including accrual-on-default, no accrued subtraction |
//! | `QuantLibIsdaParity` | Premium-leg annuity including accrual-on-default, no accrued subtraction |
//! | `IsdaDirty` | Risky annuity `RPV01` excluding accrual-on-default |
//!
//! where, for `IsdaDirty`:
//!
//! ```text
//! RPV01 = Σᵢ DF(tᵢ) × SP(tᵢ) × Δt(tᵢ₋₁, tᵢ)
//! ```
//!
//! - **DF(t)**: Discount factor from valuation date to time t
//! - **SP(t)**: Survival probability to time t (from hazard curve)
//! - **Δt**: Day count fraction for the accrual period
//!
//! The difference from including accrual-on-default is typically:
//! - **< 1bp** for investment grade credits (hazard rate < 1%)
//! - **2-5 bp** for high yield/distressed credits (hazard rate > 3%)
//!
//! # Day Count Convention Handling
//!
//! The CDS pricer uses **multiple day count conventions** for different purposes,
//! following market standard practice:
//!
//! | Calculation | Day Count Source | Rationale |
//! |-------------|------------------|-----------|
//! | **Accrual fraction** | Instrument premium leg (`premium.day_count`) | ACT/360 for NA, ACT/365F for Asia |
//! | **Survival time axis** | Hazard curve (`surv.day_count()`) | Consistent with curve construction |
//! | **Discount time axis** | Discount curve (`disc.day_count()`) | Consistent with yield curve |
//!
//! ## Accrual-on-Default (AoD) Day Count
//!
//! The accrual-on-default calculation uses the **instrument's premium leg day count**
//! for the accrual fraction (the portion of coupon accrued before default), while
//! the default timing within the period uses the **hazard curve's day count** for
//! survival probability interpolation.
//!
//! For most NA CDS (ACT/360 premium on ACT/360 hazard curves), this is identical.
//! For Asian CDS (ACT/365F premium on ACT/360 hazard curves), there can be a small
//! (~1%) difference in AoD contribution. This is the expected behavior as:
//! - The premium accrual represents the contractual payment calculation
//! - The survival probability represents the market's view of default timing
//!
//! ## References
//!
//! - ISDA CDS Standard Model (Markit, 2009) `docs/REFERENCES.md#isda-cds-standard-model`
//! - O'Kane, D. "Modelling Single-name and Multi-name Credit Derivatives" (2008), Chapter 5 `docs/REFERENCES.md#o-kane-2008`
//! - Hull, J.C. & White, A. "Valuing Credit Default Swaps I: No Counterparty Default Risk" `docs/REFERENCES.md#hull-white-2000-cds`

mod engine;
mod helpers;
mod metrics;

#[cfg(test)]
mod tests;

pub(crate) use engine::CdsHazardRepriceCache;
#[doc(hidden)]
pub use engine::CdsPricer;
pub(crate) use helpers::date_from_hazard_time;
pub(crate) use metrics::AccrualDayCountPolicy;
