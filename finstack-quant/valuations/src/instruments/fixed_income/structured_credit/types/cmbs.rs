//! Commercial-mortgage (CMBS) collateral terms: balloon maturities that may
//! extend instead of paying, prepayment penalties that flow to the trust as
//! interest, and special servicing with appraisal reductions.

use finstack_quant_core::dates::{Date, DateExt};
use serde::{Deserialize, Serialize};

/// Balloon terms of a commercial mortgage at its maturity.
///
/// The performing balance at maturity splits three ways: `loss_prob` defaults
/// (a workout that recovers `100 − severity_pct` percent after
/// `workout_months`), `extension_prob` is extended `extension_months` at
/// `extension_rate` (the loan's coupon when `None`), and the rest pays as the
/// balloon. An extended level-pay balance keeps its schedule scaled by the
/// extended fraction; a fixed `extension_rate` on a floating loan replaces the
/// index and spread (an all-in modification rate).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::structured_credit::BalloonSpec;
///
/// // 30% of the balloon extends two years at 7%; 10% defaults at 40% severity
/// // with an 18-month workout.
/// let balloon = BalloonSpec {
///     extension_prob: 0.3,
///     extension_months: 24,
///     extension_rate: Some(0.07),
///     loss_prob: 0.1,
///     severity_pct: 40.0,
///     workout_months: 18,
/// };
/// assert!(balloon.validate().is_ok());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BalloonSpec {
    /// Share of the balloon balance that fails to refinance and is extended,
    /// as a decimal in `[0, 1]`.
    pub extension_prob: f64,
    /// Months the extended share is extended.
    pub extension_months: u32,
    /// Annual coupon of the extended balance as a decimal; `None` keeps the
    /// loan's coupon (index plus spread for a floating loan). A value is an
    /// all-in fixed rate that replaces the index and spread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_rate: Option<f64>,
    /// Share of the balloon balance that defaults at maturity, as a decimal
    /// in `[0, 1]`; `extension_prob + loss_prob ≤ 1`. Defaults to `0.0`.
    #[serde(default)]
    pub loss_prob: f64,
    /// Loss severity on the defaulted share in percent of its balance
    /// (`40.0` = a 60% recovery). Defaults to `0.0`.
    #[serde(default)]
    pub severity_pct: f64,
    /// Months from maturity until the workout recovery is received; `0`
    /// uses the deal's recovery lag. Defaults to `0`.
    #[serde(default)]
    pub workout_months: u32,
}

impl BalloonSpec {
    /// Validate the extension and loss shares, terms and rate.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `extension_prob` or `loss_prob` is
    /// outside `[0, 1]` or they sum above 1, `extension_months` is zero,
    /// `severity_pct` is outside `[0, 100]`, or the extension rate is
    /// negative or non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        for (value, what) in [
            (self.extension_prob, "extension_prob"),
            (self.loss_prob, "loss_prob"),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(invalid(format!(
                    "balloon {what} ({value}) must be a decimal in [0, 1]"
                )));
            }
        }
        if self.extension_prob + self.loss_prob > 1.0 + 1e-12 {
            return Err(invalid(format!(
                "balloon extension_prob ({}) plus loss_prob ({}) must not exceed 1",
                self.extension_prob, self.loss_prob
            )));
        }
        if self.extension_months == 0 {
            return Err(invalid(
                "balloon extension_months must be positive".to_string(),
            ));
        }
        if !self.severity_pct.is_finite() || !(0.0..=100.0).contains(&self.severity_pct) {
            return Err(invalid(format!(
                "balloon severity_pct ({}) must be a percent in [0, 100]",
                self.severity_pct
            )));
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

/// One step of a step-down prepayment penalty: `pct` applies to
/// prepayments on or before `through`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PenaltyStep {
    /// Last date this step applies.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub through: Date,
    /// Premium in percent of the prepaid balance (`3.0` = 3%).
    pub pct: f64,
}

/// Prepayment protection on a commercial mortgage.
///
/// The premium a prepayment owes is collected by the trust as interest
/// (nothing is charged after `through`); a lockout blocks voluntary
/// prepayment outright.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PrepaymentPenalty {
    /// No voluntary prepayment on or before `through` (`None` locks the
    /// loan out to maturity): the deal's prepayment speed does not apply to
    /// the loan inside the lockout.
    Lockout {
        /// Last date of the lockout; `None` locks out to maturity.
        #[serde(default, with = "finstack_quant_core::wire::optional_date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "Option<finstack_quant_core::wire::DateWire>")
        )]
        through: Option<Date>,
    },
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
    /// A declining schedule of percents (5-4-3-2-1): the first step whose
    /// `through` is on or after the prepayment date applies; nothing after
    /// the last step.
    StepDown {
        /// Steps in ascending `through` order, at least one.
        schedule: Vec<PenaltyStep>,
    },
    /// Yield maintenance: the coupon the trust loses over the remaining
    /// term, `prepaid × max(0, coupon − reinvestment rate)` per year. With
    /// `discount_curve_id` the annual lost coupons are discounted on that
    /// curve (and the reinvestment rate defaults to the curve's zero rate to
    /// maturity); without it the premium is undiscounted at
    /// `reinvestment_rate`. `floor_pct` is the minimum premium in percent of
    /// the prepaid balance (the common 1% floor).
    YieldMaintenance {
        /// Discount curve the lost coupons are valued on; `None` leaves them
        /// undiscounted.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        discount_curve_id: Option<finstack_quant_core::types::CurveId>,
        /// Annual reinvestment rate as a decimal; `None` uses the curve's
        /// zero rate to maturity (then `discount_curve_id` is required).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reinvestment_rate: Option<f64>,
        /// Minimum premium in percent of the prepaid balance; `None` for no
        /// floor.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        floor_pct: Option<f64>,
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
    /// Whether voluntary prepayment is blocked on `date`.
    ///
    /// # Arguments
    ///
    /// * `date` - Payment date of the would-be prepayment.
    #[must_use]
    pub fn locks_out(&self, date: Date) -> bool {
        match self {
            Self::Lockout { through } => through.is_none_or(|through| date <= through),
            _ => false,
        }
    }

    /// Discount curve a yield-maintenance premium is valued on, if any.
    #[must_use]
    pub fn discount_curve_id(&self) -> Option<&finstack_quant_core::types::CurveId> {
        match self {
            Self::YieldMaintenance {
                discount_curve_id, ..
            } => discount_curve_id.as_ref(),
            _ => None,
        }
    }

    /// Premium on a prepayment of `prepaid` made on `date`.
    ///
    /// # Arguments
    ///
    /// * `prepaid` - Principal prepaid in the period, in currency units.
    /// * `coupon` - The loan's annual coupon as a decimal.
    /// * `date` - Payment date of the prepayment.
    /// * `maturity` - The loan's maturity, for the yield-maintenance term.
    /// * `curve` - The yield-maintenance discount curve named by
    ///   `discount_curve_id`, when the penalty names one; ignored otherwise.
    ///
    /// # Returns
    ///
    /// The premium in currency units; `0.0` outside the penalty window, for
    /// a lockout (prepayment does not happen) or when nothing is prepaid.
    ///
    /// # Errors
    ///
    /// Returns an error when a yield-maintenance penalty names a discount
    /// curve that is not supplied, or a discount factor cannot be computed.
    pub fn premium(
        &self,
        prepaid: f64,
        coupon: f64,
        date: Date,
        maturity: Date,
        curve: Option<&finstack_quant_core::market_data::term_structures::DiscountCurve>,
    ) -> finstack_quant_core::Result<f64> {
        if prepaid <= 0.0 {
            return Ok(0.0);
        }
        Ok(match self {
            Self::Lockout { .. } => 0.0,
            Self::Fixed { pct, through } => {
                if through.is_some_and(|through| date > through) {
                    0.0
                } else {
                    prepaid * pct / 100.0
                }
            }
            Self::StepDown { schedule } => schedule
                .iter()
                .find(|step| date <= step.through)
                .map_or(0.0, |step| prepaid * step.pct / 100.0),
            Self::YieldMaintenance {
                discount_curve_id,
                reinvestment_rate,
                floor_pct,
                through,
            } => {
                if through.is_some_and(|through| date > through) || maturity <= date {
                    return Ok(0.0);
                }
                let floor = prepaid * floor_pct.unwrap_or(0.0) / 100.0;
                let years = (maturity - date).whole_days() as f64 / 365.25;
                let curve = match (discount_curve_id, curve) {
                    (Some(id), None) => {
                        return Err(finstack_quant_core::Error::Validation(format!(
                            "yield maintenance names discount curve '{id}' which was not supplied"
                        )))
                    }
                    (Some(_), Some(curve)) => Some(curve),
                    (None, _) => None,
                };
                let reinvestment = match (reinvestment_rate, curve) {
                    (Some(rate), _) => *rate,
                    (None, Some(curve)) => -curve.df_between_dates(date, maturity)?.ln() / years,
                    (None, None) => {
                        return Err(finstack_quant_core::Error::Validation(
                            "yield maintenance needs a reinvestment_rate or a discount_curve_id"
                                .into(),
                        ))
                    }
                };
                let lost_per_year = prepaid * (coupon - reinvestment).max(0.0);
                let premium = match curve {
                    None => lost_per_year * years,
                    Some(curve) => {
                        // Annual lost-coupon installments on each anniversary
                        // up to maturity, the stub prorated at maturity.
                        let mut pv = 0.0;
                        let whole_years = years.floor() as i32;
                        for k in 1..=whole_years {
                            let at = date.add_months(12 * k).min(maturity);
                            pv += lost_per_year * curve.df_between_dates(date, at)?;
                        }
                        let stub = years - f64::from(whole_years);
                        if stub > 0.0 {
                            pv += lost_per_year * stub * curve.df_between_dates(date, maturity)?;
                        }
                        pv
                    }
                };
                premium.max(floor)
            }
        })
    }

    /// Validate the penalty terms.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when a percent, rate or floor is negative
    /// or non-finite, a step-down schedule is empty or not ascending, or a
    /// yield-maintenance penalty has neither a reinvestment rate nor a
    /// discount curve.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        let non_negative = |label: &str, value: f64| {
            if !value.is_finite() || value < 0.0 {
                Err(invalid(format!(
                    "{label} ({value}) must be finite and non-negative"
                )))
            } else {
                Ok(())
            }
        };
        match self {
            Self::Lockout { .. } => Ok(()),
            Self::Fixed { pct, .. } => non_negative("prepayment penalty pct", *pct),
            Self::StepDown { schedule } => {
                if schedule.is_empty() {
                    return Err(invalid(
                        "step-down prepayment penalty needs at least one step".into(),
                    ));
                }
                for pair in schedule.windows(2) {
                    if pair[1].through <= pair[0].through {
                        return Err(invalid(
                            "step-down prepayment penalty steps must be in ascending date order"
                                .into(),
                        ));
                    }
                }
                schedule
                    .iter()
                    .try_for_each(|step| non_negative("step-down prepayment penalty pct", step.pct))
            }
            Self::YieldMaintenance {
                discount_curve_id,
                reinvestment_rate,
                floor_pct,
                ..
            } => {
                if discount_curve_id.is_none() && reinvestment_rate.is_none() {
                    return Err(invalid(
                        "yield maintenance needs a reinvestment_rate or a discount_curve_id".into(),
                    ));
                }
                if let Some(rate) = reinvestment_rate {
                    non_negative("prepayment penalty reinvestment_rate", *rate)?;
                }
                if let Some(floor) = floor_pct {
                    non_negative("prepayment penalty floor_pct", *floor)?;
                }
                Ok(())
            }
        }
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
