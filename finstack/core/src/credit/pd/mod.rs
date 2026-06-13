//! PD calibration, term structures, and master scale mapping.
//!
//! This module provides utilities for working with probabilities of default:
//!
//! - [`calibration`]: PiT/TtC conversion using the Merton-Vasicek single-factor
//!   model and central tendency estimation from historical default rates.
//! - [`term_structure`]: Build monotonic cumulative PD curves from transition
//!   matrices, hazard curves, or explicit data points.
//! - [`master_scale`]: Map continuous PDs to discrete rating grades with
//!   configurable boundaries. Includes S&P and Moody's empirical presets.
//!
//! # Examples
//!
//! ```
//! use finstack_core::credit::pd::{PdCycleParams, ttc_to_pit, pit_to_ttc};
//!
//! let params = PdCycleParams {
//!     asset_correlation: 0.20,
//!     cycle_index: -1.5,
//! };
//!
//! // Downturn: PiT PD should be higher than TtC PD
//! let pd_pit = ttc_to_pit(0.02, &params).unwrap();
//! assert!(pd_pit > 0.02);
//! ```

pub mod calibration;
pub mod error;
pub mod master_scale;
pub mod term_structure;
#[cfg(test)]
mod tests;

// Re-exports
pub use calibration::{
    apply_basel_irb_pd_floor, central_tendency, pit_to_ttc, ttc_to_pit, PdCycleParams,
    BASEL_IRB_PD_FLOOR,
};
pub use error::PdCalibrationError;
pub use master_scale::{MasterScale, MasterScaleGrade, MasterScaleResult};
pub use term_structure::{PdTermStructure, PdTermStructureBuilder};
