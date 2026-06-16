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
//! The forward curve f(t₁, t₂) represents the rate agreed today for borrowing
//! from t₁ to t₂:
//! ```text
//! f(t₁, t₂) = [DF(t₁) / DF(t₂) - 1] / (t₂ - t₁)
//! ```
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
//! | `ForwardCurve`                | Rates   | `Forward`         |
//! | `HazardCurve`                 | Credit  | `Survival`        |
//! | `BaseCorrelationCurve`        | Credit  | (none)            |
//! | `InflationCurve`              | CPI     | `TermStructure`   |
//!
//! ## Choosing an interpolation style
//! All curves are bootstrapped from knot points and allow selecting an
//! [`crate::math::interp::InterpStyle`] via a builder method such as
//! `interp(...)`.
//!
//! # Conventions
//!
//! - Time is expressed as a year fraction from a base date.
//! - Rate-like curves should document whether stored values are discount
//!   factors, simple forward rates, hazard intensities, or CPI-derived levels.
//! - Builder validation is part of the public contract; prefer calling
//!   `build()` rather than bypassing validation paths.
//!
//! ## Example – assembling curves inside a `MarketContext`
//! ```rust
//! use finstack_quant_core::market_data::term_structures::{
//!     DiscountCurve, ForwardCurve, HazardCurve,
//! };
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_core::math::interp::InterpStyle;
//! use time::macros::date;
//!
//! let base = date!(2025 - 01 - 01);
//! let disc = DiscountCurve::builder("USD-OIS")
//!     .base_date(base)
//!     .knots([(0.0, 1.0), (5.0, 0.88)])
//!     .interp(InterpStyle::MonotoneConvex)
//!     .build()
//!     ?;
//! let fwd3m = ForwardCurve::builder("USD-SOFR3M", 0.25)
//!     .base_date(base)
//!     .knots([(0.0, 0.03), (5.0, 0.04)])
//!     .interp(InterpStyle::Linear)
//!     .build()
//!     ?;
//! let hazard = HazardCurve::builder("USD-CRED")
//!     .base_date(base)
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
//! - Discounting and term-structure context:
//!   `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - Curve interpolation:
//!   `docs/REFERENCES.md#hagan-west-monotone-convex`

/// Base correlation curves for CDS tranche pricing.
mod base_correlation;
/// Basis spread curves for cross-currency frameworks.
mod basis_spread_curve;
/// Internal shared helpers for 1D curves (not exported publicly).
pub(crate) mod common;
/// Credit index aggregates for CDS tranche pricing and credit derivatives.
mod credit_index;
/// Discount factor curves.
mod discount_curve;
/// Flat forward/discount curves.
mod flat;
/// Forward‐rate curves.
mod forward_curve;
/// Forward variance curves for rough volatility models.
pub mod forward_variance;
/// Credit hazard curves.
mod hazard_curve;
/// Real/Breakeven inflation curves.
mod inflation;
/// Nelson-Siegel and Nelson-Siegel-Svensson parametric yield curves.
mod parametric_curve;
/// Forward price curves (commodities, indices).
mod price_curve;
/// Volatility index forward curves (VIX, VXN, VSTOXX).
mod vol_index_curve;

// Re-export for ergonomic access (curated list)
pub use base_correlation::{
    ArbitrageCheckResult, ArbitrageViolation, BaseCorrelationCurve, BaseCorrelationCurveBuilder,
    SmoothingMethod, BASE_CORR_DETACHMENT_MATCH_TOLERANCE,
};
pub use basis_spread_curve::{BasisSpreadCurve, BasisSpreadCurveBuilder};
pub use credit_index::{CreditIndexData, CreditIndexDataBuilder};
pub use discount_curve::{
    DiscountCurve, DiscountCurveBuilder, DiscountCurveRateCalibration, DiscountCurveRateQuote,
    DiscountCurveRateQuoteType, ValidationMode, DEFAULT_MIN_FORWARD_TENOR,
};
pub use flat::FlatCurve;
pub use forward_curve::{
    ForwardCurve, ForwardCurveBuilder, ForwardCurveRateCalibration, ForwardCurveRateQuote,
};
pub use forward_variance::ForwardVarianceCurve;
pub use hazard_curve::{HazardCurve, HazardCurveBuilder, ParInterp, Seniority};
pub use inflation::{InflationCurve, InflationCurveBuilder};
pub use parametric_curve::{NelsonSiegelModel, NsVariant, ParametricCurve, ParametricCurveBuilder};
pub use price_curve::{PriceCurve, PriceCurveBuilder};
pub use vol_index_curve::{VolatilityIndexCurve, VolatilityIndexCurveBuilder};
