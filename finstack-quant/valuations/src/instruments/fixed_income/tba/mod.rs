//! Agency TBA (To-Be-Announced) forward instrument module.
//!
//! This module provides the [`AgencyTba`] instrument for modeling TBA
//! forward contracts on agency MBS.
//!
//! # Overview
//!
//! TBA trades are forward contracts where the buyer agrees to purchase
//! agency MBS at a specified price for future settlement. The specific
//! pools to be delivered are not known at trade time - they must only
//! meet good delivery standards.
//!
//! # Key Features
//!
//! - **Forward pricing**: Value based on assumed pool characteristics
//! - **Settlement conventions**: SIFMA standard notification and settlement dates
//! - **Simplified CTD**: Uses on-the-run pool assumptions
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_valuations::instruments::fixed_income::tba::{AgencyTba, TbaTerm};
//! use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyProgram;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::{CurveId, InstrumentId};
//!
//! let tba = AgencyTba::builder()
//!     .id(InstrumentId::new("FN30-4.0-202403"))
//!     .agency(AgencyProgram::Fnma)
//!     .coupon(0.04)
//!     .term(TbaTerm::ThirtyYear)
//!     .settlement_year(2024)
//!     .settlement_month(3)
//!     .notional(Money::new(10_000_000.0, Currency::USD))
//!     .trade_price(98.5)
//!     .discount_curve_id(CurveId::new("USD-OIS"))
//!     .build()
//!     .expect("Valid TBA");
//! ```

pub mod allocation;
pub(crate) mod metrics;
pub(crate) mod pricer;
pub mod settlement;
mod types;

pub use types::{AgencyTba, TbaSettlement, TbaTerm};
