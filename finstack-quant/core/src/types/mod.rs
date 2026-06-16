//! Core type definitions for the FinStack ecosystem.
//!
//! This module collects phantom-typed identifiers (`CurveId`, `InstrumentId`, …),
//! rate helpers (`Rate`, `Bps`), credit ratings, and convenient aliases used
//! throughout the platform.
//!
//! # Components
//!
//! - `id`: strongly typed identifiers used to avoid mixing unrelated IDs.
//! - `rates`: wrappers for decimal rates, percentages, and basis points.
//! - `ratings`: credit-rating enums and lookup helpers.
//! - `attributes`: lightweight attribute bags used by matching and metadata flows.
//!
//! # Conventions
//!
//! - [`crate::types::Rate`] stores decimal rates, so 5% is represented as `0.05`.
//! - [`crate::types::Bps`] stores basis points, so 25 bp is represented as `25.0`.
//! - [`crate::types::Percentage`] stores whole-percent values, so 25% is represented as `25.0`.
//! - Typed IDs preserve semantic meaning without changing the runtime string
//!   representation used in serialization or logs.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::types::{CurveId, Rate, Percentage};
//!
//! let curve_id = CurveId::from("USD-OIS");
//! let rate = Rate::from_percent(5.0);
//! let pct = Percentage::new(25.0);
//! assert_eq!(curve_id.as_str(), "USD-OIS");
//! assert_eq!(rate.as_decimal(), 0.05);
//! assert_eq!(pct.as_percent(), 25.0);
//! ```

mod attributes;
mod id;
mod rates;
mod ratings;

pub use attributes::Attributes;
pub use id::{
    CalendarId, CurveId, DealId, Id, IndexId, InstrumentId, IssuerId, PoolId, PriceId, UnderlyingId,
};
pub use rates::{Bps, Percentage, Rate};
pub use ratings::{moodys_warf_factor, CreditRating, RatingFactorTable, RatingLabel};
