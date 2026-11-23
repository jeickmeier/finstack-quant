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
//! `RateSpec::Fixed` uses the contractual rate. Floating facilities choose
//! term vs overnight from the index and `FloatingRateSpec`:
//! - term indices such as `USD-SOFR-3M` project a single forward
//! - registered overnight RFR indices such as `USD-SOFR-OIS`, or an explicit
//!   `compounding` method, compound daily fixings in arrears
//! - reset lag is applied when building the reset grid
//!
//! # Numerical Constants
//!
//! This module uses centralized numerical tolerances for consistency:
//! - `INTERPOLATION_TOLERANCE`: Tolerance for interpolation equality checks (1e-10)
//!
//! # See Also
//!
//! - [`crate::revolving_credit::RevolvingCredit`] for instrument struct
//! - [`crate::revolving_credit::DrawRepayEvent`] for utilization events
//! - [`crate::revolving_credit::RevolvingCreditFees`] for fee specifications
//! - [`crate::revolving_credit::UtilizationProcess`] for stochastic modeling

pub mod cashflow_engine;
mod constants;
pub(crate) mod metrics;
mod pricing;
pub(crate) mod types;

pub(crate) mod schedule_projection;
mod validation;

pub use constants::{
    INTERPOLATION_TOLERANCE, MAX_RECOVERY_RATE, MC_CLOCK_DAY_COUNT, MIN_CIR_SPREAD,
};

pub use types::facility::{RevolvingCredit, RevolvingCreditBuilder};
pub use types::fees::RevolvingCreditFees;
pub use types::specs::{DrawRepayEvent, DrawRepaySpec, UtilizationProcess};

pub use types::specs::{
    CreditSpreadProcessSpec, InterestRateProcessSpec, McConfig, RevolvingCreditMcRun,
    StochasticUtilizationSpec,
};

pub use pricing::{
    path_generator::build_credit_spread_params,
    results::{EnhancedMonteCarloResult, PathResult},
    unified::{DEFAULT_CREDIT_SPREAD_IMPLIED_VOL, DEFAULT_UTIL_CREDIT_CORR, RevolvingCreditPricer},
};
