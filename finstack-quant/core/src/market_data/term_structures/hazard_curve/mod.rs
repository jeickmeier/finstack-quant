//! Credit hazard rate curves for default probability modeling.
//!
//! A hazard curve represents the instantaneous probability of default (credit
//! event) for a corporate or sovereign issuer. These curves are fundamental
//! for pricing credit default swaps (CDS), corporate bonds, and credit derivatives.
//!
//! # Financial Concept
//!
//! The hazard rate λ(t) represents the instantaneous default intensity:
//! ```text
//! Survival probability: S(t) = P(τ > t) = exp(-∫₀ᵗ λ(s)ds)
//! Default probability: Q(t) = 1 - S(t)
//!
//! For piecewise-constant λ:
//! S(t) = exp(-Σ λᵢ * Δtᵢ)
//! ```
//!
//! # Market Construction
//!
//! Hazard curves are typically bootstrapped from:
//! - **CDS spreads**: Single-name CDS par spreads (market standard)
//! - **Bond spreads**: Credit spread over risk-free benchmark
//! - **Loan spreads**: Primary or secondary market loan pricing
//! - **Recovery assumptions**: Typically 40% for senior unsecured
//!
//! # Piecewise-Constant Model
//!
//! This implementation assumes constant hazard rates between knots, which:
//! - Provides analytical survival probabilities (no numerical integration)
//! - Ensures positive default probabilities (λ ≥ 0)
//! - Matches ISDA Standard CDS Model convention
//!
//! # Use Cases
//!
//! - **CDS pricing**: Protection and premium leg valuation
//! - **Corporate bond pricing**: Credit spread decomposition
//! - **CVA calculation**: Counterparty credit risk adjustment
//! - **CDO/CLO pricing**: Constituent credit curves for tranches
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::market_data::term_structures::HazardCurve;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let base = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
//! let hc = HazardCurve::builder("USD-CREDIT")
//!     .base_date(base)
//!     .recovery_rate(0.40)
//!     .knots([(1.0, 0.01), (10.0, 0.015)])
//!     .build()
//!     .expect("HazardCurve builder should succeed");
//! assert!(hc.sp(5.0) < 1.0); // Survival probability < 1
//! ```
//!
//! # References
//!
//! - **CDS Pricing**:
//! - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit Derivatives*.
//!   Wiley Finance. Chapters 3-5. `docs/REFERENCES.md#o-kane-2008`
//! - ISDA (2009). "ISDA CDS Standard Model." Version 1.8.2. `docs/REFERENCES.md#isda-cds-standard-model`
//!
//! - **Hazard Rate Models**:
//!   - Duffie, D., & Singleton, K. J. (1999). "Modeling Term Structures of Defaultable
//!   Bonds." *Review of Financial Studies*, 12(4), 687-720. `docs/REFERENCES.md#duffie-singleton-1999`
//!   - Lando, D. (1998). "On Cox Processes and Credit Risky Securities."
//!   *Review of Derivatives Research*, 2(2-3), 99-120. `docs/REFERENCES.md#lando-1998`

use crate::{
    currency::Currency,
    dates::{Date, DayCount, DayCountContext},
    error::InputError,
    market_data::traits::Survival,
    math::interp::{
        strategies::{LinearStrategy, LogLinearStrategy},
        types::Interp,
        ExtrapolationPolicy, InterpStyle, InterpolationStrategy,
    },
    types::CurveId,
};

/// Piecewise-constant credit hazard curve for default probability modeling.
///
/// Represents the instantaneous default intensity λ(t) for a credit issuer.
/// Assumes constant hazard rate between knots, providing analytical survival
/// probabilities without numerical integration.
///
/// # Mathematical Model
///
/// ```text
/// λ(t) = piecewise-constant hazard rate
/// S(t) = exp(-∫₀ᵗ λ(s)ds) = exp(-Σ λᵢ * Δtᵢ)
/// Q(t) = 1 - S(t) = cumulative default probability
/// ```
///
/// # Invariants
///
/// - All hazard rates λᵢ ≥ 0 (enforced at construction)
/// - Survival probability S(t) is monotonically decreasing
/// - Recovery rate ∈ [0, 1] (typically 40% for senior unsecured)
///
/// # Thread Safety
///
/// Immutable after construction; safe to share via `Arc<HazardCurve>`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawHazardCurve", into = "RawHazardCurve")]
pub struct HazardCurve {
    id: CurveId,
    base: Date,
    /// Time grid in years from base date; strictly increasing and non-negative.
    knots: Box<[f64]>,
    /// Piecewise-constant hazard rates λ ≥ 0; same length as `knots`.
    lambdas: Box<[f64]>,
    /// Recovery rate used during calibration/reporting (metadata)
    recovery_rate: f64,
    /// Optional issuer metadata
    issuer: Option<String>,
    /// Debt seniority
    pub seniority: Option<Seniority>,
    /// Currency of protection leg (metadata)
    currency: Option<Currency>,
    /// Day count convention for converting dates→times (metadata)
    day_count: DayCount,
    /// Stored market par spreads used to bootstrap this curve (for reporting)
    par_tenors: Box<[f64]>,
    /// Par spreads in basis points at `par_tenors`
    par_spreads_bp: Box<[f64]>,
    /// Default interpolation for par spreads
    par_interp: ParInterp,
    /// Interpolation style for survival probabilities between pillars
    /// (LogLinear ⇒ piecewise-constant hazard).
    survival_interp_style: InterpStyle,
    /// Exact typed recipe used to replay calibration after quote shocks.
    hazard_calibration: Option<crate::market_data::term_structures::HazardCalibrationRecipe>,
    /// Interpolator for survival probabilities
    interp: Interp,
    /// Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
    fx_policy: Option<String>,
}

/// Seniority level for credit exposures.
///
/// Used to tag a hazard curve with the seniority of the issuer's debt
/// observed by the curve. Drives the recovery-rate prior in default
/// modelling and selects the right LGD prior in
/// `finstack_quant_models::credit::lgd::seniority`.
///
/// Order is **not** total — `SeniorSecured` is strictly senior to `Senior`,
/// `Subordinated`, and `Junior`, but the relative ordering of `Subordinated`
/// vs. `Junior` is jurisdiction-dependent. Do not rely on `Ord` semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Seniority {
    /// Senior secured debt
    SeniorSecured,
    /// Senior unsecured debt
    Senior,
    /// Subordinated debt
    Subordinated,
    /// Junior/mezzanine debt
    Junior,
}

impl core::fmt::Display for Seniority {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Seniority::SeniorSecured => write!(f, "senior_secured"),
            Seniority::Senior => write!(f, "senior"),
            Seniority::Subordinated => write!(f, "subordinated"),
            Seniority::Junior => write!(f, "junior"),
        }
    }
}

impl core::str::FromStr for Seniority {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        match s {
            "senior_secured" => Ok(Self::SeniorSecured),
            "senior" => Ok(Self::Senior),
            "subordinated" => Ok(Self::Subordinated),
            "junior" => Ok(Self::Junior),
            _ => Err(crate::error::InputError::Invalid.into()),
        }
    }
}

/// Interpolation method for reporting par spreads stored on the curve.
///
/// Applies only to *par-spread* readouts (the spreads quoted at calibration
/// pillars), not to the underlying hazard rates. Hazard interpolation always
/// follows piecewise-constant survival. Use `LogLinear` when spreads span
/// multiple decades (e.g. high-yield issuers) so interpolation stays in
/// log-space; otherwise the default `Linear` is fine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ParInterp {
    /// Linear interpolation in spread space
    #[default]
    Linear,
    /// Log-linear interpolation when spreads are strictly positive
    LogLinear,
}

mod builder;
mod curve;
#[cfg(test)]
mod tests;
mod transform;
mod wire;
pub use builder::HazardCurveBuilder;
use wire::RawHazardCurve;

use curve::survival_pillars;
