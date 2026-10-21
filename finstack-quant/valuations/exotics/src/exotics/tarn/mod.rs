//! Target Redemption Note (TARN) - path-dependent rate exotic.
//!
//! TARNs pay periodic coupons linked to a floating rate until the
//! cumulative coupon reaches a target level. At that point the note
//! redeems at par. The path-dependency from the cumulative coupon
//! knockout precludes closed-form pricing; Monte Carlo simulation
//! under Hull-White 1F is the standard approach.
//!
//! # Structure
//!
//! - **Coupon**: max(fixed_rate - floating_rate, floor)
//! - **Knockout**: When cumulative coupon reaches target, redeem at par
//! - **Final coupon**: Reduced to hit the target exactly
//!
//! # See Also
//!
//! - [`Tarn`](crate::exotics::tarn::Tarn) for instrument definition
//! - [`finstack_quant_models::rates::hw1f::CumulativeCouponTracker`], the
//!   cumulative coupon tracker used during MC simulation

pub(crate) mod metrics;
pub(crate) mod pricer;
pub(crate) mod types;

pub use pricer::TarnPricer;
pub use types::Tarn;
