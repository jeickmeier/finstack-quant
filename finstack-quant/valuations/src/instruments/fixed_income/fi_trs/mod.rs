//! Fixed Income Index Total Return Swap (TRS) for synthetic bond exposure.
//!
//! This module provides the [`FIIndexTotalReturnSwap`] instrument for exchanging
//! fixed income index total return (carry + roll) for a financing rate.
//!
//! # Overview
//!
//! Total return swaps exchange the total return of an underlying index for a financing
//! rate. For fixed income index TRS, we use a carry model:
//!
//! ```text
//! Total return per period = e^{y × dt} - 1
//! ```
//!
//! where `y` is the continuous index yield and `dt` is the accrual period year fraction.
//! See the pricer module for full model documentation and rationale.
//!
//! # Use Cases
//!
//! - **Synthetic bond exposure**: Gain bond index exposure without buying bonds
//! - **Duration management**: Adjust portfolio duration synthetically
//! - **ETF replication**: Replicate bond ETF returns synthetically
//! - **Credit exposure**: Access corporate bond index returns
//!
//! # Key Metrics
//!
//! - **DurationDv01**: Duration-based yield sensitivity (`N × D × 1bp`)
//! - **DV01**: Sensitivity to financing rate
//! - **BucketedDV01**: Key-rate sensitivities
//! - **ParSpread**: Spread that makes NPV = 0
//!
//! # Example
//!
//! ```
//! use finstack_quant_valuations::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap;
//!
//! let trs = FIIndexTotalReturnSwap::example().unwrap();
//! // let pv = trs.value(&market_context, as_of_date)?;
//! ```
//!
//! # See Also
//!
//! - equity TRS module for equity total return swaps
//! - [`TrsEngine`](crate::instruments::common_impl::pricing::TrsEngine) for shared pricing logic

pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use types::FIIndexTotalReturnSwap;

// Re-export common TRS types for convenience
pub use crate::instruments::common_impl::parameters::trs_common::{TrsScheduleSpec, TrsSide};
