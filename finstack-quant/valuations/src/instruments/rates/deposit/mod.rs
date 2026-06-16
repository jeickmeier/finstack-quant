//! Money market deposit instruments for short-term interest rates.
//!
//! Deposits are the simplest interest-bearing instruments, representing
//! unsecured lending between financial institutions. They are fundamental
//! building blocks for calibrating the short end of discount curves.
//!
//! # Structure
//!
//! - **Principal**: Amount deposited at start
//! - **Simple interest**: Accrues using day count convention
//! - **Single payment**: Principal + interest at maturity
//!
//! # Pricing
//!
//! Present value using discount curve:
//!
//! ```text
//! PV = -Principal·DF(t_start) + [Principal·(1 + r·τ)]·DF(t_end)
//! ```
//!
//! where:
//! - r = deposit rate (simple, not compounded)
//! - τ = year fraction (day count)
//! - DF = discount factor
//!
//! # Market Conventions
//!
//! Standard deposit conventions by currency:
//! - **USD**: ACT/360, T+2 settlement, overnight to 1 year
//! - **EUR**: ACT/360, T+2 settlement, overnight to 1 year
//! - **GBP**: ACT/365, T+0 settlement, overnight to 1 year
//! - **JPY**: ACT/365F (TONA convention), T+2 settlement, overnight to 1 year
//!
//! # Curve Calibration Role
//!
//! Deposits calibrate the very short end (overnight to 12 months):
//! - Overnight deposits → DF(T+1)
//! - 1W, 1M, 3M deposits → discount factors
//! - Longer maturities use OIS swaps or FRAs
//!
//! # See Also
//!
//! - [`Deposit`] for instrument struct
//! - Plan-driven calibration in `calibration::api` (Discount step) for curve bootstrap

pub(crate) mod metrics;
mod types;

pub use types::{ConventionDepositParams, Deposit};
