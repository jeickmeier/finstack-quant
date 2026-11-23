//! Volatility surface arbitrage detection framework.
//!
//! Provides model-free arbitrage detection for any
//! [`VolSurface`](finstack_quant_core::market_data::surfaces::VolSurface) regardless of how
//! it was constructed (market quotes, SABR, SVI, etc.).
//!
//! # Architecture
//!
//! - **Types**: violation taxonomy ([`ArbitrageType`], [`ViolationLocation`]),
//!   severity model ([`ArbitrageSeverity`]) and report ([`ArbitrageViolation`],
//!   [`ArbitrageReport`])
//! - **Checks**: [`ButterflyCheck`], [`CalendarSpreadCheck`] and
//!   [`LocalVolDensityCheck`], each with a `check(&self, surface)` method
//! - **Orchestrator**
//!   ([`check_surface`]): Runs all
//!   enabled checks and aggregates results into an
//!   [`ArbitrageReport`]
//!
//! # Usage
//!
//! ```rust
//! use finstack_quant_core::market_data::surfaces::{
//!     VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
//! };
//! use finstack_quant_models::volatility::arbitrage::{check_surface, ArbitrageCheckConfig};
//!
//! let surface = VolSurface::builder("TEST")
//!     .expiries(&[0.5, 1.0, 2.0])
//!     .secondary_values(&[80.0, 90.0, 100.0, 110.0, 120.0])
//!     .secondary_axis(VolSurfaceAxis::Strike)
//!     .quote_type(VolQuoteType::BlackLognormal)
//!     .interpolation_mode(VolInterpolationMode::Vol)
//!     .row(&[0.30, 0.25, 0.20, 0.22, 0.28])
//!     .row(&[0.28, 0.24, 0.19, 0.21, 0.26])
//!     .row(&[0.26, 0.22, 0.18, 0.20, 0.24])
//!     .build()
//!     .expect("surface should build");
//!
//! let report = check_surface(&surface, &ArbitrageCheckConfig {
//!     forward_prices: Some(vec![100.0]),
//!     ..Default::default()
//! })?;
//!
//! if report.passed {
//!     println!("Surface is arbitrage-free");
//! } else {
//!     for v in &report.violations {
//!         println!("{}: {}", v.severity, v.description);
//!     }
//! }
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

mod checks;
mod engine;
#[cfg(test)]
mod tests;
mod types;

pub use checks::{ButterflyCheck, CalendarSpreadCheck, LocalVolDensityCheck};
pub use engine::{
    ArbitrageCheckConfig, DEFAULT_ARBITRAGE_TOLERANCE, check_butterfly_grid,
    check_calendar_spread_grid, check_local_vol_density_grid, check_surface, check_surface_grid,
};
pub use types::{
    ArbitrageReport, ArbitrageSeverity, ArbitrageType, ArbitrageViolation, ViolationLocation,
};
