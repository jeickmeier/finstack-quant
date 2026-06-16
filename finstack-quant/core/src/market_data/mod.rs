//! Market data infrastructure for financial instrument pricing.
//!
//! This module provides the core market data types and containers used by
//! pricing engines throughout Finstack. It includes yield curves, credit curves,
//! volatility surfaces, and the `MarketContext` that aggregates them for
//! valuation workflows.
//!
//! # Market Data Types
//!
//! ## Term Structures (1D Curves)
//!
//! - **Discount curves** (`DiscountCurve`): Risk-free rates for discounting
//! - **Forward curves** (`ForwardCurve`): Expected future interest rates
//! - **Hazard curves** (`HazardCurve`): Credit default intensities
//! - **Inflation curves** (`InflationCurve`): CPI expectations
//!
//! ## Surfaces (2D)
//!
//! - **Volatility surfaces** (`VolSurface`): Implied volatility by strike/maturity
//! - **FX delta vol surfaces** (`FxDeltaVolSurface`): FX smiles quoted in ATM /
//!   risk-reversal / butterfly form, converted to strikes for pricing
//!
//! ## Scalars and Time Series
//!
//! - **Market scalars** (`MarketScalar`): Spot prices, FX rates, indices
//! - **Inflation indices** (`InflationIndex`): CPI/RPI time series
//!
//! # Market Context
//!
//! `MarketContext` aggregates all market data needed for a valuation run:
//! - Stores curves by ID with type-safe retrieval
//! - Provides FX conversion via [`crate::money::fx::FxMatrix`]
//! - Supports scenario bumps and stress testing
//! - Thread-safe for parallel pricing
//!
//! # Industry Standards
//!
//! Market data handling follows conventions from:
//! - **ISDA** for day-count, business-day, and credit-market terminology
//! - **Andersen / Piterbarg** for modern term-structure construction context
//! - **Gatheral** and standard practitioner texts for volatility-surface
//!   interpretation
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::{context::MarketContext, term_structures::DiscountCurve};
//! use time::macros::date;
//!
//! let base = date!(2025 - 01 - 01);
//!
//! // Build a discount curve
//! let curve = DiscountCurve::builder("USD-OIS")
//!     .base_date(base)
//!     .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78)])
//!     .build()?;
//!
//! // Add to market context
//! let market = MarketContext::new().insert(curve);
//! assert!(market.get_discount("USD-OIS").is_ok());
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # References
//!
//! - Term structures and discounting:
//!   `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - Core derivatives and market-data context:
//!   `docs/REFERENCES.md#hull-options-futures`
//! - Volatility surfaces:
//!   `docs/REFERENCES.md#gatheral-volatility-surface`

/// Volatility surface arbitrage detection framework.
///
/// Model-free checks (butterfly, calendar spread, local vol density)
/// and SVI-specific checks (moment bounds, density, cross-slice).
pub mod arbitrage;
/// Bump functionality for scenario analysis and stress testing.
pub mod bumps;
/// Market data context with enum-based storage (simplified from V2).
pub mod context;
/// Market data comparison and shift measurement.
pub mod diff;
/// Shared dividend schedules (cash/yield/stock) for equities/ETFs.
pub mod dividends;
/// Dynamic term structure models (Diebold-Li, PCA).
pub mod dtsm;
/// Historical rate fixing lookup utilities.
///
/// Provides the canonical `FIXING:{curve_id}` convention and shared helpers
/// for seasoned instrument pricing.
pub mod fixings;
/// Market data hierarchy for organizational grouping and scenario targeting.
pub mod hierarchy;
/// Scalar market data types and time series (including primitives)
pub mod scalars;
/// Two-dimensional surfaces (e.g. volatility).
pub mod surfaces;
/// One-dimensional term structures (yield, credit, ...).
pub mod term_structures;
/// Traits for market data types (Discounting, Forward, Survival, etc.).
///
/// These traits define the interface for curve types used in pricing.
/// Useful for generic programming and custom curve implementations.
pub mod traits;
pub use context::MarketContext;
// Re-export dividend schedule types for convenience
pub use dividends::{DividendEvent, DividendKind, DividendSchedule, DividendScheduleBuilder};
pub use term_structures::DiscountCurve;
