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
//! - **Commitment fee**: Paid on undrawn amount (bps per annum)
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
//! # Convention Limitations
//!
//! The current implementation uses **term-style** forward rate projection for
//! floating-rate facilities. It does **not** apply overnight compounding
//! (SOFR in-arrears), fixing lags, or payment lags from `FloatingRateSpec`.
//! If your facility requires these (e.g., daily-compounded SOFR with a 2-day
//! lookback), extend `utils::build_reset_dates` and
//! `utils::project_floating_rate_with_curve`.
//!
//! # Numerical Constants
//!
//! This module uses centralized numerical tolerances for consistency:
//! - `ZERO_TOLERANCE`: General zero comparison threshold (1e-8)
//! - `UTILIZATION_CHANGE_THRESHOLD`: Threshold for detecting utilization changes (1e-6)
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
pub(crate) mod pricer;
pub(crate) mod types;

mod utils;

// ============================================================================
// Numerical Constants
// ============================================================================
// Centralized thresholds for numerical stability and consistency across the module.

/// General zero comparison threshold for numerical stability.
/// Used for comparing floating-point values to zero.
pub const ZERO_TOLERANCE: f64 = 1e-8;

/// Threshold for detecting significant utilization changes.
/// Changes smaller than this are treated as noise and ignored.
pub const UTILIZATION_CHANGE_THRESHOLD: f64 = 1e-6;

/// Tolerance for interpolation equality checks.
/// Used when comparing time points or interpolation boundaries.
pub const INTERPOLATION_TOLERANCE: f64 = 1e-10;

/// Minimum spread value for CIR process stability.
/// Ensures spreads don't go to exactly zero, which would cause numerical issues.
pub const MIN_CIR_SPREAD: f64 = 1e-8;

/// Maximum allowed recovery rate (exclusive).
/// Recovery rate must be strictly less than 1.0 to avoid division by zero
/// in hazard-to-spread mapping: λ = s / (1 - R).
pub const MAX_RECOVERY_RATE: f64 = 1.0 - 1e-6;

// Re-export main types
pub use cashflow_engine::{PathAwareCashflowSchedule, ThreeFactorPathData};
pub use pricer::unified::EnhancedMonteCarloResult;
pub use pricer::unified::PathResult;
pub use pricer::unified::RevolvingCreditPricer;
pub use types::{
    BaseRateSpec, DrawRepayEvent, DrawRepaySpec, RevolvingCredit, RevolvingCreditFees,
    UtilizationProcess,
};

pub use types::{
    CreditSpreadProcessSpec, InterestRateProcessSpec, McConfig, StochasticUtilizationSpec,
};
