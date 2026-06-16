//! Equity Total Return Swap (TRS) for synthetic equity exposure.
//!
//! This module provides the [`EquityTotalReturnSwap`] instrument for exchanging
//! equity index total return (price appreciation + dividends) for a financing rate.
//!
//! # Overview
//!
//! Total return swaps exchange the total return of an underlying index for a financing
//! rate. For equity TRS:
//!
//! ```text
//! Total return = (Index_end - Index_start) / Index_start + Dividend_yield × T
//! ```
//!
//! # Use Cases
//!
//! - **Synthetic long exposure**: Gain equity index exposure without buying assets
//! - **Leverage**: Minimize upfront capital requirements
//! - **ETF replication**: Replicate equity ETF returns synthetically
//! - **Short exposure**: Easier than borrowing securities
//!
//! # Key Metrics
//!
//! - **Delta**: Sensitivity to underlying equity index
//! - **Dividend01**: Sensitivity to dividend yield changes
//! - **DV01**: Sensitivity to financing rate
//! - **ParSpread**: Spread that makes NPV = 0
//!
//! # Example
//!
//! ```
//! use finstack_quant_valuations::instruments::equity::equity_trs::EquityTotalReturnSwap;
//!
//! let trs = EquityTotalReturnSwap::example().unwrap();
//! // let pv = trs.value(&market_context, as_of_date)?;
//! ```
//!
//! # See Also
//!
//! - [`crate::instruments::fixed_income::fi_trs`] for fixed income index TRS
//! - [`TrsEngine`](crate::instruments::common_impl::pricing::TrsEngine) for shared pricing logic

pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use types::EquityTotalReturnSwap;

// Re-export common TRS types for convenience
pub use crate::instruments::common_impl::parameters::trs_common::{TrsScheduleSpec, TrsSide};
