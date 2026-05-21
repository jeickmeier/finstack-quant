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
//! - **Exotics**: Range accruals, Bermudan swaptions
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
//! ```ignore
//! use finstack_valuations::instruments::rates::InterestRateSwap;
//! use finstack_valuations::instruments::{FixedLegSpec, FloatLegSpec};
//! use finstack_valuations::instruments::rates::irs::{FloatingLegCompounding, PayReceive};
//! use finstack_core::currency::Currency;
//! use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
//! use finstack_core::money::Money;
//! use finstack_core::types::InstrumentId;
//! use rust_decimal_macros::dec;
//! use time::macros::date;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a 5-year USD payer swap (pay fixed, receive floating)
//! let swap = InterestRateSwap::builder()
//!     .id(InstrumentId::new("IRS-5Y-USD"))
//!     .notional(Money::new(10_000_000.0, Currency::USD))
//!     .side(PayReceive::PayFixed)
//!     .fixed(FixedLegSpec {
//!         discount_curve_id: "USD-OIS".into(),
//!         rate: dec!(0.04),  // 4% fixed rate
//!         frequency: Tenor::semi_annual(),
//!         day_count: DayCount::Thirty360,
//!         bdc: BusinessDayConvention::ModifiedFollowing,
//!         calendar_id: Some("usny".to_string()),
//!         stub: StubKind::None,
//!         start: date!(2025-01-15),
//!         end: date!(2030-01-15),
//!         end_of_month: false,
//!         par_method: None,
//!         compounding_simple: true,
//!         payment_lag_days: 0,
//!     })
//!     .float(FloatLegSpec {
//!         discount_curve_id: "USD-OIS".into(),
//!         forward_curve_id: "USD-SOFR-3M".into(),
//!         spread_bp: dec!(0.0),
//!         frequency: Tenor::quarterly(),
//!         day_count: DayCount::Act360,
//!         bdc: BusinessDayConvention::ModifiedFollowing,
//!         calendar_id: Some("usny".to_string()),
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
//! - ISDA 2006 Definitions for swap conventions
//! - Black (1976) for cap/floor and swaption pricing
//! - Hull-White (1990) for short rate models
//!
//! # See Also
//!
//! - [`InterestRateSwap`] for vanilla IRS
//! - [`Swaption`] for European swaptions
//! - [`CapFloor`] for caps and floors
//! - [`crate::calibration`] for curve calibration

/// Basis swap module - Floating vs floating swaps.
pub mod basis_swap;
/// Callable range accrual module - Range accrual with Bermudan call.
pub mod callable_range_accrual;
/// Cap/floor module - Interest rate caps and floors.
pub mod cap_floor;
/// CMS option module - Constant maturity swap options.
pub mod cms_option;
/// CMS spread option module - Option on spread between two CMS rates.
pub mod cms_spread_option;
/// CMS swap module - Constant maturity swaps.
pub mod cms_swap;
/// Deposit module - Money market deposits.
pub mod deposit;
/// Shared rates pricing utilities.
pub mod exotics_shared;
/// FRA module - Forward rate agreements.
pub mod fra;
/// Inflation cap/floor module.
pub mod inflation_cap_floor;
/// Inflation swap module.
pub mod inflation_swap;
/// IR future module - Interest rate futures.
pub mod ir_future;
/// IR future option module - Options on interest rate futures.
pub mod ir_future_option;
/// IRS module - Interest rate swaps.
pub mod irs;
/// Range accrual module.
pub mod range_accrual;
/// Repo module - Repurchase agreements.
pub mod repo;
/// Snowball / Inverse Floater structured note module.
pub mod snowball;
/// Swaption module - Options on interest rate swaps.
pub mod swaption;
/// TARN (Target Redemption Note) module.
pub mod tarn;
/// Cross-currency swap module.
pub mod xccy_swap;

// Re-export primary types
pub use basis_swap::BasisSwap;
pub use callable_range_accrual::CallableRangeAccrual;
pub use cap_floor::{CapFloor, InterestRateOption, RateOptionType};
pub use cms_option::CmsOption;
pub use cms_spread_option::{CmsSpreadOption, CmsSpreadOptionType};
pub use cms_swap::CmsSwap;
pub use deposit::{ConventionDepositParams, Deposit};
pub use fra::{ConventionFraParams, ForwardRateAgreement};
pub use inflation_cap_floor::{InflationCapFloor, InflationCapFloorType};
pub use inflation_swap::{InflationSwap, YoYInflationSwap};
pub use ir_future::InterestRateFuture;
pub use ir_future_option::IrFutureOption;
pub use irs::InterestRateSwap;
pub use range_accrual::RangeAccrual;
pub use repo::{CollateralSpec, CollateralType, Repo, RepoType};
pub use snowball::{Snowball, SnowballVariant};
pub use swaption::{BermudanSwaption, Swaption};
pub use tarn::Tarn;
pub use xccy_swap::XccySwap;
