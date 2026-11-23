//! Market data context for aggregating curves, surfaces, and FX rates.
//!
//! `MarketContext` is the primary container for market data used in valuations.
//! It aggregates discount curves, forward curves, hazard curves, volatility
//! surfaces, FX rates, and market scalars into a single, thread-safe structure.
//!
//! # Design
//!
//! - **Arc-based storage**: Cheap to clone and share across threads
//! - **Type-safe retrieval**: Separate methods for each curve type
//! - **Builder pattern**: Fluent API for constructing contexts
//! - **Scenario support**: Bump curves for risk sensitivities
//!
//! # API boundaries
//!
//! - **Public surface**: `new`, `insert_*`, typed getters (`get_discount`, `surface`,
//!   `price`, `series`, etc.), scenario helpers (`bump`, scratch bump tokens),
//!   stats (`stats`) and serde states (`CurveState`, `MarketContextState`).
//! - **Advanced plumbing**:
//!   [`CurveStorage`][crate::market_data::context::CurveStorage] is
//!   intentionally public because scenario adapters and snapshot serde use it
//!   across crate boundaries, but most callers should prefer the typed
//!   insert/getter methods.
//! - **Internal details**: map layout, caches, and rebind mechanics are not a
//!   stable API. Prefer the public methods above for all access and mutation.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::market_data::term_structures::DiscountCurve;
//! use finstack_quant_core::math::interp::InterpStyle;
//! use finstack_quant_core::types::CurveId;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let base_date = Date::from_calendar_date(2024, Month::January, 1).expect("Valid date");
//! let curve = DiscountCurve::builder("USD-OIS")
//!     .base_date(base_date)
//!     .day_count(finstack_quant_core::dates::DayCount::Act365F)
//!     .knots([(0.0, 1.0), (1.0, 0.98)])
//!     .interp(InterpStyle::Linear)
//!     .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
//!     .build()
//!     .expect("DiscountCurve builder should succeed");
//!
//! let ctx = MarketContext::new().try_insert(curve).expect("insert market curve");
//! let retrieved = ctx.get_discount("USD-OIS").expect("Discount curve should exist");
//! assert_eq!(retrieved.id(), &CurveId::from("USD-OIS"));
//! ```

mod curve_storage;
mod getters;
mod insert;
mod market_context;
mod ops_bump;
mod ops_roll;
mod state_serde;
mod stats;
#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "../../../tests/performance/context_bump_work.rs"]
mod context_bump_work;

pub use curve_storage::CurveStorage;
pub(crate) use curve_storage::for_each_context_curve;
use market_context::ScratchBumpState;
pub use market_context::{ContextScratchBump, ConvertedMoney, MarketContext};
pub use state_serde::{CreditIndexState, CurveState, MarketContextState};
pub use stats::ContextStats;
