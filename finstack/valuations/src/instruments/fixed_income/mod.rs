//! Fixed income instruments: bonds, loans, MBS, and structured products.
//!
//! This module provides fixed income instrument modeling including
//! government and corporate bonds, mortgage-backed securities, and structured
//! credit products. All instruments support cashflow generation, discounting,
//! and standard risk metrics (duration, convexity, DV01).
//!
//! # Features
//!
//! - **Bonds**: Fixed-rate, floating-rate, callable, putable, amortizing
//! - **Mortgage Securities**: Agency MBS pass-throughs, CMOs, TBAs, dollar rolls
//! - **Structured Credit**: ABS, CLO, RMBS, CMBS with tranches and waterfalls
//! - **Lending**: Term loans, revolving credit facilities
//! - **Derivatives**: Bond futures with CTD mechanics, FI index TRS
//!
//! # Pricing Models
//!
//! Fixed income instruments support multiple pricing approaches:
//! - **Discounting**: Present value of projected cashflows
//! - **Tree-based**: Hull-White short rate trees for callable/putable bonds
//! - **Quote-based**: Yield-to-maturity, clean/dirty price conversion
//! - **OAS**: Option-adjusted spread for embedded options
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_valuations::instruments::fixed_income::Bond;
//! use finstack_core::currency::Currency;
//! use finstack_core::money::Money;
//! use time::macros::date;
//!
//! // Create a 5-year USD Treasury bond
//! let bond = Bond::fixed(
//!     "UST-5Y",
//!     Money::new(1_000_000.0, Currency::USD),
//!     0.045, // 4.5% coupon
//!     date!(2025-01-15),
//!     date!(2030-01-15),
//!     "USD-TREASURY",
//! ).expect("valid bond");
//!
//! assert_eq!(bond.id.as_str(), "UST-5Y");
//! ```
//!
//! # See Also
//!
//! - [`Bond`] for standard fixed/floating rate bonds
//! - [`StructuredCredit`] for ABS, CLO, and securitized products
//! - [`AgencyMbsPassthrough`] for mortgage pass-throughs
//! - [`crate::cashflow`] for cashflow generation

/// Bond module - Fixed and floating rate bonds.
pub mod bond;
/// Bond future module - Bond futures contracts.
pub mod bond_future;
/// CMO module - Collateralized Mortgage Obligations (renamed from agency_cmo).
pub mod cmo;
/// Convertible bond module.
pub mod convertible;
/// Dollar roll module - MBS dollar rolls.
pub mod dollar_roll;
/// Fixed income TRS module - Fixed income index total return swaps.
pub mod fi_trs;
/// Inflation-linked bond module.
pub mod inflation_linked_bond;
/// MBS passthrough module - Agency MBS pass-throughs (renamed from agency_mbs_passthrough).
pub mod mbs_passthrough;
/// Revolving credit facility module.
pub mod revolving_credit;
/// Structured credit module - ABS, RMBS, CMBS, CLO.
pub mod structured_credit;
/// TBA module - To Be Announced trades (renamed from agency_tba).
pub mod tba;
/// Term loan module.
pub mod term_loan;

// Re-export primary types
pub use bond::{Bond, BondSettlementConvention};
pub use bond_future::{BondFuture, BondFutureBuilder, BondFutureSpecs, DeliverableBond};
pub use cmo::{AgencyCmo, CmoTranche, CmoTrancheType, CmoWaterfall};
pub use convertible::ConvertibleBond;
pub use dollar_roll::DollarRoll;
pub use fi_trs::FIIndexTotalReturnSwap;
pub use inflation_linked_bond::InflationLinkedBond;
pub use mbs_passthrough::{AgencyMbsPassthrough, AgencyProgram, PoolType};
pub use revolving_credit::RevolvingCredit;
pub use structured_credit::StructuredCredit;
pub use tba::{AgencyTba, TbaTerm};
pub use term_loan::TermLoan;
