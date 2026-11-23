//! Calibration step enum and curve-family step parameters.

use super::surface_steps::*;
use crate::config::{CalibrationMethod, RatesStepConventions};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::term_structures::{NelsonSiegelModel, NsVariant, Seniority};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::market::conventions::CdsValuationConvention;
use serde::{Deserialize, Serialize};

/// Polymorphic parameters for different calibration step types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StepParams {
    /// Discount curve calibration.
    Discount(DiscountCurveParams),

    /// Forward curve calibration.
    Forward(ForwardCurveParams),

    /// Hazard curve calibration.
    Hazard(HazardCurveParams),

    /// Inflation curve calibration.
    Inflation(InflationCurveParams),

    /// Volatility surface calibration.
    VolSurface(VolSurfaceParams),

    /// Swaption volatility surface calibration.
    SwaptionVol(SwaptionVolParams),

    /// Base correlation calibration.
    BaseCorrelation(BaseCorrelationParams),

    /// Student-t copula degrees of freedom calibration.
    StudentT(StudentTParams),

    /// Hull-White 1-factor model calibration.
    HullWhite(HullWhiteStepParams),

    /// Hull-White 1-factor calibration to cap/floor volatility quotes.
    CapFloorHullWhite(CapFloorHullWhiteStepParams),

    /// SVI volatility surface calibration.
    SviSurface(SviSurfaceParams),

    /// Cross-currency basis curve calibration.
    XccyBasis(XccyBasisParams),

    /// Parametric (Nelson-Siegel / NSS) curve calibration.
    Parametric(ParametricCurveParams),
}

/// Static input/output metadata for a calibration step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StepIo {
    /// Curve / surface / scalar IDs the step reads.
    pub(crate) reads: Vec<String>,
    /// Curve / surface / scalar IDs the step writes.
    pub(crate) writes: Vec<String>,
}

impl StepParams {
    /// Step kind tag as serialized in envelope JSON (`"discount"`, `"forward"`, ...).
    #[must_use]
    pub const fn kind_str(&self) -> &'static str {
        match self {
            Self::Discount(_) => "discount",
            Self::Forward(_) => "forward",
            Self::Hazard(_) => "hazard",
            Self::Inflation(_) => "inflation",
            Self::VolSurface(_) => "vol_surface",
            Self::SwaptionVol(_) => "swaption_vol",
            Self::BaseCorrelation(_) => "base_correlation",
            Self::StudentT(_) => "student_t",
            Self::HullWhite(_) => "hull_white",
            Self::CapFloorHullWhite(_) => "cap_floor_hull_white",
            Self::SviSurface(_) => "svi_surface",
            Self::XccyBasis(_) => "xccy_basis",
            Self::Parametric(_) => "parametric",
        }
    }

    /// Return the static dependency metadata for this step.
    pub(crate) fn io(&self) -> StepIo {
        match self {
            StepParams::Discount(p) => StepIo {
                reads: [
                    p.pricing_discount_id.as_ref(),
                    p.pricing_forward_id.as_ref(),
                ]
                .into_iter()
                .flatten()
                .filter(|id| *id != &p.curve_id)
                .map(ToString::to_string)
                .collect(),
                writes: vec![p.curve_id.to_string()],
            },
            StepParams::Forward(p) => StepIo {
                reads: vec![p.discount_curve_id.to_string()],
                writes: vec![p.curve_id.to_string()],
            },
            StepParams::Hazard(p) => StepIo {
                reads: vec![p.discount_curve_id.to_string()],
                writes: vec![p.curve_id.to_string()],
            },
            StepParams::Inflation(p) => StepIo {
                reads: vec![p.discount_curve_id.to_string()],
                writes: vec![p.curve_id.to_string()],
            },
            StepParams::VolSurface(p) => StepIo {
                reads: p
                    .discount_curve_id
                    .as_ref()
                    .map(ToString::to_string)
                    .into_iter()
                    .chain(
                        p.spot_override
                            .is_none()
                            .then(|| p.underlying_ticker.clone()),
                    )
                    .collect(),
                writes: vec![p.vol_surface_id.clone()],
            },
            StepParams::SwaptionVol(p) => StepIo {
                reads: std::iter::once(p.discount_curve_id.to_string())
                    .chain(p.forward_id.clone())
                    .collect(),
                writes: vec![p.vol_surface_id.clone()],
            },
            StepParams::BaseCorrelation(p) => {
                let curve_id = CurveId::from(format!("{}_CORR", p.index_id));
                StepIo {
                    reads: vec![p.discount_curve_id.to_string(), p.index_id.clone()],
                    writes: vec![curve_id.to_string(), p.index_id.clone()],
                }
            }
            StepParams::StudentT(p) => {
                let mut reads = vec![p.base_correlation_curve_id.clone()];
                if let Some(discount_curve_id) = &p.discount_curve_id {
                    reads.push(discount_curve_id.to_string());
                }
                let scalar_key = format!("{}_STUDENT_T_DF", p.tranche_instrument_id);
                StepIo {
                    reads,
                    writes: vec![scalar_key],
                }
            }
            StepParams::HullWhite(p) => {
                let (kappa_key, sigma_key) =
                    finstack_quant_models::rates::hull_white::hw1f_scalar_keys(p.curve_id.as_str());
                StepIo {
                    reads: vec![p.curve_id.to_string()],
                    writes: vec![kappa_key, sigma_key],
                }
            }
            StepParams::CapFloorHullWhite(p) => {
                let mut reads = vec![p.discount_curve_id.to_string()];
                if p.forward_curve_id != p.discount_curve_id {
                    reads.push(p.forward_curve_id.to_string());
                }
                let (kappa_key, sigma_key) =
                    finstack_quant_models::rates::hull_white::capfloor_hw1f_scalar_keys(
                        p.discount_curve_id.as_str(),
                    );
                let volatility_key = match p.volatility_mode {
                    HullWhiteVolatilityMode::Scalar => sigma_key,
                    HullWhiteVolatilityMode::Piecewise => {
                        finstack_quant_models::rates::hull_white::capfloor_hw1f_sigma_schedule_key(
                            p.discount_curve_id.as_str(),
                        )
                    }
                };
                StepIo {
                    reads,
                    writes: vec![kappa_key, volatility_key],
                }
            }
            StepParams::SviSurface(p) => StepIo {
                reads: p
                    .discount_curve_id
                    .as_ref()
                    .map(ToString::to_string)
                    .into_iter()
                    .chain(
                        p.spot_override
                            .is_none()
                            .then(|| p.underlying_ticker.clone()),
                    )
                    .collect(),
                writes: vec![p.vol_surface_id.clone()],
            },
            StepParams::XccyBasis(p) => StepIo {
                reads: vec![p.domestic_discount_id.to_string()],
                writes: vec![p.curve_id.to_string()],
            },
            StepParams::Parametric(p) => StepIo {
                reads: Vec::new(),
                writes: vec![p.curve_id.to_string()],
            },
        }
    }
}

// Step Parameter Structs

/// Parameters for discount curve calibration step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscountCurveParams {
    /// Identifier for the discount curve being built.
    pub curve_id: CurveId,
    /// Currency of the curve.
    pub currency: Currency,
    /// Base date for the curve.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Calibration method to use.
    #[serde(default)]
    pub method: CalibrationMethod,
    /// Interpolation style for the constructed discount curve.
    ///
    /// Defaults to log-linear discount factors, preserving positive discount
    /// factors and piecewise-constant continuously compounded forwards.
    /// `Linear` remains available only when explicitly requested.
    #[serde(default = "default_interp_log_linear")]
    pub interpolation: InterpStyle,
    /// Extrapolation policy for the curve.
    #[serde(default = "default_extrap_extend")]
    pub extrapolation: ExtrapolationPolicy,
    /// Optional separate ID for pricing logic (defaults to curve_id).
    #[serde(default)]
    pub pricing_discount_id: Option<CurveId>,
    /// Optional forward curve ID for pricing (if needed).
    #[serde(default)]
    pub pricing_forward_id: Option<CurveId>,

    /// Step-level conventions for pricing and curve time axis.
    #[serde(default)]
    pub conventions: RatesStepConventions,
}

/// Parameters for forward curve calibration step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ForwardCurveParams {
    /// Identifier for the forward curve being built.
    pub curve_id: CurveId,
    /// Currency of the curve.
    pub currency: Currency,
    /// Base date for the curve.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Contractual index tenor metadata attached to the calibrated forward curve.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_tenor: Option<Tenor>,
    /// Identifier for the discount curve to use.
    pub discount_curve_id: CurveId,
    /// Calibration method to use.
    ///
    /// Forward curves require [`CalibrationMethod::GlobalSolve`], which is the
    /// default (finite-difference Jacobian). Sequential bootstrap is rejected
    /// because contractual off-grid reset intervals couple adjacent rates
    /// through projection discount factors.
    #[serde(default = "default_forward_method")]
    pub method: CalibrationMethod,
    /// Interpolation style for the constructed forward curve's projection factors.
    ///
    /// Defaults to log-linear factor interpolation (`log_linear`). All five
    /// [`InterpStyle`] variants (`linear`, `log_linear`, `cubic_hermite`,
    /// `piecewise_quadratic_forward`, `monotone_convex`) are supported on the
    /// underlying positive projection-factor evaluator.
    #[serde(default = "default_interp_log_linear")]
    pub interpolation: InterpStyle,
    /// Policy controlling queries beyond the final calibrated pillar time.
    #[serde(default = "default_extrap_extend")]
    pub extrapolation: ExtrapolationPolicy,

    /// Step-level conventions for pricing and curve time axis.
    #[serde(default)]
    pub conventions: RatesStepConventions,
}

/// Parameters for hazard curve calibration step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HazardCurveParams {
    /// Identifier for the hazard curve being built.
    pub curve_id: CurveId,
    /// Entity name.
    pub entity: String,
    /// Seniority of the debt.
    pub seniority: Seniority,
    /// Currency of the curve.
    pub currency: Currency,
    /// Base date for the curve.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Identifier for the discount curve to use.
    pub discount_curve_id: CurveId,
    /// Required recovery-rate assumption as a decimal fraction in `[0, 1]`.
    pub recovery_rate: f64,
    /// Notional used to price synthetic CDS instruments during calibration.
    ///
    /// Calibration normalizes residuals by notional, so this is typically left as
    /// the unit-notional default unless you have a specific reason to change it.
    #[serde(default = "default_unit_notional")]
    pub notional: f64,
    /// Calibration method to use.
    #[serde(default)]
    pub method: CalibrationMethod,

    /// Optional CDS documentation-clause assertion.
    ///
    /// Hazard schedule conventions come from the quote `CdsConventionKey`. When
    /// this field is set, it must be consistent with those quote-derived
    /// conventions (same clause or the matching regional family).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_clause: Option<String>,
    /// Optional CDS valuation convention used by synthetic CDS instruments
    /// during hazard calibration and rebootstrap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cds_valuation_convention: Option<CdsValuationConvention>,
}

/// Parameters for inflation curve calibration step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InflationCurveParams {
    /// Identifier for the inflation curve being built.
    pub curve_id: CurveId,
    /// Currency of the curve.
    pub currency: Currency,
    /// Valuation and calibration-instrument start date, independent of the source origin.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Identifier for the discount curve to use.
    pub discount_curve_id: CurveId,
    /// Reference index (e.g. "USA-CPI-U").
    pub index: String,
    /// Explicit contractual month observation lag (e.g. "3M"); "none" means zero months.
    /// Overrides the quote convention's lag without changing the projection source origin.
    pub observation_lag: String,
    /// Explicit contractual reference CPI at the start date, after lag and interpolation.
    /// This denominator is independent of the projection source's origin CPI.
    pub base_cpi: f64,
    /// Explicit original date of the absolute CPI projection source.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub source_base_date: Date,
    /// Finite positive absolute CPI level at the projection source's origin.
    pub source_base_cpi: f64,
    /// Complete source-clock convention used for every projected anchor.
    pub day_count: DayCount,
    /// Source-clock holiday calendar; required for business-day clocks.
    pub calendar_id: Option<String>,
    /// Notional used to price synthetic inflation swaps during calibration.
    ///
    /// Calibration normalizes residuals by notional, so this is typically left as
    /// the unit-notional default unless you have a specific reason to change it.
    #[serde(default = "default_unit_notional")]
    pub notional: f64,
    /// Calibration method to use.
    #[serde(default)]
    pub method: CalibrationMethod,
    /// Interpolation style for the curve.
    pub interpolation: InterpStyle,
    /// Explicit scalar CPI tail policy, applied without lag or normalization.
    pub extrapolation: finstack_quant_core::math::interp::ExtrapolationPolicy,
}

pub(super) fn default_forward_method() -> CalibrationMethod {
    CalibrationMethod::GlobalSolve {
        use_analytical_jacobian: false,
    }
}

pub(super) fn default_interp_log_linear() -> InterpStyle {
    InterpStyle::LogLinear
}

pub(super) fn default_extrap_extend() -> ExtrapolationPolicy {
    ExtrapolationPolicy::Extend
}

pub(super) fn default_unit_notional() -> f64 {
    1.0
}

pub(super) fn default_sabr_beta() -> f64 {
    0.5
}

// XCCY Basis Curve

/// Parameters for cross-currency basis curve calibration step.
///
/// Derives a foreign-currency discount curve from a domestic OIS curve,
/// FX spot rate, and cross-currency basis swap or FX forward quotes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XccyBasisParams {
    /// Identifier for the foreign discount curve being built.
    pub curve_id: CurveId,
    /// Foreign currency being calibrated.
    pub currency: Currency,
    /// Base date for the curve.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// T+0 cash FX rate (domestic per foreign), used when a quote omits `spot_fx`.
    ///
    /// Covered-interest parity from today needs the cash FX, not the screen
    /// "spot". Market spot is T+2 for most G10 pairs and T+1 for USD/CAD;
    /// convert to T+0 using ON/TN points before passing the rate here.
    /// Mixing T+2 screen spot with T+0 discounting biases long-tenor basis
    /// by roughly 1–2 bp.
    pub fx_spot: f64,
    /// Identifier for the pre-calibrated domestic discount curve.
    pub domestic_discount_id: CurveId,
    /// Sequential bootstrap method. `GlobalSolve` is unsupported and rejected.
    #[serde(default)]
    pub method: CalibrationMethod,
    /// Interpolation style for the constructed foreign discount curve.
    ///
    /// Defaults to log-linear discount factors. `Linear` remains available
    /// only when explicitly requested.
    #[serde(default = "default_interp_log_linear")]
    pub interpolation: InterpStyle,
    /// Extrapolation policy for the foreign curve.
    #[serde(default = "default_extrap_extend")]
    pub extrapolation: ExtrapolationPolicy,
    /// Step-level conventions for pricing and curve time axis.
    #[serde(default)]
    pub conventions: RatesStepConventions,
}

// Parametric (NS / NSS) Curve

/// Parameters for parametric curve calibration step.
///
/// Fits a Nelson-Siegel or Nelson-Siegel-Svensson yield curve model to
/// rate instrument quotes using global (Levenberg-Marquardt) optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ParametricCurveParams {
    /// Identifier for the single discount curve being fitted. All calibration
    /// instruments use this curve for discounting and implied projection.
    pub curve_id: CurveId,
    /// Base date for the curve.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub base_date: Date,
    /// Nelson-Siegel variant (NS or NSS).
    pub model: NsVariant,
    /// Optional initial parameter guesses.
    #[serde(default)]
    pub initial_params: Option<NelsonSiegelModel>,
}
