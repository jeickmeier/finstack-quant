//! Traits and result types for margin calculators.

use crate::traits::Marginable;
use finstack_core::dates::Date;
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_core::Result;

use super::super::types::ImMethodology;

/// Initial margin calculation result.
///
/// Contains the calculated IM amount along with methodology details
/// and breakdown information.
#[derive(Debug, Clone, PartialEq)]
pub struct ImResult {
    /// Calculated initial margin amount
    pub amount: Money,

    /// Methodology used for calculation
    pub methodology: ImMethodology,

    /// Calculation date
    pub as_of: Date,

    /// Margin Period of Risk (days) used in calculation
    pub mpor_days: u32,

    /// Breakdown by risk class (if available)
    ///
    /// Keys are risk class names (e.g., "interest_rate", "credit", "equity")
    /// Values are IM amounts for that risk class
    pub breakdown: finstack_core::HashMap<String, Money>,
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
            breakdown: finstack_core::HashMap::default(),
        }
    }

    /// Create an IM result with breakdown by risk class.
    #[must_use]
    pub fn with_breakdown(
        amount: Money,
        methodology: ImMethodology,
        as_of: Date,
        mpor_days: u32,
        breakdown: finstack_core::HashMap<String, Money>,
    ) -> Self {
        Self {
            amount,
            methodology,
            as_of,
            mpor_days,
            breakdown,
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
/// ```ignore
/// use finstack_margin::{ImCalculator, ImMethodology, ImResult, Marginable};
/// use finstack_core::market_data::context::MarketContext;
/// use finstack_core::dates::Date;
/// use finstack_core::money::Money;
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
///     ) -> finstack_core::Result<ImResult> {
///         let mtm = instrument.mtm_for_vm(context, as_of)?;
///         let im = Money::new(mtm.amount().abs() * self.fixed_rate, mtm.currency());
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
