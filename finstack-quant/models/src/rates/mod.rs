//! Product-independent interest-rate models.
//!
//! - [`hull_white`]: HW1F closed-form kernels and calibration parameters.
//! - [`hw1f`]: HW1F Monte Carlo and Longstaff-Schwartz engines.
//! - [`dtsm`]: Dynamic term-structure models.
//! - [`clock`]: Model-time conventions.

pub mod clock;
pub mod dtsm;
pub mod hull_white;
pub mod hw1f;
