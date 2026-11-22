//! Levered real estate equity instrument.
//!
//! Composes an unlevered [`finstack_quant_valuations_equity::real_estate::RealEstateAsset`]
//! with bond, term-loan, revolving-credit, or repo financing legs from other
//! families.

mod levered;
mod levered_pricer;
/// Levered real estate metric calculators.
pub(crate) mod metrics;

pub use levered::{LeveredRealEstateEquity, LeveredRealEstateEquityBuilder, RealEstateFinancing};
