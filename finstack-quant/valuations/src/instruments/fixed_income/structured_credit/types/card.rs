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
//!
//! Once the revolving period ends the investor's allocation of trust
//! collections is fixed: the seller replenishes the receivables, so the
//! trust balance stays level while the investor interest amortizes, and the
//! investor keeps receiving its frozen share of principal collections,
//! finance charges and charge-offs.

use finstack_quant_core::money::Money;
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
    /// Seller's interest in the trust receivables at the deal's valuation
    /// date, in the pool currency. The pool is the investor interest; the
    /// trust receivables are the two together. `None` means no seller
    /// interest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seller_interest: Option<Money>,
    /// Investor allocation of trust collections fixed at the end of the
    /// revolving period, as a decimal in `(0, 1]`. `None` freezes the
    /// allocation at the investor's floating share
    /// `investor_interest / (investor_interest + seller_interest)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_allocation_pct: Option<f64>,
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
            seller_interest: None,
            fixed_allocation_pct: None,
        }
    }

    /// Set the seller's interest in the trust receivables.
    ///
    /// # Arguments
    ///
    /// * `seller_interest` - Seller's share of the receivables at the
    ///   valuation date, in the pool currency; non-negative.
    pub fn with_seller_interest(mut self, seller_interest: Money) -> Self {
        self.seller_interest = Some(seller_interest);
        self
    }

    /// Fix the investor allocation applied after the revolving period.
    ///
    /// # Arguments
    ///
    /// * `fixed_allocation_pct` - Investor share of trust collections as a
    ///   decimal in `(0, 1]`, replacing the floating share at the freeze.
    pub fn with_fixed_allocation_pct(mut self, fixed_allocation_pct: f64) -> Self {
        self.fixed_allocation_pct = Some(fixed_allocation_pct);
        self
    }

    /// Per-asset investor flow base once the allocation is fixed.
    ///
    /// The trust receivables at the freeze are the investor interest
    /// (`balances`) plus the seller interest, spread over the pool's lines
    /// pro rata; from then on the investor's principal collections, finance
    /// charges and charge-offs are the fixed allocation of the flows on
    /// those level receivables.
    ///
    /// # Arguments
    ///
    /// * `balances` - Live per-asset investor balances at the freeze, in
    ///   the pool currency.
    /// * `currency` - Pool currency; the seller interest must match it.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the seller interest is in another
    /// currency.
    pub fn investor_flow_base(
        &self,
        balances: &[f64],
        currency: finstack_quant_core::currency::Currency,
    ) -> finstack_quant_core::Result<Vec<f64>> {
        let seller = match self.seller_interest {
            Some(seller) if seller.currency() != currency => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "card seller_interest currency {} must match the pool currency {currency}",
                    seller.currency()
                )));
            }
            Some(seller) => seller.amount().max(0.0),
            None => 0.0,
        };
        let investor: f64 = balances.iter().filter(|b| **b > 0.0).sum();
        if investor <= 0.0 {
            return Ok(balances.to_vec());
        }
        let trust = investor + seller;
        let allocation = self.fixed_allocation_pct.unwrap_or(investor / trust);
        // Each line's share of the level trust receivables times the fixed allocation.
        Ok(balances
            .iter()
            .map(|balance| balance.max(0.0) / investor * trust * allocation)
            .collect())
    }

    /// Validate the ranges.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` when the payment or charge-off rate is
    /// outside `[0, 1]`, the yield is negative or non-finite, the seller
    /// interest is negative, or the fixed allocation is outside `(0, 1]`.
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
        if let Some(seller) = self.seller_interest {
            if !seller.amount().is_finite() || seller.amount() < 0.0 {
                return Err(invalid(format!(
                    "card seller_interest ({seller}) must be finite and non-negative"
                )));
            }
        }
        if let Some(pct) = self.fixed_allocation_pct {
            if !pct.is_finite() || pct <= 0.0 || pct > 1.0 {
                return Err(invalid(format!(
                    "card fixed_allocation_pct ({pct}) must be a decimal in (0, 1]"
                )));
            }
        }
        Ok(())
    }
}
