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
//! Par Spread = Protection_PV / RPV01
//! ```
//!
//! where RPV01 (Risky PV01 or Risky Duration) is defined as:
//!
//! ```text
//! RPV01 = Σᵢ DF(tᵢ) × SP(tᵢ) × Δt(tᵢ₋₁, tᵢ)
//! ```
//!
//! - **DF(t)**: Discount factor from valuation date to time t
//! - **SP(t)**: Survival probability to time t (from hazard curve)
//! - **Δt**: Day count fraction for the accrual period
//!
//! This is the **Risky Annuity** excluding accrual-on-default, which matches
//! the ISDA CDS Standard Model convention.
//!
//! ## ISDA vs Bloomberg CDSW Methodology
//!
//! | Methodology | Denominator | Use Case |
//! |-------------|-------------|----------|
//! | ISDA Standard Model | Risky Annuity only | Default, curve building |
//! | Bloomberg CDSW | Includes accrual-on-default | Trading desk analytics |
//!
//! The difference is typically:
//! - **< 1bp** for investment grade credits (hazard rate < 1%)
//! - **2-5 bps** for high yield/distressed credits (hazard rate > 3%)
//!
//! To use Bloomberg CDSW-style calculations, set `par_spread_uses_full_premium = true`
//! in the [`CDSPricerConfig`].
//!
//! # Day Count Convention Handling
//!
//! The CDS pricer uses **multiple day count conventions** for different purposes,
//! following market standard practice:
//!
//! | Calculation | Day Count Source | Rationale |
//! |-------------|------------------|-----------|
//! | **Accrual fraction** | Instrument premium leg (`premium.dc`) | ACT/360 for NA, ACT/365F for Asia |
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
//! - ISDA CDS Standard Model (Markit, 2009)
//! - O'Kane, D. "Modelling Single-name and Multi-name Credit Derivatives" (2008), Chapter 5
//! - Hull, J.C. & White, A. "Valuing Credit Default Swaps I: No Counterparty Default Risk"

mod config;
mod engine;
mod helpers;
mod integration;
mod metrics;

#[cfg(test)]
mod tests;

pub(crate) use config::CDSPricerConfig;
pub(crate) use engine::CDSPricer;
pub(crate) use helpers::date_from_hazard_time;
pub(crate) use metrics::AccrualDayCountPolicy;
