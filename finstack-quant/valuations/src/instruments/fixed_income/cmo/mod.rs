//! Agency CMO (Collateralized Mortgage Obligation) module.
//!
//! This module provides the [`AgencyCmo`] instrument for modeling CMO deals
//! backed by agency MBS collateral with multiple tranches.
//!
//! # Overview
//!
//! CMOs are structured products that redistribute MBS cashflows into
//! tranches with different risk/return profiles. This module supports:
//!
//! - **Sequential tranches**: Principal paid in priority order
//! - **PAC/Support**: Protected amortization class with support absorption
//! - **IO/PO strips**: Interest-only and principal-only components
//!
//! # Waterfall Engine
//!
//! The waterfall engine distributes collateral cashflows to tranches
//! according to the deal structure. Key features:
//!
//! - Interest allocation based on tranche coupon and balance
//! - Principal allocation by priority (sequential) or rules (PAC/support)
//! - Support for pro-rata allocation within same priority
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_valuations::instruments::fixed_income::cmo::{
//!     AgencyCmo, CmoTranche, CmoWaterfall,
//! };
//! use finstack_quant_valuations::instruments::fixed_income::mbs_passthrough::AgencyProgram;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::dates::Date;
//! use finstack_quant_core::types::{CurveId, InstrumentId};
//! use time::Month;
//!
//! // Create a sequential CMO structure (coupon demand must stay within the
//! // collateral pass-through interest, ~4.0% here after 50bp of fees)
//! let tranches = vec![
//!     CmoTranche::sequential("A", Money::new(40_000_000.0, Currency::USD), 0.035, 1),
//!     CmoTranche::sequential("B", Money::new(30_000_000.0, Currency::USD), 0.04, 2),
//!     CmoTranche::sequential("Z", Money::new(30_000_000.0, Currency::USD), 0.045, 3),
//! ];
//!
//! let cmo = AgencyCmo::builder()
//!     .id(InstrumentId::new("FNR-2024-1-A"))
//!     .deal_name("FNR 2024-1".into())
//!     .agency(AgencyProgram::Fnma)
//!     .issue_date(Date::from_calendar_date(2024, Month::January, 1).unwrap())
//!     .waterfall(CmoWaterfall::new(tranches))
//!     .reference_tranche_id("A".to_string())
//!     .collateral_wac(0.045)
//!     .collateral_wam(360)
//!     .discount_curve_id(CurveId::new("USD-OIS"))
//!     .build()
//!     .expect("Valid CMO");
//! ```

pub(crate) mod metrics;
pub(crate) mod pricer;
pub mod tranches;
mod types;
pub mod waterfall;

pub use types::{AgencyCmo, CmoTranche, CmoTrancheType, CmoWaterfall, PacCollar};
