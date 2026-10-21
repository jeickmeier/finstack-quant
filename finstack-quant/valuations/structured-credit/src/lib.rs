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

//! Structured credit: securitised deals, agency CMOs and asset-backed
//! facilities.
//!
//! The second tier of the valuations stack. The three modules here build on
//! the fixed-income family (bond cashflow specs, loan terms, the revolving
//! credit Monte Carlo engine, the agency pass-through pool model) and on the
//! rates family (the deal-level interest-rate swap hedge), and share one
//! registration unit, [`structured_credit::STRUCTURED_CREDIT`]:
//!
//! - [`structured_credit`]: ABS, RMBS, CMBS and CLO deals as one instrument —
//!   collateral pool, tranche capital structure, payment waterfall with
//!   coverage tests, deal-type assumptions, deterministic and stochastic
//!   pricing.
//! - [`cmo`]: agency CMOs that redistribute pass-through collateral cashflows
//!   into sequential, PAC/support, IO/PO and accrual tranches.
//! - [`asset_backed_facility`]: warehouse and forward-flow lines against a
//!   collateral pool, priced through a synthesized two-class deal.
//!
//! The umbrella crate `finstack-quant-valuations` re-exports the three modules
//! as `finstack_quant_valuations::instruments::fixed_income::{structured_credit,
//! cmo, asset_backed_facility}`, so public paths there are unchanged.
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_valuations_core::instruments::Instrument;
//! use finstack_quant_valuations_structured_credit::cmo::AgencyCmo;
//! use finstack_quant_valuations_structured_credit::structured_credit::StructuredCredit;
//!
//! let deal = StructuredCredit::example().unwrap();
//! let cmo = AgencyCmo::example().unwrap();
//! assert_ne!(deal.id(), cmo.id());
//! ```

extern crate self as finstack_quant_valuations_structured_credit;

/// Asset-backed facility module - warehouse lines against collateral pools.
pub mod asset_backed_facility;
/// CMO module - Collateralized Mortgage Obligations.
pub mod cmo;
/// Structured credit module - ABS, RMBS, CMBS, CLO.
pub mod structured_credit;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
