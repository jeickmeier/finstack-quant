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
//! use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
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
//! - [`Bond`](crate::instruments::fixed_income::bond::Bond) for standard fixed/floating rate bonds
//! - [`StructuredCredit`](crate::instruments::fixed_income::structured_credit::StructuredCredit) for ABS, CLO, and securitized products
//! - [`AgencyMbsPassthrough`](crate::instruments::fixed_income::mbs_passthrough::AgencyMbsPassthrough) for mortgage pass-throughs
//! - [`finstack_quant_cashflows`] for cashflow generation
//!
//! The family lives in `finstack-quant-valuations-fixed-income` and is
//! re-exported here unchanged; the structured-credit tier
//! (`structured_credit`, `cmo`, `asset_backed_facility`, registering
//! with `finstack_quant_valuations_structured_credit::STRUCTURED_CREDIT`) lives in
//! `finstack-quant-valuations-structured-credit` and is re-exported under the
//! same paths.

pub use finstack_quant_valuations_fixed_income::*;
pub use finstack_quant_valuations_structured_credit::{
    asset_backed_facility, cmo, structured_credit,
};

mod bond_json;
pub use bond_json::bond_from_cashflows_json;
