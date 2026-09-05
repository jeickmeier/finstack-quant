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
//! - **Exotics**: Bermudan swaptions (rate-linked notes live in
//!   [`crate::instruments::exotics`])
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
//! use finstack_quant_valuations::instruments::rates::InterestRateSwap;
//! use finstack_quant_valuations::instruments::{FixedLegSpec, FloatLegSpec};
//! use finstack_quant_valuations::instruments::rates::irs::{FloatingLegCompounding, PayReceive};
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
//!     .fixed(FixedLegSpec {
//!         discount_curve_id: "USD-OIS".into(),
//!         rate: dec!(0.04),  // 4% fixed rate
//!         frequency: Tenor::semi_annual(),
//!         day_count: DayCount::Thirty360,
//!         business_day_convention: BusinessDayConvention::ModifiedFollowing,
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
//!         business_day_convention: BusinessDayConvention::ModifiedFollowing,
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
//! - ISDA 2021 Definitions for current swap and RFR conventions, with ISDA
//!   2006 retained for legacy transactions `docs/REFERENCES.md#isda-2021-definitions`
//! - Black (1976) for cap/floor and swaption pricing `docs/REFERENCES.md#black-1976`
//! - Hull-White (1990) for short rate models `docs/REFERENCES.md#hull-white-1990-pricing-ird`
//!
//! # See Also
//!
//! - [`InterestRateSwap`] for vanilla IRS
//! - [`Swaption`] for European swaptions
//! - [`CapFloor`] for caps and floors
//! - the calibration crate for curve construction

/// Basis swap module - Floating vs floating swaps.
pub mod basis_swap;
/// Cap/floor module - Interest rate caps and floors.
pub mod cap_floor;
/// Reference-swap resolution shared by the CMS instruments.
pub mod cms_common;
/// CMS option module - Constant maturity swap options.
pub mod cms_option;
/// CMS spread option module - Option on spread between two CMS rates.
pub mod cms_spread_option;
/// CMS swap module - Constant maturity swaps.
pub mod cms_swap;
/// Deposit module - Money market deposits.
pub mod deposit;
/// FRA module - Forward rate agreements.
pub mod fra;
/// Hull-White one-factor Monte Carlo / LSMC pricing infrastructure.
pub mod hw1f;
/// Inflation cap/floor module.
pub mod inflation_cap_floor;
/// Inflation swap module.
pub mod inflation_swap;
/// IR future module - Interest rate futures.
pub mod ir_future;
/// Exchange-listed options on interest-rate futures.
pub mod ir_future_option;
/// IRS module - Interest rate swaps.
pub mod irs;
/// Repo module - Repurchase agreements.
pub mod repo;
/// Swaption module - Options on interest rate swaps.
pub mod swaption;
/// Cross-currency swap module.
pub mod xccy_swap;

pub use basis_swap::BasisSwap;
pub use cap_floor::{CapFloor, RateOptionType};
pub use cms_option::CmsOption;
pub use cms_spread_option::{CmsSpreadOption, CmsSpreadOptionType};
pub use cms_swap::CmsSwap;
pub use deposit::{ConventionDepositParams, Deposit};
pub use fra::{ConventionFraParams, ForwardRateAgreement};
pub use inflation_cap_floor::{InflationCapFloor, InflationCapFloorType};
pub use inflation_swap::{InflationSwap, YoYInflationSwap};
pub use ir_future::{FutureContractSpecs, InterestRateFuture, RateAveragingMethod};
pub use ir_future_option::InterestRateFutureOption;
pub use irs::InterestRateSwap;
pub use repo::{CollateralSpec, CollateralType, Repo, RepoType};
pub use swaption::{BermudanSwaption, Swaption};
pub use xccy_swap::XccySwap;
