//! Stochastic prepayment models for structured credit.
//!
//! This module provides factor-driven prepayment models that capture:
//! - Interest rate sensitivity (refinancing incentive)
//! - Burnout effects (pool exhaustion)
//! - Seasonality patterns
//! - Correlation with systematic factors
//!
//! # Models
//!
//! - **FactorCorrelatedPrepay**: Base CPR shocked by systematic factor
//! - **RichardRollPrepay**: Full RMBS prepayment model with refi incentive
//! - **RegimeSwitchingPrepay**: Two-state prepayment model
//!
//! # References
//!
//! - Richard, S.F., & Roll, R. (1989). "Prepayments on Fixed-Rate Mortgage-Backed Securities."
//! - Schwartz, E.S., & Torous, W.N. (1989). "Prepayment and the Valuation of Mortgage-Backed Securities."

mod factor_correlated;
mod regime_switching;
mod richard_roll;
mod spec;
mod traits;

pub(crate) use factor_correlated::FactorCorrelatedPrepay;
pub(crate) use regime_switching::RegimeSwitchingPrepay;
pub(crate) use richard_roll::RichardRollPrepay;
pub use spec::StochasticPrepaySpec;
pub(crate) use traits::StochasticPrepayment;
