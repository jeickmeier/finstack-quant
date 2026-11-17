//! Term structure curves for rates, credit, and inflation.
//!
//! This module implements one-dimensional term structures that map time to
//! market observables. These curves are fundamental building blocks for pricing
//! fixed income securities and derivatives.
//!
//! # What is a Term Structure?
//!
//! A term structure maps a time coordinate (in years from a base date) to a
//! numerical value representing market expectations:
//! - **Discount factors**: Present value of $1 at time t
//! - **Forward rates**: Expected future interest rates
//! - **Hazard rates**: Credit event intensity (default probability)
//! - **Survival probabilities**: Probability of no default by time t
//! - **Inflation expectations**: Expected CPI growth
//!
//! # Financial Concepts
//!
//! ## Discount Curve
//!
//! The discount curve DF(t) represents the present value of $1 received at time t:
//! ```text
//! DF(t) = e^(-r(t) * t)
//!
//! where r(t) is the continuously compounded zero rate
//! ```
//!
//! ## Forward Curve
//!
//! [`DiscountCurve::forward`](crate::market_data::term_structures::DiscountCurve::forward) returns the **continuously compounded** interval
//! forward:
//! ```text
//! f(t₁, t₂) = -ln(DF(t₂) / DF(t₁)) / (t₂ - t₁)
//! ```
//!
//! The simple money-market forward `(P(t₁)/P(t₂) − 1) / alpha` lives on
//! [`crate::market_data::term_structures::ForwardCurve::rate_between`], not on
//! the discount curve.
//!
//! ## Hazard Curve
//!
//! The hazard rate λ(t) represents the instantaneous probability of default:
//! ```text
//! Survival(t) = e^(-∫₀ᵗ λ(s)ds)
//! ```
//!
//! # Curve Types
//!
//! | Struct                        | Domain  | Specialised trait |
//! |-------------------------------|---------|-------------------|
//! | `DiscountCurve`               | Rates   | `Discounting`     |
//! | `ForwardCurve`                | Rates   | (none)            |
//! | `HazardCurve`                 | Credit  | `Survival`        |
//! | `BaseCorrelationCurve`        | Credit  | (none)            |
//! | `InflationCurve`              | CPI     | (none)            |
//!
//! ## Choosing an interpolation style
//! All curves are bootstrapped from knot points and allow selecting an
//! [`crate::math::interp::InterpStyle`] via a builder method such as
//! `interp(...)`.
//!
//! # Conventions
//!
//! - Time is expressed as a year fraction from a base date.
//! - Rate-like curves document whether stored values are discount
//!   factors, projection factors, hazard intensities, or CPI-derived levels.
//! - Builder validation is part of the public contract; prefer calling
//!   `build()` rather than bypassing validation paths.
//!
//! ## Example – assembling curves inside a `MarketContext`
//! ```rust
//! use finstack_quant_core::dates::{DayCount, Tenor};
//! use finstack_quant_core::market_data::term_structures::{
//!     DiscountCurve, ForwardCurve, HazardCurve,
//! };
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
//! use time::macros::date;
//!
//! let base = date!(2025 - 01 - 01);
//! let disc = DiscountCurve::builder("USD-OIS")
//!     .base_date(base)
//!     .day_count(DayCount::Act365F)
//!     .knots([(0.0, 1.0), (5.0, 0.88)])
//!     .interp(InterpStyle::MonotoneConvex)
//!     .extrapolation(ExtrapolationPolicy::Extend)
//!     .build()
//!     ?;
//! let fwd3m = ForwardCurve::builder("USD-SOFR3M")
//!     .base_date(base)
//!     .day_count(DayCount::Act360)
//!     .index_tenor(Tenor::quarterly())
//!     .knots([(0.0, 1.0), (5.0, 0.85)])
//!     .interp(InterpStyle::LogLinear)
//!     .extrapolation(ExtrapolationPolicy::Extend)
//!     .build()
//!     ?;
//! let hazard = HazardCurve::builder("USD-CRED")
//!     .base_date(base)
//!     .day_count(finstack_quant_core::dates::DayCount::Act365F)
//!     .recovery_rate(0.40)
//!     .knots([(1.0, 0.01), (10.0, 0.015)])
//!     .build()
//!     ?;
//!
//! let curves = MarketContext::new()
//!     .insert(disc)
//!     .insert(fwd3m)
//!     .insert(hazard);
//! assert!(curves.get_discount("USD-OIS").is_ok());
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # References
//!
//! - Discounting and term-structure context: `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//!
//! - Curve interpolation: `docs/REFERENCES.md#hagan-west-monotone-convex`
//!

/// Base correlation curves for CDS tranche pricing.
mod base_correlation;
/// Internal shared helpers for 1D curves (not exported publicly).
pub(crate) mod common;
/// Credit index aggregates for CDS tranche pricing and credit derivatives.
mod credit_index;
/// Discount factor curves.
mod discount_curve;
/// Forward‐rate curves.
mod forward_curve;
/// Forward variance curves for rough volatility models.
mod forward_variance;
/// Serializable hazard-curve calibration replay inputs.
mod hazard_calibration;
/// Credit hazard curves.
mod hazard_curve;
/// Real/Breakeven inflation curves.
mod inflation;
/// Nelson-Siegel and Nelson-Siegel-Svensson parametric yield curves.
mod parametric_curve;
/// Forward price curves (commodities, indices, volatility indices).
mod price_curve;
/// Serializable rate-curve calibration replay conventions.
mod rate_calibration;

pub use base_correlation::{
    BaseCorrelationCurve, BaseCorrelationCurveBuilder, CorrelationShapeReport,
    CorrelationShapeViolation,
};
pub use credit_index::{CreditIndexData, CreditIndexDataBuilder};
pub use discount_curve::{DiscountCurve, DiscountCurveBuilder};
pub use forward_curve::{ForwardCurve, ForwardCurveBuilder};
pub use forward_variance::ForwardVarianceCurve;
pub use hazard_calibration::{HazardCalibrationInput, HazardCalibrationRecipe};
pub use hazard_curve::{HazardCurve, HazardCurveBuilder, Seniority};
pub use inflation::{InflationCurve, InflationCurveBuilder};
pub use parametric_curve::{NelsonSiegelModel, NsVariant, ParametricCurve, ParametricCurveBuilder};
pub use price_curve::{PriceCurve, PriceCurveBuilder, PriceCurveKind};
pub use rate_calibration::{
    RateCalibrationCurveRole, RateCalibrationFutureContractId, RateCalibrationMethod,
    RateCalibrationOisCompounding, RateCalibrationPillar, RateCalibrationQuote,
    RateCalibrationRecipe,
};
