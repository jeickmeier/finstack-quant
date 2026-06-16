//! Scalar market data and time series helpers.
//!
//! The submodules expose lightweight types that complement the full
//! term-structure framework.  They cover single-value quotes (`MarketScalar`),
//! generic time series (`ScalarTimeSeries`), and inflation-specific indices.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::market_data::scalars::{MarketScalar, ScalarTimeSeries, SeriesInterpolation};
//! use finstack_quant_core::currency::Currency;
//! use time::macros::date;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! // 1. Store a spot quote as a scalar
//! let spot = MarketScalar::Price(finstack_quant_core::money::Money::new(101.5, Currency::USD));
//! if let MarketScalar::Price(m) = &spot {
//!     assert_eq!(m.currency(), Currency::USD);
//! }
//!
//! // 2. Build a small time series with linear interpolation
//! let ts = ScalarTimeSeries::new(
//!     "US CPI",
//!     vec![
//!         (date!(2024 - 01 - 31), 100.0),
//!         (date!(2024 - 02 - 29), 101.0),
//!     ],
//!     None,
//! )
//! ?
//! .with_interpolation(SeriesInterpolation::Linear);
//! let mid = date!(2024 - 02 - 14);
//! let value = ts.value_on(mid)?;
//! assert!(value > 100.0 && value < 101.0);
//! # Ok(())
//! # }
//! ```

/// Lightweight storage for time series data.
mod storage;

/// Generic market primitives: scalars and ad-hoc time series.
mod primitives;

/// Inflation index data (CPI/RPI) with lagging and seasonality support.
mod inflation_index;

// Re-export for ergonomic access (curated list)
pub use inflation_index::{
    InflationIndex, InflationIndexBuilder, InflationInterpolation, InflationLag,
};
pub use primitives::{MarketScalar, ScalarTimeSeries, SeriesInterpolation};
