//! Forward rate curves for simple term floating-rate indices.
//!
//! A forward curve represents expected future simple rates for a specific
//! tenor-index projection (e.g., 3-month SOFR term, 6-month EURIBOR). These
//! curves are essential for pricing floating-rate instruments and calculating
//! forward-looking cash flows in swaps and floating-rate notes.
//!
//! # Financial Concept
//!
//! The forward rate f(t₁, t₂) is the rate agreed today for borrowing/lending
//! from time t₁ to t₂:
//! ```text
//! f(t₁, t₂) = [DF(t₁) / DF(t₂) - 1] / (t₂ - t₁)
//!
//! For a fixed-tenor index (e.g., 3M):
//! f(t) = forward rate resetting at time t for the index tenor
//! ```
//!
//! # Market Construction
//!
//! Forward curves are typically bootstrapped from:
//! - **Futures**: SOFR futures, Eurodollar futures (liquid up to ~5 years)
//! - **FRA** (Forward Rate Agreements): OTC quotes for forward rates
//! - **Swaps**: Float leg expectations from swap rates
//! - **Basis spreads**: Tenor basis between different index tenors
//!
//! # Index Conventions
//!
//! This type stores simple tenor forwards plus day-count/reset-lag metadata.
//! It does **not** model overnight compounded-in-arrears fixings, observation
//! shifts, or lookbacks. Use it for term indices or already-compounded term
//! projections. Overnight RFR instruments need a separate compounding model.
//!
//! # Use Cases
//!
//! - **Floating-rate note pricing**: Project future coupon payments
//! - **Interest rate swap valuation**: Mark-to-market floating leg
//! - **Cap/floor pricing**: Forward rates determine intrinsic value
//! - **Basis swap pricing**: Spread between different index tenors
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::term_structures::ForwardCurve;
//! use finstack_quant_core::math::interp::InterpStyle;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let fc = ForwardCurve::builder("USD-SOFR3M", 0.25)
//!     .base_date(base)
//!     .knots([(0.0, 0.03), (5.0, 0.04)])
//!     .interp(InterpStyle::Linear)
//!     .build()
//!     .expect("ForwardCurve builder should succeed");
//! assert!(fc.rate(1.0) > 0.0);
//! ```
//!
//! # References
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Chapters 4-6 (Forward rates and curve construction). `docs/REFERENCES.md#hull-options-futures`
//! - Andersen, L., & Piterbarg, V. (2010). *Interest Rate Modeling*.
//!   Volume 1, Chapter 3 (Multi-curve framework). `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
//! - Ametrano, F. M., & Bianchetti, M. (2013). "Everything You Always Wanted to
//!   Know About Multiple Interest Rate Curve Bootstrapping but Were Afraid to Ask."
//!   SSRN Working Paper. `docs/REFERENCES.md#ametrano-bianchetti-2013`

use crate::market_data::term_structures::common::{
    build_interp_allow_any_values, infer_forward_curve_defaults, roll_knots, split_points,
};
use crate::math::interp::{ExtrapolationPolicy, InterpStyle};
use crate::{
    dates::{Date, DayCount, DayCountContext},
    error::InputError,
    math::integration::simpson_rule,
    math::interp::types::Interp,
    types::CurveId,
};

/// Forward rate curve for a simple floating-rate index with fixed tenor.
///
/// Represents expected future simple rates for a specific tenor-index projection
/// (e.g., 3-month SOFR term, 6-month EURIBOR). Stores simple forward rates at
/// knot times and interpolates between them.
///
/// # Index Components
///
/// - **Tenor**: Index accrual period (e.g., 0.25 years = 3 months)
/// - **Reset lag**: Days from fixing date to effective date
/// - **Day count**: Convention for accrual (usually Act/360 or Act/365F)
///
/// # Thread Safety
///
/// Immutable after construction; safe to share via `Arc<ForwardCurve>`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawForwardCurve", into = "RawForwardCurve")]
pub struct ForwardCurve {
    id: CurveId,
    base: Date,
    /// Business days from fixing to spot using positive T-minus semantics.
    reset_lag: i32,
    /// Day-count basis used for accrual.
    day_count: DayCount,
    /// Index tenor in **years** (0.25 = 3M).
    tenor: f64,
    /// Knot times in **years** (strictly increasing, first may be 0.0).
    knots: Box<[f64]>,
    /// Simple forward rates (e.g. 0.025 = 2.5 %).
    forwards: Box<[f64]>,
    /// Optional contractual reset/end-date boundaries, separate from interpolation knots.
    projection_grid: Option<Box<[f64]>>,
    interp: Interp,
    /// Exact typed recipe used to replay calibration after quote shocks.
    rate_calibration: Option<crate::market_data::term_structures::RateCalibrationRecipe>,
    /// Opaque FX policy stamp; see [`DiscountCurve::fx_policy`].
    fx_policy: Option<String>,
}

mod builder;
mod curve;
#[cfg(test)]
mod tests;
mod transform;
mod wire;
pub use builder::ForwardCurveBuilder;
use wire::RawForwardCurve;
