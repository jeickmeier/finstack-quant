//! Revolving credit facility instruments with stochastic utilization.
//!
//! Models corporate revolving credit facilities (revolvers) with:
//! - Draw and repayment schedules (deterministic or stochastic)
//! - Complex fee structures (commitment, usage, facility, upfront)
//! - Floating rate bases with spreads
//! - Utilization limits and covenants
//!
//! # Facility Structure
//!
//! - **Commitment**: Maximum drawable amount
//! - **Utilization**: Amount currently drawn
//! - **Availability**: Commitment - Utilization
//! - **Pricing**: Interest on drawn amounts + fees on commitment
//!
//! # Fee Structure
//!
//! Typical fees:
//! - **Upfront fee**: Paid at facility origination (% of commitment)
//! - **Commitment fee**: Paid on undrawn amount (bp per annum)
//! - **Usage fee**: Additional spread when utilization > threshold
//! - **Facility fee**: Paid on total commitment regardless of usage
//!
//! # Pricing
//!
//! Present value combines interest, fees, and principal flows (lender perspective):
//!
//! ```text
//! PV = PV(interest) + PV(fees) + PV(upfront) - PV(initial draw) + PV(repayments)
//! ```
//!
//! For stochastic utilization, Monte Carlo simulates draw/repayment paths.
//!
//! # Utilization Modeling
//!
//! - **Deterministic**: Fixed draw/repayment schedule
//! - **Stochastic**: Monte Carlo with mean reversion to target utilization
//! - **Seasonal**: Cyclical patterns (e.g., retail seasonal borrowing)
//!
//! # Key Metrics
//!
//! - **Facility value**: PV of all cashflows
//! - **Utilization rate**: Drawn / Commitment
//! - **DV01**: Interest rate sensitivity
//! - **CS01**: Credit spread sensitivity
//!
//! # Rate conventions
//!
//! `BaseRateSpec::Fixed` uses the contractual rate. Floating facilities choose
//! term vs overnight from the index and `FloatingRateSpec`:
//! - term indices such as `USD-SOFR-3M` project a single forward
//! - registered overnight RFR indices such as `USD-SOFR-OIS`, or an explicit
//!   `overnight_compounding` method, compound daily fixings in arrears
//! - reset lag is applied when building the reset grid
//!
//! # Numerical Constants
//!
//! This module uses centralized numerical tolerances for consistency:
//! - `ZERO_TOLERANCE`: General zero comparison threshold (1e-8)
//! - `INTERPOLATION_TOLERANCE`: Tolerance for interpolation equality checks (1e-10)
//!
//! # See Also
//!
//! - [`crate::instruments::fixed_income::revolving_credit::RevolvingCredit`] for instrument struct
//! - [`crate::instruments::fixed_income::revolving_credit::DrawRepayEvent`] for utilization events
//! - [`crate::instruments::fixed_income::revolving_credit::RevolvingCreditFees`] for fee specifications
//! - [`crate::instruments::fixed_income::revolving_credit::UtilizationProcess`] for stochastic modeling

pub mod cashflow_engine;
pub(crate) mod metrics;
pub(crate) mod pricing;
pub(crate) mod types;

mod utils;

// Numerical Constants
// Centralized thresholds for numerical stability and consistency across the module.

/// General zero comparison threshold for numerical stability.
/// Used for comparing floating-point values to zero.
pub const ZERO_TOLERANCE: f64 = 1e-8;

/// Tolerance for interpolation equality checks.
/// Used when comparing time points or interpolation boundaries.
pub const INTERPOLATION_TOLERANCE: f64 = 1e-10;

/// Minimum spread value for CIR process stability.
/// Ensures spreads don't go to exactly zero, which would cause numerical issues.
pub const MIN_CIR_SPREAD: f64 = 1e-8;

/// Maximum allowed recovery rate (inclusive).
pub const MAX_RECOVERY_RATE: f64 = 1.0;

/// Day count of the Monte Carlo model clock.
///
/// Factor paths (`ThreeFactorPathData::time_points`), the pathwise bank
/// account and pathwise survival integrate on ACT/365F years, the clock the
/// `finstack_quant_models` rate and credit processes are calibrated on. Interest
/// and fee accrual keep the facility's own `day_count`; only the simulation
/// time axis is fixed here, so an ACT/360 facility no longer over-accrues its
/// Hull-White numeraire by the 365/360 ratio.
pub const MC_CLOCK_DAY_COUNT: finstack_quant_core::dates::DayCount =
    finstack_quant_core::dates::DayCount::Act365F;

pub use cashflow_engine::{PathAwareCashflowSchedule, ThreeFactorPathData};
pub use pricing::unified::EnhancedMonteCarloResult;
pub use pricing::unified::PathResult;
pub use pricing::unified::RevolvingCreditPricer;
pub use types::{
    BaseRateSpec, DrawRepayEvent, DrawRepaySpec, RevolvingCredit, RevolvingCreditBuilder,
    RevolvingCreditFees, UtilizationProcess,
};

pub use types::{
    CreditSpreadProcessSpec, InterestRateProcessSpec, McConfig, StochasticUtilizationSpec,
};
