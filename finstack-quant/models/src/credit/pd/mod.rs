//! PD calibration, term structures, and master scale mapping.
//!
//! This module provides utilities for working with probabilities of default:
//!
//! - `calibration`: PiT/TtC conversion using the Merton-Vasicek single-factor
//!   model and central tendency estimation from historical default rates.
//! - `master_scale`: Map continuous PDs to discrete rating grades with
//!   configurable boundaries. Includes versioned library assumptions using
//!   S&P-style and Moody's-style labels; these are not agency calibrations.
//!
//! # Examples
//!
//! ```
//! use finstack_quant_models::credit::pd::{PdCycleParams, ttc_to_pit, pit_to_ttc};
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

pub(crate) mod calibration;
pub(crate) mod error;
pub(crate) mod master_scale;
#[cfg(test)]
mod tests;

pub use calibration::{
    apply_basel_irb_pd_floor, central_tendency, pit_to_ttc, ttc_to_pit, PdCycleParams,
    BASEL_IRB_PD_FLOOR,
};
pub use error::PdCalibrationError;
pub use master_scale::{MasterScale, MasterScaleGrade, MasterScaleResult};
