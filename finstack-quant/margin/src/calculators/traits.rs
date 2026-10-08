//! Traits and result types for margin calculators.

use crate::traits::Marginable;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use super::super::types::{ImMethodology, SimmRiskClass};

/// Initial margin calculation result.
///
/// Contains the calculated IM amount along with methodology details
/// and breakdown information.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ImResult {
    /// Calculated initial margin amount
    pub amount: Money,

    /// Methodology used for calculation
    pub methodology: ImMethodology,

    /// Calculation date
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub as_of: Date,

    /// Margin Period of Risk in business days used in calculation
    pub mpor_days: u32,

    /// Breakdown by risk class (if available)
    ///
    /// Keys are methodology-specific component labels. SIMM publishes
    /// `IR_Delta`, `IR_Vega`, `Credit_Qualifying_Delta`,
    /// `Credit_Qualifying_Vega`, `Credit_NonQualifying_Delta`,
    /// `Credit_NonQualifying_Vega`, `Equity_Delta`, `Equity_Vega`, `FX_Delta`,
    /// `FX_Vega`, `Commodity_Delta`, `Commodity_Vega` and `Curvature`; the
    /// schedule calculator publishes the normalised asset class (for example
    /// `interest_rate`). Values are IM amounts for that component.
    pub breakdown: std::collections::BTreeMap<String, Money>,

    /// Whether the amount is an approximation rather than
    /// an exact computation under the named methodology.
    ///
    /// Approximations need not be conservative. Historical SIMM sets this flag
    /// because its input dimensions and some aggregation stages are simplified.
    /// Also set by the clearing-house and internal-model calculators when
    /// they fall back to `|exposure_base| x conservative_rate` because no
    /// [`ExternalImSource`](crate::calculators::im::ExternalImSource) supplied
    /// a real margin amount. Portfolio-level consumers should surface this
    /// flag: an approximated IM is suitable for indicative funding/capacity
    /// analysis, not for reconciling actual CCP margin calls.
    pub approximation: bool,

    /// SIMM aggregation detail behind `breakdown` and `amount`: weighted
    /// sensitivities, concentration factors and bucket-level `K` for every
    /// component, plus the per-risk-class totals and the MPOR scale.
    ///
    /// `Some` only for results produced by the SIMM calculator; `None` for
    /// every other methodology and for JSON written before this field
    /// existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simm_detail: Option<SimmDetail>,
}

/// Aggregation detail of one SIMM calculation.
///
/// All amounts are in the calculation currency (USD) and are stated
/// **before** the MPOR scale, so:
///
/// ```text
/// breakdown[c.component]      = c.margin × mpor_scale
/// risk_class_margins[class]   = Σ c.margin over components of that class
/// amount                      = sqrt(Σᵢ Σⱼ ψᵢⱼ Kᵢ Kⱼ) × mpor_scale
/// ```
///
/// where `Kᵢ` are the `risk_class_margins` and `ψ` is the SIMM
/// inter-risk-class correlation matrix of the calculator's parameters.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SimmDetail {
    /// One entry per key of [`ImResult::breakdown`], in calculation order.
    pub components: Vec<SimmComponentDetail>,

    /// Margin per risk class before inter-risk-class correlation and before
    /// the MPOR scale: the sum of that class's delta, vega and curvature
    /// component margins. Risk classes with no positive component are absent.
    pub risk_class_margins: std::collections::BTreeMap<SimmRiskClass, f64>,

    /// Multiplier applied to the aggregate and to every breakdown amount:
    /// `sqrt(mpor_days / 10)` (dimensionless; `1` at the standard 10-day
    /// MPOR).
    pub mpor_scale: f64,
}

/// Bucket-level detail of one SIMM breakdown component.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SimmComponentDetail {
    /// Component label; equals the matching [`ImResult::breakdown`] key (for
    /// example `IR_Delta`).
    pub component: String,

    /// Risk class whose margin this component adds to.
    pub risk_class: SimmRiskClass,

    /// Historical volatility ratio (dimensionless). For equity, FX and
    /// commodity vega components it multiplies every `sensitivity` before
    /// risk weighting. For the interest-rate curvature component the
    /// component margin is divided by its square. `1` everywhere else.
    pub historical_volatility_ratio: f64,

    /// Buckets in the canonical order used by the cross-bucket aggregation.
    pub buckets: Vec<SimmBucketDetail>,

    /// Component margin after cross-bucket aggregation, before the MPOR
    /// scale, in the calculation currency.
    pub margin: f64,
}

/// One SIMM bucket: its weighted sensitivities and intra-bucket aggregate.
///
/// What a bucket is depends on the component: a currency for interest rate,
/// a credit sector for credit qualifying, `residual` for credit
/// non-qualifying, one name for equity, `fx` for FX, and the commodity
/// bucket number for commodity.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SimmBucketDetail {
    /// Bucket label (see the type documentation).
    pub bucket: String,

    /// Weighted sensitivities in the order they enter the intra-bucket
    /// correlated sum.
    pub weighted_sensitivities: Vec<SimmWeightedSensitivity>,

    /// Intra-bucket aggregate `K_b = sqrt(Σᵢ Σⱼ ρᵢⱼ WSᵢ WSⱼ)` in the
    /// calculation currency, with `ρ` from the calculator's parameters.
    pub k: f64,

    /// Sum of the weighted sensitivities capped to `[-k, k]`: the `S_b` term
    /// used wherever the component correlates this bucket with another.
    pub signed_sum: f64,
}

/// One weighted sensitivity inside a SIMM bucket.
///
/// For delta and vega components
/// `weighted_sensitivity = sensitivity × historical_volatility_ratio ×
/// risk_weight × concentration_factor` (the ratio is the component's). For
/// curvature components `sensitivity` is the factor's curvature risk
/// exposure after the expiry scaling factor and netting, and the weight and
/// concentration factor are `1`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SimmWeightedSensitivity {
    /// Risk factor label: currency, issuer or name, currency pair
    /// (`AAA/BBB`), or commodity bucket number.
    pub risk_factor: String,

    /// Tenor label of the sensitivity; `None` for factors without a tenor
    /// dimension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenor: Option<String>,

    /// Net input sensitivity for this factor, in the units documented on
    /// [`SimmSensitivities`](crate::SimmSensitivities) (calculation currency
    /// per unit move).
    pub sensitivity: f64,

    /// SIMM risk weight from the calculator's parameters, as published and
    /// multiplied directly against the sensitivity.
    pub risk_weight: f64,

    /// Concentration risk factor `max(1, sqrt(|net sensitivity| / threshold))`
    /// applied to this sensitivity (dimensionless, `≥ 1`).
    pub concentration_factor: f64,

    /// Weighted sensitivity `WS` entering the intra-bucket aggregation, in
    /// the calculation currency.
    pub weighted_sensitivity: f64,
}

impl ImResult {
    /// Create a simple IM result with no breakdown.
    #[must_use]
    pub fn simple(amount: Money, methodology: ImMethodology, as_of: Date, mpor_days: u32) -> Self {
        Self {
            amount,
            methodology,
            as_of,
            mpor_days,
            breakdown: std::collections::BTreeMap::new(),
            approximation: false,
            simm_detail: None,
        }
    }

    /// Create an IM result with breakdown by risk class.
    #[must_use]
    pub fn with_breakdown(
        amount: Money,
        methodology: ImMethodology,
        as_of: Date,
        mpor_days: u32,
        breakdown: finstack_quant_core::HashMap<String, Money>,
    ) -> Self {
        Self {
            amount,
            methodology,
            as_of,
            mpor_days,
            breakdown: breakdown.into_iter().collect(),
            approximation: false,
            simm_detail: None,
        }
    }
}

/// Trait for initial margin calculators.
///
/// Implement this trait to provide custom IM calculation logic
/// for different methodologies or instrument types.
///
/// # Example Implementation
///
/// ```
/// use finstack_quant_margin::{ImCalculator, ImMethodology, ImResult, Marginable};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
/// use finstack_quant_core::money::Money;
///
/// struct CustomImCalculator {
///     fixed_rate: f64,
/// }
///
/// impl ImCalculator for CustomImCalculator {
///     fn calculate(
///         &self,
///         instrument: &dyn Marginable,
///         context: &MarketContext,
///         as_of: Date,
///     ) -> finstack_quant_core::Result<ImResult> {
///         let mtm = instrument.mtm_for_vm(context, as_of)?;
///         let im = Money::new(mtm.amount().abs() * self.fixed_rate, mtm.currency()).expect("valid money fixture");
///         Ok(ImResult::simple(
///             im,
///             ImMethodology::InternalModel,
///             as_of,
///             10,
///         ))
///     }
///
/// }
/// ```
pub trait ImCalculator: Send + Sync {
    /// Calculate initial margin for an instrument.
    ///
    /// # Arguments
    ///
    /// * `instrument` - The financial instrument requiring IM
    /// * `context` - Market data context with curves and surfaces
    /// * `as_of` - Valuation date
    ///
    /// # Returns
    ///
    /// [`ImResult`] containing the calculated IM amount and methodology details.
    fn calculate(
        &self,
        instrument: &dyn Marginable,
        context: &MarketContext,
        as_of: Date,
    ) -> Result<ImResult>;
}
