#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
// Allow expect() in doc tests (they are test code)
#![doc(test(attr(allow(clippy::expect_used))))]

//! Exotic options, structured rate notes and commodity derivatives.
//!
//! Two instrument families share this crate and one registration unit,
//! [`EXOTICS`]:
//!
//! - [`exotics`]: path-dependent options (Asian, barrier, lookback), range
//!   accruals and the Hull-White structured notes (TARN, snowball, callable
//!   range accrual).
//! - [`commodity`]: forwards, futures, swaps and the commodity option family
//!   (spot/future options, Asians, spread options, swaptions), which borrow
//!   the Asian averaging engine of the exotics.
//!
//! The multi-asset `Basket` holds legs of any family and therefore lives in
//! the umbrella crate `finstack-quant-valuations`, which re-exports this crate
//! as `finstack_quant_valuations::instruments::{exotics, commodity}`.
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_quant_valuations_core::instruments::Instrument;
//! use finstack_quant_valuations_exotics::commodity::commodity_forward::CommodityForward;
//! use finstack_quant_valuations_exotics::exotics::asian_option::AsianOption;
//!
//! let asian = AsianOption::example().unwrap();
//! let forward = CommodityForward::example().unwrap();
//! assert_ne!(asian.id(), forward.id());
//! ```

/// Commodity derivatives.
pub mod commodity;
/// Exotic and path-dependent options.
pub mod exotics;

mod family;
pub use family::EXOTICS;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
