//! Dollar roll instrument module.
//!
//! This module provides the [`DollarRoll`] instrument for modeling
//! simultaneous sale and purchase of TBAs for different settlement months.
//!
//! # Overview
//!
//! A dollar roll is a financing trade where an investor:
//! 1. Sells TBA for near-month (front) settlement
//! 2. Buys TBA for far-month (back) settlement
//!
//! The price difference ("drop") represents the cost of financing
//! the MBS position for one month.
//!
//! # Key Features
//!
//! - **Implied financing**: Calculate implied repo rate from drop
//! - **Roll specialness**: Compare to market repo rates
//! - **Carry analysis**: Break-even and P&L projections
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_valuations::instruments::fixed_income::dollar_roll::DollarRoll;
//! use finstack_quant_valuations::instruments::fixed_income::tba::TbaTerm;
//! use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyProgram;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::{CurveId, InstrumentId};
//!
//! let roll = DollarRoll::builder()
//!     .id(InstrumentId::new("FN30-4.0-ROLL"))
//!     .agency(AgencyProgram::Fnma)
//!     .coupon(0.04)
//!     .term(TbaTerm::ThirtyYear)
//!     .notional(Money::new(10_000_000.0, Currency::USD))
//!     .front_settlement_year(2024)
//!     .front_settlement_month(3)
//!     .back_settlement_year(2024)
//!     .back_settlement_month(4)
//!     .front_price(98.5)
//!     .back_price(98.0)
//!     .discount_curve_id(CurveId::new("USD-OIS"))
//!     .build()
//!     .expect("Valid dollar roll");
//!
//! // Get the drop
//! let drop = roll.drop(); // 0.5 points = 16/32nds
//! ```

pub mod carry;
pub(crate) mod metrics;
pub(crate) mod pricer;
mod types;

pub use types::DollarRoll;
