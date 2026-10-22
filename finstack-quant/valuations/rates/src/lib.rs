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

//! Interest rate derivatives and money market instruments.
//!
//! This module provides interest rate instruments from simple money market
//! products to complex volatility derivatives. All instruments support
//! multi-curve pricing with separate discount and projection curves.
//!
//! # Features
//!
//! - **Swaps**: Vanilla IRS, basis swaps, cross-currency swaps
//! - **Options**: Caps, floors, swaptions, CMS options
//! - **Money Market**: Deposits, FRAs, repos
//! - **Futures**: SOFR futures, Eurodollar futures
//! - **Inflation**: Zero-coupon swaps, YoY swaps, inflation caps/floors
//! - **Exotics**: Bermudan swaptions (rate-linked notes live in the
//!   `finstack-quant-valuations-exotics` crate)
//!
//! # Pricing Framework
//!
//! Post-2008 multi-curve framework:
//! - **Discount curve**: OIS curve for collateralized discounting
//! - **Projection curves**: Term SOFR, EURIBOR, etc. for floating legs
//! - **Volatility surfaces**: Normal or lognormal vol for options
//!
//! # Quick Example
//!
//! ```
//! use finstack_quant_valuations_rates::irs::InterestRateSwap;
//! use finstack_quant_valuations_core::instruments::{FixedLegSpec, FloatLegSpec};
//! use finstack_quant_valuations_core::instruments::PayReceive;
//! use finstack_quant_cashflows::builder::FloatingLegCompounding;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::types::InstrumentId;
//! use rust_decimal_macros::dec;
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a 5-year USD payer swap (pay fixed, receive floating)
//! let swap = InterestRateSwap::builder()
//!     .id(InstrumentId::new("IRS-5Y-USD"))
//!     .notional(Money::from((10_000_000_i64, Currency::USD)))
//!     .side(PayReceive::Pay)
//!     .fixed_leg(FixedLegSpec {
//!         discount_curve_id: "USD-OIS".into(),
//!         rate: dec!(0.04),  // 4% fixed rate
//!         frequency: Tenor::semi_annual(),
//!         day_count: DayCount::Thirty360,
//!         business_day_convention: BusinessDayConvention::ModifiedFollowing,
//!         calendar_id: Some("usny".into()),
//!         stub: StubKind::None,
//!         start: date!(2025-01-15),
//!         end: date!(2030-01-15),
//!         end_of_month: false,
//!         payment_lag_days: 0,
//!     })
//!     .float_leg(FloatLegSpec {
//!         discount_curve_id: "USD-OIS".into(),
//!         forward_curve_id: "USD-SOFR-3M".into(),
//!         spread_bp: dec!(0.0),
//!         frequency: Tenor::quarterly(),
//!         day_count: DayCount::Act360,
//!         business_day_convention: BusinessDayConvention::ModifiedFollowing,
//!         calendar_id: Some("usny".into()),
//!         stub: StubKind::None,
//!         reset_lag_days: 0,
//!         fixing_calendar_id: None,
//!         start: date!(2025-01-15),
//!         end: date!(2030-01-15),
//!         end_of_month: false,
//!         compounding: FloatingLegCompounding::Simple,
//!         payment_lag_days: 0,
//!     })
//!     .build()?;
//! swap.validate()?;
//! # Ok(()) }
//! ```
//!
//! # Risk Metrics
//!
//! All rate instruments support:
//! - **DV01**: Dollar value of 1bp parallel curve shift
//! - **Bucketed DV01**: Sensitivity by tenor bucket
//! - **Convexity**: Second-order rate sensitivity
//! - **Theta**: Time decay
//!
//! # References
//!
//! - ISDA 2021 Definitions for current swap and RFR conventions, with ISDA
//!   2006 retained for legacy transactions `docs/REFERENCES.md#isda-2021-definitions`
//! - Black (1976) for cap/floor and swaption pricing `docs/REFERENCES.md#black-1976`
//! - Hull-White (1990) for short rate models `docs/REFERENCES.md#hull-white-1990-pricing-ird`
//!
//! # See Also
//!
//! - [`InterestRateSwap`](crate::irs::InterestRateSwap) for vanilla IRS
//! - [`Swaption`](crate::swaption::Swaption) for European swaptions
//! - [`CapFloor`](crate::cap_floor::CapFloor) for caps and floors
//! - the calibration crate for curve construction

/// Basis swap module - Floating vs floating swaps.
pub mod basis_swap;
/// Cap/floor module - Interest rate caps and floors.
pub mod cap_floor;
/// Reference-swap resolution shared by the CMS instruments.
pub(crate) mod cms_common;
/// Historical CMS (par swap rate) fixing lookups for seasoned CMS trades.
/// CMS option module - Constant maturity swap options.
pub mod cms_option;
/// CMS spread option module - Option on spread between two CMS rates.
pub mod cms_spread_option;
/// CMS swap module - Constant maturity swaps.
pub mod cms_swap;
/// Deposit module - Money market deposits.
pub mod deposit;
/// Forward swap rate and annuity helpers shared by the CMS instruments.
pub(crate) mod forward_swap_rate;
/// FRA module - Forward rate agreements.
pub mod fra;
/// Inflation cap/floor module.
pub mod inflation_cap_floor;
/// Finite-difference inflation-curve sensitivities shared by the inflation instruments.
pub(crate) mod inflation_fd;
/// Inflation swap module.
pub mod inflation_swap;
/// IR future module - Interest rate futures.
pub mod ir_future;
/// Exchange-listed options on interest-rate futures.
pub mod ir_future_option;
/// IRS module - Interest rate swaps.
pub mod irs;
/// Rate-magnitude checks shared by the rates instruments.
pub(crate) mod rate_validation;
/// Repo module - Repurchase agreements.
pub mod repo;
/// Swaption module - Options on interest rate swaps.
pub mod swaption;
/// Cross-currency swap module.
pub mod xccy_swap;

mod family;
pub use family::RATES;

// `irs::marginable`'s unit tests `include!` the integration-test helper
// `tests/support/rates.rs`, which names this crate from outside.
extern crate self as finstack_quant_valuations_rates;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
