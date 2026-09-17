//! Credit-card master-trust portfolio model.
//!
//! Card receivables have no contractual amortization: cardholders pay down a
//! share of the balance every month (the *payment rate*), the portfolio earns
//! finance charges and fees (the *portfolio yield*) rather than a coupon, and
//! losses are charge-offs at an annual rate. With a [`CardPortfolioSpec`] the
//! pool's assets are the investor interest in the receivables: their coupon,
//! prepayment and default assumptions are replaced by the spec, the
//! revolving period (`pool.reinvestment_period`) recycles principal
//! collections into new receivables, and controlled accumulation or early
//! amortization then pays the notes down at the payment rate.

use serde::{Deserialize, Serialize};

/// Portfolio assumptions for a credit-card master trust.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::instruments::fixed_income::structured_credit::CardPortfolioSpec;
///
/// // 15% monthly payment rate, 18% portfolio yield, 5% annual charge-offs.
/// let spec = CardPortfolioSpec::new(0.15, 0.18, 0.05);
/// assert!(spec.validate().is_ok());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CardPortfolioSpec {
    /// Share of the receivables balance paid down each month, as a decimal
    /// (`0.15` = 15% monthly payment rate). Drives principal collections in
    /// place of the prepayment model.
    pub monthly_payment_rate: f64,
    /// Annual portfolio yield (finance charges, interchange and fees) as a
    /// decimal of the receivables balance; replaces the assets' coupons.
    pub portfolio_yield: f64,
    /// Annual charge-off rate as a decimal of the receivables balance;
    /// replaces the default model unless an asset carries `mdr_override`.
    pub charge_off_rate: f64,
}

impl CardPortfolioSpec {
    /// Build a portfolio spec.
    ///
    /// # Arguments
    ///
    /// * `monthly_payment_rate` - Monthly principal payment rate as a decimal
    ///   in `[0, 1]`.
    /// * `portfolio_yield` - Annual portfolio yield as a decimal.
    /// * `charge_off_rate` - Annual charge-off rate as a decimal in `[0, 1]`.
    ///
    /// # Returns
    ///
    /// The spec; call [`validate`](Self::validate) or price the deal to
    /// check the ranges.
    pub fn new(monthly_payment_rate: f64, portfolio_yield: f64, charge_off_rate: f64) -> Self {
        Self {
            monthly_payment_rate,
            portfolio_yield,
            charge_off_rate,
        }
    }

    /// Validate the ranges.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the payment or charge-off rate is
    /// outside `[0, 1]` or the yield is negative or non-finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let invalid = |msg: String| finstack_quant_core::Error::Validation(msg);
        let unit = |value: f64| value.is_finite() && (0.0..=1.0).contains(&value);
        if !unit(self.monthly_payment_rate) {
            return Err(invalid(format!(
                "card monthly_payment_rate ({}) must be a decimal in [0, 1]",
                self.monthly_payment_rate
            )));
        }
        if !unit(self.charge_off_rate) {
            return Err(invalid(format!(
                "card charge_off_rate ({}) must be a decimal in [0, 1]",
                self.charge_off_rate
            )));
        }
        if !self.portfolio_yield.is_finite() || self.portfolio_yield < 0.0 {
            return Err(invalid(format!(
                "card portfolio_yield ({}) must be finite and non-negative",
                self.portfolio_yield
            )));
        }
        Ok(())
    }
}
