//! Snowball / Inverse Floater structured notes.
//!
//! Snowball notes have path-dependent coupons where each period's coupon
//! depends on the previous period's coupon level. Inverse floaters are
//! a simpler variant where the coupon is a direct function of the floating
//! rate without path dependency.
//!
//! # Variants
//!
//! - **Snowball**: c_i = max(c_{i-1} + fixed - floating, 0)
//! - **Inverse Floater**: c_i = max(fixed - leverage * floating, 0)
//!
//! # See Also
//!
//! - [`Snowball`] for instrument definition
//! - [`SnowballVariant`] for variant selection

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod types;

pub use pricer::{SnowballDiscountingPricer, SnowballHw1fMcPricer};
pub use types::{Snowball, SnowballVariant};
