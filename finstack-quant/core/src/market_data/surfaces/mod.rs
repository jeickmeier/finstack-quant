//! Two-dimensional market data surfaces.
//!
//! Provides 2D interpolation structures for market observables that vary by
//! two parameters (for example volatility by strike and maturity).
//!
//! # Surface Types
//!
//! - `VolSurface`: Observed implied volatility by strike and maturity
//! - `VolCube`: SABR parameter and forward nodes by expiry and tenor
//! - `FxDeltaVolSurface`: FX smile quotes in delta space; conversion and
//!   smile materialization live in `finstack-quant-models`
//!
//! # When to use which surface
//!
//! - Use [`crate::market_data::surfaces::VolSurface`] when market data is already quoted on a strike grid.
//! - Use [`crate::market_data::surfaces::FxDeltaVolSurface`] when FX options are quoted in ATM, risk-reversal,
//!   and butterfly form at standard deltas.
//!
//! # Conventions
//!
//! Surface expiries are expressed as year fractions. Equity-style surfaces are
//! typically indexed by strike, while FX smile inputs may begin in forward-delta
//! space before being mapped onto strikes.
//!
//! # Examples
//! ```rust
//! use finstack_quant_core::market_data::surfaces::VolSurface;
//! use finstack_quant_core::types::CurveId;
//! # fn main() -> finstack_quant_core::Result<()> {
//!
//! let surface = VolSurface::builder("EQ-FLAT")
//!     .expiries(&[1.0, 2.0])
//!     .strikes(&[90.0, 100.0])
//!     .row(&[0.2, 0.2])
//!     .row(&[0.2, 0.2])
//!     .build()
//!     ?;
//! assert_eq!(surface.id(), &CurveId::from("EQ-FLAT"));
//! # Ok(())
//! # }
//! ```
//!
//! # References
//!
//! - General volatility-surface conventions: `docs/REFERENCES.md#gatheral-volatility-surface`
//!
//! - FX volatility quoting: `docs/REFERENCES.md#clark-fx-options`,
//!   `docs/REFERENCES.md#wystup-fx-options`

pub mod fx_delta_vol_surface;
mod sabr_parameter_data;
mod vol_cube;
mod vol_surface;

pub use fx_delta_vol_surface::FxDeltaVolSurface;
pub use sabr_parameter_data::SabrParameterData;
pub use vol_cube::{VolCube, VolCubeBuilder};
pub use vol_surface::{
    VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis, VolSurfaceBuilder,
};
