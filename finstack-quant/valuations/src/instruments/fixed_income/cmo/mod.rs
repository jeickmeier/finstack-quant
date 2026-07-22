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
//! - **Accrual (Z) bonds**: Interest capitalized while current-pay tranches
//!   are outstanding; the accrual is redirected as accretion-directed
//!   principal to the current-pay tranches (Fabozzi, *The Handbook of
//!   Mortgage-Backed Securities*, 7th ed., Ch. 21)
//!
//! Validation requires every fixed-coupon principal tranche — including an
//! accrual (Z) tranche, whose accrual is funded from interest collections —
//! to have a coupon at or below the collateral's net pass-through coupon.
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
//! // Create a sequential CMO structure. Every fixed-coupon tranche must stay
//! // at or below the net pass-through coupon (~4.0% here after 50bp of fees).
//! let tranches = vec![
//!     CmoTranche::sequential("A", Money::new(40_000_000.0, Currency::USD), 0.035, 1),
//!     CmoTranche::sequential("B", Money::new(30_000_000.0, Currency::USD), 0.04, 2),
//!     CmoTranche::sequential("C", Money::new(30_000_000.0, Currency::USD), 0.04, 3),
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
//!
//! A sequential structure with an accrual (Z) tranche: while A and B are
//! outstanding the Z receives no cash — its coupon accrual is capitalized
//! into its balance and redirected as accretion-directed principal, which
//! retires A and B faster than a plain sequential:
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
//! let tranches = vec![
//!     CmoTranche::sequential("A", Money::new(40_000_000.0, Currency::USD), 0.035, 1),
//!     CmoTranche::sequential("B", Money::new(30_000_000.0, Currency::USD), 0.04, 2),
//!     CmoTranche::accrual("Z", Money::new(30_000_000.0, Currency::USD), 0.04, 3),
//! ];
//!
//! let cmo = AgencyCmo::builder()
//!     .id(InstrumentId::new("FNR-2024-3-Z"))
//!     .deal_name("FNR 2024-3".into())
//!     .agency(AgencyProgram::Fnma)
//!     .issue_date(Date::from_calendar_date(2024, Month::January, 1).unwrap())
//!     .waterfall(CmoWaterfall::new(tranches))
//!     .reference_tranche_id("Z".to_string())
//!     .collateral_wac(0.045)
//!     .collateral_wam(360)
//!     .discount_curve_id(CurveId::new("USD-OIS"))
//!     .build()
//!     .expect("Valid Z CMO");
//! ```

pub(crate) mod metrics;
pub(crate) mod pricer;
pub mod tranches;
mod types;
pub mod waterfall;

pub use types::{AgencyCmo, CmoTranche, CmoTrancheType, CmoWaterfall, PacCollar};
