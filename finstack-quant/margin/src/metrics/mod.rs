//! Margin-specific metrics.
//!
//! This module provides metrics calculators for margin-related analysis:
//!
//! - Margin utilization
//! - Excess collateral
//! - Margin funding cost
//! - Haircut sensitivity (Haircut01)

use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use tracing::warn;

/// Reject implicit cross-currency arithmetic between two [`Money`] operands.
///
/// The workspace forbids implicit cross-currency math: converting requires an
/// explicit [`finstack_quant_core::money::fx::FxProvider`] so the applied policy can be
/// stamped into the result. Mirrors the guard in
/// [`crate::calculators::VmCalculator::calculate`].
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when the two amounts are
/// denominated in different currencies.
fn ensure_same_currency(what: &str, left: Money, right: Money) -> Result<()> {
    if left.currency() == right.currency() {
        return Ok(());
    }
    warn!(metric = what, expected = %left.currency(), got = %right.currency(), "margin metric currency mismatch");
    Err(finstack_quant_core::Error::Validation(format!(
        "{what} currency mismatch: expected {}, got {}",
        left.currency(),
        right.currency()
    )))
}

/// Margin utilization result.
///
/// Ratio of posted margin to required margin.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "MarginUtilizationWire", into = "MarginUtilizationWire")]
pub struct MarginUtilization {
    /// Posted margin amount
    pub posted: Money,
    /// Required margin amount
    pub required: Money,
    /// Utilization ratio (posted / required)
    pub ratio: f64,
}

// Only primitive amounts are serialized. The ratio is derived on decode, so
// positive collateral against zero requirement never writes invalid JSON infinity.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MarginUtilizationWire {
    posted: Money,
    required: Money,
}

impl TryFrom<MarginUtilizationWire> for MarginUtilization {
    type Error = finstack_quant_core::Error;
    fn try_from(value: MarginUtilizationWire) -> Result<Self> {
        Self::new(value.posted, value.required)
    }
}

impl From<MarginUtilization> for MarginUtilizationWire {
    fn from(value: MarginUtilization) -> Self {
        Self {
            posted: value.posted,
            required: value.required,
        }
    }
}

impl MarginUtilization {
    /// Create a new margin utilization result.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when `posted` and
    /// `required` are denominated in different currencies. The ratio is a
    /// same-currency quotient; converting first requires an explicit FX policy
    /// so it cannot be done implicitly here.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_margin::metrics::MarginUtilization;
    ///
    /// let util = MarginUtilization::new(
    ///     Money::from((12_000_000_i64, Currency::USD)),
    ///     Money::from((10_000_000_i64, Currency::USD)),
    /// )?;
    /// assert!(util.is_adequate());
    ///
    /// // Cross-currency inputs are rejected rather than silently mixed.
    /// assert!(MarginUtilization::new(
    ///     Money::from((12_000_000_i64, Currency::EUR)),
    ///     Money::from((10_000_000_i64, Currency::USD)),
    /// )
    /// .is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn new(posted: Money, required: Money) -> Result<Self> {
        ensure_same_currency("margin utilization", posted, required)?;
        if [posted.amount(), required.amount()]
            .iter()
            .any(|x| !x.is_finite() || *x < 0.0)
        {
            return Err(finstack_quant_core::Error::Validation(
                "margin amounts must be finite and nonnegative".into(),
            ));
        }

        let ratio = if required.amount() > 0.0 {
            posted.amount() / required.amount()
        } else if posted.amount() > 0.0 {
            f64::INFINITY
        } else {
            1.0
        };

        Ok(Self {
            posted,
            required,
            ratio,
        })
    }

    /// Check if margin is adequate (ratio >= 1.0).
    #[must_use]
    pub fn is_adequate(&self) -> bool {
        self.ratio >= 1.0
    }

    /// Get the shortfall amount (if any).
    pub fn shortfall(&self) -> finstack_quant_core::Result<Money> {
        if self.ratio < 1.0 {
            Money::new(
                self.required.amount() - self.posted.amount(),
                self.posted.currency(),
            )
        } else {
            Ok(Money::from((0_i64, self.posted.currency())))
        }
    }
}

/// Excess collateral result.
///
/// Amount of collateral above the required level.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExcessCollateral {
    /// Collateral value
    pub collateral_value: Money,
    /// Required value
    pub required_value: Money,
    /// Excess amount (positive) or shortfall (negative)
    pub excess: Money,
}

impl ExcessCollateral {
    /// Create a new excess collateral result.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when
    /// `collateral_value` and `required_value` are denominated in different
    /// currencies. Subtracting across currencies and stamping the difference
    /// with the collateral currency would report arithmetic nonsense as a
    /// currency-tagged amount.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_margin::metrics::ExcessCollateral;
    ///
    /// let excess = ExcessCollateral::new(
    ///     Money::from((105_000_000_i64, Currency::USD)),
    ///     Money::from((100_000_000_i64, Currency::USD)),
    /// )?;
    /// assert!(excess.has_excess());
    ///
    /// // Cross-currency inputs are rejected rather than silently mixed.
    /// assert!(ExcessCollateral::new(
    ///     Money::from((105_000_000_i64, Currency::EUR)),
    ///     Money::from((100_000_000_i64, Currency::USD)),
    /// )
    /// .is_err());
    /// # Ok::<(), finstack_quant_core::Error>(())
    /// ```
    pub fn new(collateral_value: Money, required_value: Money) -> Result<Self> {
        ensure_same_currency("excess collateral", collateral_value, required_value)?;

        let currency = collateral_value.currency();
        let excess = Money::new(
            collateral_value.amount() - required_value.amount(),
            currency,
        )?;

        Ok(Self {
            collateral_value,
            required_value,
            excess,
        })
    }

    /// Check if there is excess collateral.
    #[must_use]
    pub fn has_excess(&self) -> bool {
        self.excess.amount() > 0.0
    }

    /// Check if there is a shortfall.
    #[must_use]
    pub fn has_shortfall(&self) -> bool {
        self.excess.amount() < 0.0
    }

    /// Get the excess percentage.
    #[must_use]
    pub fn excess_percentage(&self) -> f64 {
        if self.required_value.amount() > 0.0 {
            self.excess.amount() / self.required_value.amount()
        } else {
            0.0
        }
    }
}

/// Margin funding cost result.
///
/// Cost of funding posted margin collateral.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MarginFundingCost {
    /// Posted margin amount
    pub margin_posted: Money,
    /// Funding rate (annualized)
    pub funding_rate: f64,
    /// Collateral return rate (e.g., Fed Funds)
    pub collateral_rate: f64,
    /// Net funding cost (annualized)
    pub annual_cost: Money,
}

impl MarginFundingCost {
    /// Calculate margin funding cost.
    ///
    /// # Formula
    ///
    /// ```text
    /// Annual_Cost = Margin × (Funding_Rate - Collateral_Rate)
    /// ```
    pub fn calculate(margin_posted: Money, funding_rate: f64, collateral_rate: f64) -> Self {
        let spread = funding_rate - collateral_rate;
        let annual_cost = margin_posted * spread;

        Self {
            margin_posted,
            funding_rate,
            collateral_rate,
            annual_cost,
        }
    }

    /// Get the funding spread (funding rate - collateral rate).
    #[must_use]
    pub fn spread(&self) -> f64 {
        self.funding_rate - self.collateral_rate
    }

    /// Calculate cost for a specific period.
    pub fn cost_for_period(&self, year_fraction: f64) -> Money {
        self.annual_cost * year_fraction
    }
}

/// Haircut sensitivity (Haircut01) result.
///
/// Change in PV for a 1bp change in haircut.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Haircut01 {
    /// Collateral value
    pub collateral_value: Money,
    /// Current haircut (decimal)
    pub current_haircut: f64,
    /// PV change for +1bp haircut
    pub pv_change: Money,
}

impl Haircut01 {
    /// Calculate Haircut01.
    ///
    /// # Formula
    ///
    /// ```text
    /// Haircut01 = Collateral_Value × 0.0001
    /// ```
    pub fn calculate(collateral_value: Money, current_haircut: f64) -> Self {
        const ONE_BP: f64 = 0.0001;
        let pv_change = collateral_value * ONE_BP;

        Self {
            collateral_value,
            current_haircut,
            pv_change,
        }
    }

    /// Get the haircut in basis points.
    #[must_use]
    pub fn haircut_bp(&self) -> f64 {
        self.current_haircut * 10_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn margin_utilization() {
        let posted = Money::from((12_000_000_i64, Currency::USD));
        let required = Money::from((10_000_000_i64, Currency::USD));
        let util = MarginUtilization::new(posted, required).unwrap();

        assert!(util.is_adequate());
        assert_eq!(util.ratio, 1.2);
        assert_eq!(
            util.shortfall()
                .expect("valid margin shortfall fixture")
                .amount(),
            0.0
        );
    }

    #[test]
    fn margin_shortfall() {
        let posted = Money::from((8_000_000_i64, Currency::USD));
        let required = Money::from((10_000_000_i64, Currency::USD));
        let util = MarginUtilization::new(posted, required).unwrap();

        assert!(!util.is_adequate());
        assert_eq!(util.ratio, 0.8);
        assert_eq!(
            util.shortfall()
                .expect("valid margin shortfall fixture")
                .amount(),
            2_000_000.0
        );
    }

    #[test]
    fn excess_collateral() {
        let collateral = Money::from((105_000_000_i64, Currency::USD));
        let required = Money::from((100_000_000_i64, Currency::USD));
        let excess = ExcessCollateral::new(collateral, required).unwrap();

        assert!(excess.has_excess());
        assert!(!excess.has_shortfall());
        assert_eq!(excess.excess.amount(), 5_000_000.0);
        assert_eq!(excess.excess_percentage(), 0.05);
    }

    #[test]
    fn margin_funding_cost() {
        let margin = Money::from((50_000_000_i64, Currency::USD));
        let funding_rate = 0.055; // 5.5%
        let collateral_rate = 0.053; // 5.3%

        let cost = MarginFundingCost::calculate(margin, funding_rate, collateral_rate);

        assert!((cost.spread() - 0.002).abs() < 1e-10); // 20bp
        assert!((cost.annual_cost.amount() - 100_000.0).abs() < 0.01); // 50M × 0.2%
    }

    #[test]
    fn margin_utilization_rejects_currency_mismatch() {
        let posted = Money::from((12_000_000_i64, Currency::EUR));
        let required = Money::from((10_000_000_i64, Currency::USD));

        let err = MarginUtilization::new(posted, required).unwrap_err();
        assert!(
            matches!(err, finstack_quant_core::Error::Validation(_)),
            "expected a Validation error, got {err:?}"
        );
    }

    #[test]
    fn excess_collateral_rejects_currency_mismatch() {
        let collateral = Money::from((105_000_000_i64, Currency::EUR));
        let required = Money::from((100_000_000_i64, Currency::USD));

        let err = ExcessCollateral::new(collateral, required).unwrap_err();
        assert!(
            matches!(err, finstack_quant_core::Error::Validation(_)),
            "expected a Validation error, got {err:?}"
        );
    }

    #[test]
    fn haircut01() {
        let collateral = Money::from((100_000_000_i64, Currency::USD));
        let h01 = Haircut01::calculate(collateral, 0.02);

        assert_eq!(h01.pv_change.amount(), 10_000.0); // 100M × 0.01%
        assert_eq!(h01.haircut_bp(), 200.0); // 2% = 200bp
    }
}
