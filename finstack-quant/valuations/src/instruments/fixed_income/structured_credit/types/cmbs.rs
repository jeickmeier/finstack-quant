//! Commercial-mortgage (CMBS) collateral terms: balloon maturities that may
//! extend instead of paying, prepayment penalties that flow to the trust as
//! interest, and special servicing with appraisal reductions.

use finstack_quant_core::dates::Date;
use serde::{Deserialize, Serialize};

/// Balloon maturity behavior of a commercial mortgage.
///
/// At the loan's maturity the share `default_prob` of the balance fails to
/// refinance and is extended by `extension_months` at `extension_rate` (the
/// loan's own coupon when `None`); the rest pays as the balloon. The extended
/// balance pays at the extended maturity (one extension per loan).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::structured_credit::BalloonSpec;
///
/// // 30% of the balloon extends two years at 7%.
/// let balloon = BalloonSpec { default_prob: 0.3, extension_months: 24, extension_rate: Some(0.07) };
/// assert!(balloon.validate().is_ok());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BalloonSpec {
    /// Share of the balloon balance that fails to refinance at maturity, as
    /// a decimal in `[0, 1]`.
    pub default_prob: f64,
    /// Months the non-refinancing share is extended.
    pub extension_months: u32,
    /// Annual coupon of the extended balance as a decimal; `None` keeps the
    /// loan's coupon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_rate: Option<f64>,
}

impl BalloonSpec {
    /// Validate the extension share, term and rate.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `default_prob` is outside `[0, 1]`,
    /// `extension_months` is zero, or the extension rate is negative or
    /// non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        if !self.default_prob.is_finite() || !(0.0..=1.0).contains(&self.default_prob) {
            return Err(invalid(format!(
                "balloon default_prob ({}) must be a decimal in [0, 1]",
                self.default_prob
            )));
        }
        if self.extension_months == 0 {
            return Err(invalid(
                "balloon extension_months must be positive".to_string(),
            ));
        }
        if let Some(rate) = self.extension_rate {
            if !rate.is_finite() || rate < 0.0 {
                return Err(invalid(format!(
                    "balloon extension_rate ({rate}) must be finite and non-negative"
                )));
            }
        }
        Ok(())
    }
}

/// Prepayment penalty a borrower pays on voluntary prepayment; the premium
/// reaches the trust as interest collections.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PrepaymentPenalty {
    /// A fixed percent of the prepaid balance.
    Fixed {
        /// Premium in percent of the prepaid balance (`3.0` = 3%).
        pct: f64,
        /// Last date the penalty applies; `None` applies it to maturity.
        #[serde(default, with = "finstack_quant_core::wire::optional_date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Option<finstack_quant_core::wire::DateWire>")
        )]
        through: Option<Date>,
    },
    /// Yield maintenance: the coupon the trust loses over the remaining
    /// term, `prepaid × max(0, coupon − reinvestment_rate) × years to
    /// maturity` (undiscounted).
    YieldMaintenance {
        /// Annual rate the prepaid principal is assumed to be reinvested at,
        /// as a decimal.
        reinvestment_rate: f64,
        /// Last date the penalty applies; `None` applies it to maturity.
        #[serde(default, with = "finstack_quant_core::wire::optional_date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Option<finstack_quant_core::wire::DateWire>")
        )]
        through: Option<Date>,
    },
}

impl PrepaymentPenalty {
    /// Premium on a prepayment of `prepaid` made on `date`.
    ///
    /// # Arguments
    ///
    /// * `prepaid` - Principal prepaid in the period, in currency units.
    /// * `coupon` - The loan's annual coupon as a decimal.
    /// * `date` - Payment date of the prepayment.
    /// * `maturity` - The loan's maturity, for the yield-maintenance term.
    ///
    /// # Returns
    ///
    /// Premium in currency units; zero outside the penalty window.
    pub fn premium(&self, prepaid: f64, coupon: f64, date: Date, maturity: Date) -> f64 {
        if prepaid <= 0.0 {
            return 0.0;
        }
        match self {
            Self::Fixed { pct, through } => {
                if through.is_some_and(|through| date > through) {
                    0.0
                } else {
                    prepaid * pct / 100.0
                }
            }
            Self::YieldMaintenance {
                reinvestment_rate,
                through,
            } => {
                if through.is_some_and(|through| date > through) || maturity <= date {
                    return 0.0;
                }
                let years = (maturity - date).whole_days() as f64 / 365.25;
                prepaid * (coupon - reinvestment_rate).max(0.0) * years
            }
        }
    }

    /// Validate the penalty terms.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the fixed percent or the
    /// reinvestment rate is negative or non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let (label, value) = match self {
            Self::Fixed { pct, .. } => ("prepayment penalty pct", *pct),
            Self::YieldMaintenance {
                reinvestment_rate, ..
            } => ("prepayment penalty reinvestment_rate", *reinvestment_rate),
        };
        if !value.is_finite() || value < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{label} ({value}) must be finite and non-negative"
            )));
        }
        Ok(())
    }
}

/// Special-servicing state of a commercial mortgage.
///
/// The appraisal reduction cuts the interest advanced on the loan
/// (an appraisal subordinate entitlement reduction, ASER): the trust
/// collects interest on `1 − appraisal_reduction_pct / 100` of the balance
/// and the shortfall falls on the most junior classes first through the
/// sequential interest waterfall. The special servicing fee accrues only on
/// specially serviced balances.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SpecialServicingSpec {
    /// Appraisal reduction in percent of the loan balance (`40.0` = 40%).
    pub appraisal_reduction_pct: f64,
}

impl SpecialServicingSpec {
    /// Validate the appraisal reduction.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the percent is outside `[0, 100]`.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.appraisal_reduction_pct.is_finite()
            || !(0.0..=100.0).contains(&self.appraisal_reduction_pct)
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "appraisal_reduction_pct ({}) must be a percent in [0, 100]",
                self.appraisal_reduction_pct
            )));
        }
        Ok(())
    }
}
