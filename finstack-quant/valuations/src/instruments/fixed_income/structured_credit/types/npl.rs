//! Non-performing (NPL) and re-performing (RPL) loan resolution: a loan that
//! pays nothing until a resolution date, where a share liquidates for a
//! proceeds percentage net of carry costs and the rest re-performs at a
//! modified coupon.

use serde::{Deserialize, Serialize};

/// Resolution timeline of a non-performing loan.
///
/// The loan pays no interest or principal until the first payment date at
/// or after `months_to_resolution` months from its origination (the asset's
/// `acquisition_date`, else the deal closing). On that date the share
/// `1 − reperformance_prob` of the balance liquidates: it is booked as a
/// default whose recovery is `(proceeds_pct − carry_cost_pct)%` of the
/// liquidated balance, released on the resolution date itself (the workout
/// is the timeline). The share `reperformance_prob` re-performs: it becomes a
/// performing loan on its original amortization terms at `modified_rate`
/// (the loan's coupon when `None`) from the following period on, with the
/// level payment recast on the re-performing balance.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::structured_credit::LiquidationSpec;
///
/// // Two-year workout: 55% gross proceeds less 5% carry, 20% re-performs at 4%.
/// let spec = LiquidationSpec {
///     months_to_resolution: 24,
///     proceeds_pct: 55.0,
///     carry_cost_pct: 5.0,
///     reperformance_prob: 0.2,
///     modified_rate: Some(0.04),
/// };
/// assert!(spec.validate().is_ok());
/// assert!((spec.net_proceeds_fraction() - 0.5).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct LiquidationSpec {
    /// Months from the loan's origination (`acquisition_date`, else the deal
    /// closing) to the resolution date.
    pub months_to_resolution: u32,
    /// Gross liquidation proceeds as a percent of the liquidated balance.
    pub proceeds_pct: f64,
    /// Carry costs over the workout (taxes, insurance, legal, servicing
    /// advances) as a percent of the liquidated balance, deducted from the
    /// proceeds.
    #[serde(default)]
    pub carry_cost_pct: f64,
    /// Share of the balance that re-performs at resolution instead of
    /// liquidating, as a decimal in `[0, 1]`.
    #[serde(default)]
    pub reperformance_prob: f64,
    /// Annual coupon the re-performing share pays as a decimal; `None` keeps
    /// the loan's coupon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_rate: Option<f64>,
}

impl LiquidationSpec {
    /// Net proceeds of the liquidated balance as a decimal fraction:
    /// `max(0, proceeds_pct − carry_cost_pct) / 100`.
    pub fn net_proceeds_fraction(&self) -> f64 {
        (self.proceeds_pct - self.carry_cost_pct).max(0.0) / 100.0
    }

    /// Validate the timeline, percentages and coupon.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when `months_to_resolution` is zero,
    /// `proceeds_pct` or `carry_cost_pct` is outside `[0, 100]`,
    /// `reperformance_prob` is outside `[0, 1]`, or `modified_rate` is not a
    /// finite non-negative decimal.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        if self.months_to_resolution == 0 {
            return Err(invalid(
                "liquidation months_to_resolution must be at least one month".to_string(),
            ));
        }
        let pct_ok = |value: f64| value.is_finite() && (0.0..=100.0).contains(&value);
        if !pct_ok(self.proceeds_pct) {
            return Err(invalid(format!(
                "liquidation proceeds_pct ({}) must be a percent in [0, 100]",
                self.proceeds_pct
            )));
        }
        if !pct_ok(self.carry_cost_pct) {
            return Err(invalid(format!(
                "liquidation carry_cost_pct ({}) must be a percent in [0, 100]",
                self.carry_cost_pct
            )));
        }
        if !self.reperformance_prob.is_finite() || !(0.0..=1.0).contains(&self.reperformance_prob) {
            return Err(invalid(format!(
                "liquidation reperformance_prob ({}) must be a decimal in [0, 1]",
                self.reperformance_prob
            )));
        }
        if let Some(rate) = self.modified_rate {
            if !rate.is_finite() || rate < 0.0 {
                return Err(invalid(format!(
                    "liquidation modified_rate ({rate}) must be a finite non-negative decimal"
                )));
            }
        }
        Ok(())
    }
}
