//! Scalar market data and time series helpers.
//!
//! The submodules expose lightweight types that complement the full
//! term-structure framework.  They cover single-value quotes (`MarketScalar`),
//! generic time series (`ScalarTimeSeries`), and inflation-specific indices.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::market_data::scalars::{
//!     MarketScalar, ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
//! };
//! use finstack_quant_core::currency::Currency;
//! use time::macros::date;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! // 1. Store a spot quote as a scalar
//! let spot = MarketScalar::price(finstack_quant_core::money::Money::new(101.5, Currency::USD).expect("valid money fixture"));
//! if let Some(m) = spot.get_price() {
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
//!     SeriesInterpolation::Linear,
//!     SeriesExtrapolation::Error,
//! )?;
//! let mid = date!(2024 - 02 - 14);
//! let value = ts.get_value(mid)?;
//! assert!(value > 100.0 && value < 101.0);
//! # Ok(())
//! # }
//! ```

/// Generic market primitives: scalars and ad-hoc time series.
mod primitives;

/// Monthly CPI/RPI observation snapshots with explicit release availability.
mod inflation_index;

pub use inflation_index::{
    InflationExpectedRelease, InflationIndex, InflationLag, InflationObservation, InflationSeries,
};
pub use primitives::{MarketScalar, ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation};
