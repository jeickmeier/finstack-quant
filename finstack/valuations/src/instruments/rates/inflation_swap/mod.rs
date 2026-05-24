//! Zero-coupon inflation swap instruments for inflation exposure.
//!
//! Zero-coupon inflation swaps exchange a fixed rate (breakeven inflation)
//! for the realized cumulative inflation over the swap's life. They are
//! fundamental instruments for calibrating real (inflation-adjusted) curves.
//!
//! # Structure
//!
//! - **Fixed leg**: Pay or receive fixed breakeven rate (compounded)
//! - **Inflation leg**: Pay or receive cumulative CPI growth
//! - **No interim payments**: Single payment at maturity
//!
//! # Payoff at Maturity
//!
//! ```text
//! Inflation leg = Notional × [CPI(T)/CPI(0) - 1]
//! Fixed leg     = Notional × [(1 + fixed_rate)^τ - 1]
//! ```
//!
//! where τ is the year fraction from start to maturity using the instrument's day count.
//!
//! Net payment = Inflation leg - Fixed leg (for Pay side)
//!
//! # Breakeven Inflation Rate (Par Rate)
//!
//! The fixed rate K that makes PV = 0:
//!
//! ```text
//! (1 + K)^τ = CPI(T) / CPI(0)
//! K = [CPI(T) / CPI(0)]^(1/τ) - 1
//! ```
//!
//! This is the annualized compound inflation rate implied by the inflation curve.
//!
//! # Inflation Indexation
//!
//! Standard conventions:
//! - **Index**: CPI-U (US), HICP ex-tobacco (EU), RPI or CPI (UK)
//! - **Lag**: Typically 3 months
//! - **Base CPI**: Inflation index at swap start (with lag)
//!
//! # Market Conventions
//!
//! - **USD**: CPI-U, 3-month lag, ACT/ACT
//! - **EUR**: HICP ex-tobacco, 3-month lag, ACT/ACT
//! - **GBP**: RPI or CPI, 3-month lag, ACT/ACT
//!
//! # Calibration Role
//!
//! Zero-coupon inflation swaps calibrate real discount curves:
//! - 1Y to 30Y maturities typically quoted
//! - Real curve = Nominal curve / Inflation curve
//! - Used to price inflation-linked bonds
//!
//! # See Also
//!
//! - [`InflationSwap`] for instrument struct
//! - fixed income inflation-linked bond module for linkers
//! - Plan-driven calibration in `calibration::api` (Inflation step)

pub(crate) mod metrics;
mod types;

pub use types::{InflationSwap, InflationSwapBuilder, YoYInflationSwap, YoYInflationSwapBuilder};
