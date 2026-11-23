//! Volatility surface, correlation, and Hull-White calibration step parameters.

use super::steps::{default_sabr_beta, default_unit_notional};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::BusinessDayConvention;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::types::{CurveId, IndexId};
use serde::{Deserialize, Serialize};

/// Parameters for volatility surface calibration step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VolSurfaceParams {
    /// Identifier for the volatility surface being built.
    pub vol_surface_id: String,
    /// Base date for the surface.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Identifier for the underlying instrument.
    pub underlying_ticker: String,
    /// Discount curve ID.
    #[serde(default)]
    pub discount_curve_id: Option<CurveId>,
    /// SABR Beta parameter.
    #[serde(default = "default_sabr_beta")]
    pub beta: f64,
    /// Target expiries for calibration.
    #[serde(default)]
    pub target_expiries: Vec<f64>,
    /// Target strikes for calibration.
    #[serde(default)]
    pub target_strikes: Vec<f64>,
    /// Optional spot price override.
    #[serde(default)]
    pub spot_override: Option<f64>,
    /// Optional dividend yield override.
    #[serde(default)]
    pub dividend_yield_override: Option<f64>,
    /// Extrapolation policy for total-variance fill across expiries.
    ///
    /// After each quoted expiry is calibrated, the published expiry×strike
    /// grid is filled by interpolating total variance `w = σ²T` in expiry
    /// (not by interpolating SABR α/ν/ρ and evaluating Hagan at the target
    /// `T`). This policy controls targets that fall outside the calibrated
    /// expiry range: `Error` rejects them; `Clamp` holds the nearest slice's
    /// total variance flat.
    #[serde(default)]
    pub expiry_extrapolation: SurfaceExtrapolationPolicy,
}

/// Parameters for calibrating swaption volatility surfaces.
///
/// Defines the structure and conventions for building a volatility surface
/// from swaption quotes using the SABR model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SwaptionVolParams {
    /// Identifier for the volatility surface.
    pub vol_surface_id: String,
    /// Base date for the calibration.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Discount curve identifier for pricing.
    pub discount_curve_id: CurveId,
    /// Optional forward curve identifier (if different from discount curve).
    #[serde(default)]
    pub forward_id: Option<String>,
    /// Currency for the swaption surface.
    pub currency: Currency,
    /// Volatility quoting convention (normal or lognormal).
    #[serde(default)]
    pub vol_convention: SwaptionVolConvention,
    /// SABR beta parameter (typically 0.0 for normal, 1.0 for lognormal).
    #[serde(default = "default_sabr_beta")]
    pub sabr_beta: f64,
    /// Target option expiry times in ACT/365F years from `base_date`, independent
    /// of the fixed-leg coupon day-count convention.
    #[serde(default)]
    pub target_expiries: Vec<f64>,
    /// Target tenor times (in years) for the surface grid.
    #[serde(default)]
    pub target_tenors: Vec<f64>,
    /// Optional calendar identifier for date adjustments.
    #[serde(default)]
    pub calendar_id: Option<String>,
    /// Optional day count convention for fixed-leg coupon accrual only.
    /// Option expiry and variance always use ACT/365F.
    #[serde(default)]
    pub fixed_day_count: Option<DayCount>,

    /// Optional floating index identifier used to resolve market swap conventions.
    ///
    /// Swaption forward/par rate calculations require swap schedule conventions (fixed frequency,
    /// day count, calendar, BDC) that are now indexed off a rate index conventions registry.
    ///
    /// If omitted, individual swaption quotes must provide `float_leg_conventions.index`.
    #[serde(default)]
    pub swap_index: Option<IndexId>,
    /// Maximum absolute error of any fitted volatility quote; defaults to 0.0015.
    ///
    /// This is distinct from `plan.settings.tolerance` (solver tolerance). For swaption-vol
    /// calibration, success should reflect whether the fitted smile residuals are within a
    /// market-appropriate tolerance (e.g., 10–20 normal vol bp), not machine epsilon.
    /// Normal quotes use decimal rate per square-root year; Black quotes use
    /// dimensionless annual volatility. Errors are never scaled by vega.
    #[serde(default)]
    pub vol_tolerance: Option<f64>,

    /// Extrapolation policy used when interpolating SABR parameters across the
    /// expiry–tenor grid for target points that do not have a directly calibrated bucket.
    #[serde(default)]
    pub sabr_extrapolation: SurfaceExtrapolationPolicy,

    /// Allow deterministic fallbacks when SABR grid corners are missing during interpolation.
    ///
    /// If `false` (default), missing corner buckets are treated as an error instead of
    /// silently substituting a nearby bucket.
    #[serde(default)]
    pub allow_sabr_missing_bucket_fallback: bool,
}

/// Extrapolation policy for volatility surface construction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SurfaceExtrapolationPolicy {
    /// Reject out-of-bounds targets with an explicit error (vendor-matching).
    #[default]
    Error,
    /// Clamp targets to the nearest boundary (flat extrapolation).
    Clamp,
}

/// Parameters for calibrating base correlation curves.
///
/// Defines the structure for building a base correlation curve from
/// CDS tranche quotes with different detachment points.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BaseCorrelationParams {
    /// Credit index identifier (e.g., CDX, iTraxx).
    pub index_id: String,
    /// Series number of the credit index.
    pub series: u16,
    /// Finite positive CDS tenor in years, representable as a whole number of months.
    /// Quotes must resolve to the same CDS convention maturity as this tenor.
    pub maturity_years: f64,
    /// Base date for the calibration.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Discount curve identifier for pricing.
    pub discount_curve_id: CurveId,
    /// Currency used for synthetic tranche pricing.
    pub currency: Currency,
    /// Notional used to price synthetic tranches during calibration.
    ///
    /// Calibration can be expressed in upfront % terms, so this is typically left
    /// as unit-notional unless you have a specific reason to change it.
    #[serde(default = "default_unit_notional")]
    pub notional: f64,
    /// Payment frequency for synthetic tranches (e.g., quarterly).
    #[serde(default)]
    pub frequency: Option<Tenor>,
    /// Day count convention for synthetic tranche premium accrual.
    #[serde(default)]
    pub day_count: Option<DayCount>,
    /// Business day convention for synthetic tranche schedule adjustments.
    #[serde(default)]
    pub business_day_convention: Option<BusinessDayConvention>,
    /// Optional calendar identifier for schedule generation and date adjustments.
    #[serde(default)]
    pub calendar_id: Option<String>,
    /// Expected tranche detachment points as decimals in `(0, 1]` (e.g. `0.03`
    /// for 3%), the same unit as `CdsTrancheQuote.detachment`. When non-empty,
    /// every quote must match one point and every point must be quoted.
    #[serde(default)]
    pub detachment_points: Vec<f64>,
    /// Contractual date regime of the synthetic tranches (record
    /// CALIBRATION-FRAMEWORK-007, O7): `standard` CDS roll dates (20th of
    /// Mar/Jun/Sep/Dec) or `bespoke` for a schedule from the convention
    /// frequency and stub. Required; there is no default regime.
    pub date_regime: finstack_quant_valuations_credit::CdsDateRegime,
}

/// Parameters for Student-t copula degrees of freedom calibration.
///
/// Calibrates the `df` (degrees of freedom) parameter of a Student-t copula
/// by repricing a market tranche upfront quote and minimizing the residual
/// using Brent root-finding.
///
/// # Fields
///
/// - `tranche_instrument_id`: Identifier for the reference tranche instrument
///   used as the calibration target.
/// - `base_correlation_curve_id`: Identifier for the pre-calibrated base
///   correlation curve (must already exist in the market context).
/// - `initial_df`: Starting guess for the degrees of freedom (e.g., 5.0).
/// - `df_bounds`: Feasible domain for `df` as `(lo, hi)`, e.g., `(2.1, 50.0)`.
/// - `correlation`: Market-implied flat correlation for the tranche.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StudentTParams {
    /// Identifier for the reference tranche instrument.
    pub tranche_instrument_id: String,
    /// Identifier for the pre-calibrated base correlation curve.
    pub base_correlation_curve_id: String,
    /// Discount curve identifier used to price the tranche.
    ///
    /// When omitted, calibration falls back to the only discount curve present
    /// in the market context as a convenience default.
    #[serde(default)]
    pub discount_curve_id: Option<CurveId>,
    /// Starting guess for degrees of freedom (typically 4-10).
    #[serde(default = "default_student_t_initial_df")]
    pub initial_df: f64,
    /// Feasible domain for `df` as `(lower_bound, upper_bound)`.
    ///
    /// `df` must be > 2 for finite variance. Typical range: `(2.1, 50.0)`.
    #[serde(default = "default_student_t_df_bounds")]
    pub df_bounds: (f64, f64),
    /// Market-implied flat correlation for the tranche.
    #[serde(default = "default_student_t_correlation")]
    pub correlation: f64,
}

pub(super) fn default_student_t_initial_df() -> f64 {
    5.0
}

pub(super) fn default_student_t_df_bounds() -> (f64, f64) {
    (2.1, 50.0)
}

pub(super) fn default_student_t_correlation() -> f64 {
    0.3
}

/// Parameters for Hull-White 1-factor model calibration step.
///
/// Calibrates κ (mean reversion) and σ (short rate volatility) by fitting
/// ATM European swaption market prices using Jamshidian decomposition.
/// Supplied strikes must match the contractual forward swap rate within
/// `1e-8` in decimal rate units (0.0001 bp); off-ATM quotes are rejected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HullWhiteStepParams {
    /// Required positive maximum implied-quote error in quoted volatility units.
    /// Normal quotes use decimal rate volatility; Black quotes use relative volatility.
    /// This acceptance budget is independent of the numerical solver tolerance.
    pub fit_tolerance: f64,
    /// Discount curve ID (must already exist in market context).
    pub curve_id: CurveId,
    /// Currency for conventions.
    pub currency: Currency,
    /// Base date for the calibration.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Optional initial guess for mean reversion κ.
    #[serde(default)]
    pub initial_kappa: Option<f64>,
    /// Optional initial guess for short rate vol σ.
    #[serde(default)]
    pub initial_sigma: Option<f64>,
}

/// Parameters for Hull-White 1-factor calibration to cap/floor volatility quotes.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum HullWhiteVolatilityMode {
    /// Calibrate and persist one scalar short-rate volatility.
    #[default]
    Scalar,
    /// Bootstrap a piecewise-constant short-rate volatility schedule at quote expiries.
    Piecewise,
}

/// Parameters for Hull-White 1-factor calibration to cap/floor volatility quotes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CapFloorHullWhiteStepParams {
    /// Required positive maximum implied-quote error in quoted volatility units.
    /// Normal quotes use decimal rate volatility; Black quotes use relative volatility.
    /// This acceptance budget is independent of the numerical solver tolerance.
    pub fit_tolerance: f64,
    /// Discount curve ID (must already exist in market context).
    pub discount_curve_id: CurveId,
    /// Forward/projection curve ID. If equal to `discount_curve_id`, the
    /// discount curve is used as the single-curve projection proxy.
    pub forward_curve_id: CurveId,
    /// Currency for conventions.
    pub currency: Currency,
    /// Base date for the calibration.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Optional source mean reversion κ. Required for one-quote calibration.
    #[serde(default)]
    pub fixed_kappa: Option<f64>,
    /// Optional initial guess for mean reversion κ when solving both κ and σ.
    #[serde(default)]
    pub initial_kappa: Option<f64>,
    /// Optional initial guess for short-rate volatility σ when solving both κ and σ.
    #[serde(default)]
    pub initial_sigma: Option<f64>,
    /// Term-rate index defining settlement, reset tenor, accrual day count,
    /// calendar, business-day adjustments, and payment lag for caplets.
    /// Its currency must match `currency`; overnight indices are unsupported.
    pub index_id: IndexId,
    /// Scalar or expiry-bootstraped piecewise short-rate volatility calibration.
    #[serde(default)]
    pub volatility_mode: HullWhiteVolatilityMode,
}

/// Parameters for SVI volatility surface calibration step.
///
/// Fits a Stochastic Volatility Inspired (SVI) parameterization per-expiry
/// to market-implied volatilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SviSurfaceParams {
    /// Identifier for the volatility surface being built.
    pub vol_surface_id: String,
    /// Base date for the surface.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Underlying instrument ticker.
    pub underlying_ticker: String,
    /// Discount curve ID (optional).
    #[serde(default)]
    pub discount_curve_id: Option<CurveId>,
    /// Target expiries for calibration.
    #[serde(default)]
    pub target_expiries: Vec<f64>,
    /// Target strikes for calibration.
    #[serde(default)]
    pub target_strikes: Vec<f64>,
    /// Optional spot price override.
    #[serde(default)]
    pub spot_override: Option<f64>,
    /// Optional continuous dividend yield in decimal units. Without an override,
    /// uses the unitless market scalar `"<underlying_ticker>-DIVYIELD"`. When a
    /// discount curve is supplied, a matching cash-dividend schedule may supply
    /// carry instead; otherwise an explicit yield is required.
    #[serde(default)]
    pub dividend_yield_override: Option<f64>,
}

/// Volatility quoting convention for swaptions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SwaptionVolConvention {
    /// Normal (Bachelier) absolute volatility in decimal rate units.
    ///
    /// Example: `0.0050` means 50 basis points of absolute volatility.
    Normal,
    /// Lognormal (Black) volatility in decimal units.
    ///
    /// Example: `0.20` means 20% Black volatility.
    /// Requires `black_lognormal` quotes and positive forwards/strikes;
    /// calibration does not introduce a displacement automatically.
    #[default]
    Lognormal,
    /// Shifted lognormal (Black) volatility in decimal units, with an explicit shift.
    ///
    /// Example: `0.20` means 20% Black volatility on displaced forward/strike
    /// coordinates. Requires `shifted_black_lognormal` quotes.
    ShiftedLognormal {
        /// Finite signed additive shift in decimal rate units, applied to
        /// both forwards and strikes and retained in calibrated artifacts.
        /// The effective forward and strikes must be positive.
        shift: f64,
    },
}
