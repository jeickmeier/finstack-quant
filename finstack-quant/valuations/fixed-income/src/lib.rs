#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
// Allow expect() in doc tests (they are test code)
#![doc(test(attr(allow(clippy::expect_used))))]

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
//! use finstack_quant_valuations_fixed_income::Bond;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::Rate;
//! use time::macros::date;
//!
//! // Create a 5-year USD Treasury bond
//! let bond = Bond::fixed(
//!     "UST-5Y",
//!     Money::from((1_000_000_i64, Currency::USD)),
//!     Rate::from_percent(4.5).expect("valid rate fixture"),
//!     date!(2025-01-15),
//!     date!(2030-01-15),
//!     finstack_quant_core::dates::StubKind::None,
//!     "USD-TREASURY",
//! ).expect("valid bond");
//!
//! assert_eq!(bond.id.as_str(), "UST-5Y");
//! ```
//!
//! # See Also
//!
//! - [`Bond`] for standard fixed/floating rate bonds
//! - `StructuredCredit` for ABS, CLO, and securitized products (the
//!   `finstack-quant-valuations-structured-credit` crate, re-exported by the
//!   umbrella as `instruments::fixed_income::structured_credit`)
//! - [`AgencyMbsPassthrough`] for mortgage pass-throughs
//! - [`finstack_quant_cashflows`] for cashflow generation

/// Bond module - Fixed and floating rate bonds.
pub mod bond;
/// Bond future module - Bond futures contracts.
pub mod bond_future;
/// Convertible bond module.
pub mod convertible;
/// Dollar roll module - MBS dollar rolls.
pub mod dollar_roll;
/// Fixed income TRS module - Fixed income index total return swaps.
pub mod fi_trs;
/// Inflation-linked bond module.
pub mod inflation_linked_bond;
/// Quote arithmetic shared by term loans and revolving credit facilities.
pub mod loan_quotes;
/// Dated loan terms shared by term loans and revolving credit facilities.
pub mod loan_terms;
/// MBS passthrough module - Agency MBS pass-throughs (renamed from agency_mbs_passthrough).
pub mod mbs_passthrough;
pub mod revolving_credit;
/// TBA module - To Be Announced trades (renamed from agency_tba).
pub mod tba;
/// Term loan module.
pub mod term_loan;

mod family;
pub use family::FIXED_INCOME;

pub use bond::{Bond, BondSettlementConvention};
pub use bond_future::{BondFuture, BondFutureBuilder, BondFutureSpecs, DeliverableBond};
pub use convertible::ConvertibleBond;
pub use dollar_roll::DollarRoll;
pub use fi_trs::FiIndexTotalReturnSwap;
pub use inflation_linked_bond::InflationLinkedBond;
pub use mbs_passthrough::{AgencyMbsPassthrough, AgencyProgram, PoolType};
pub use revolving_credit::RevolvingCredit;
pub use tba::{AgencyTba, TbaTerm};
pub use term_loan::TermLoan;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
