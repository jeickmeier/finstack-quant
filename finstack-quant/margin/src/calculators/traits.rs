//! Traits and result types for margin calculators.

use crate::traits::Marginable;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use super::super::types::ImMethodology;

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
///     fn methodology(&self) -> ImMethodology {
///         ImMethodology::InternalModel
///     }
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

    /// Get the methodology this calculator implements.
    fn methodology(&self) -> ImMethodology;
}
