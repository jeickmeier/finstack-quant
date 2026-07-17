//! Value types for node data.

use crate::error::{Error, Result};
use crate::types::NodeValueType;
use finstack_quant_core::{currency::Currency, money::Money};
use serde::{Deserialize, Serialize};

/// Value that can be currency-aware or unitless.
///
/// Used for node values that can represent:
/// - **Amount**: Currency-aware monetary values (e.g., USD 1,000,000)
/// - **Scalar**: Unitless values (e.g., ratios, percentages, counts)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AmountOrScalar {
    /// Currency-aware amount
    Amount(Money),
    /// Unitless scalar (ratio, percentage, count, etc.)
    Scalar(f64),
}

impl AmountOrScalar {
    /// Create a currency-aware amount.
    ///
    /// # Arguments
    /// * `value` - Numeric amount
    /// * `currency` - ISO currency of the amount
    pub fn amount(value: f64, currency: Currency) -> Self {
        Self::Amount(Money::new(value, currency))
    }

    /// Create a unitless scalar.
    ///
    /// # Arguments
    /// * `value` - Numeric value interpreted as a unitless scalar
    pub fn scalar(value: f64) -> Self {
        Self::Scalar(value)
    }

    /// Extract the numeric value (ignoring currency if present).
    pub fn value(&self) -> f64 {
        match self {
            Self::Amount(money) => money.amount(),
            Self::Scalar(value) => *value,
        }
    }

    /// Get the currency if this is an amount.
    pub fn currency(&self) -> Option<Currency> {
        match self {
            Self::Amount(money) => Some(money.currency()),
            Self::Scalar(_) => None,
        }
    }

    /// Check if this is a currency-aware amount.
    pub fn is_amount(&self) -> bool {
        matches!(self, Self::Amount(_))
    }

    /// Check if this is a unitless scalar.
    pub fn is_scalar(&self) -> bool {
        matches!(self, Self::Scalar(_))
    }

    /// Get the Money value if this is an amount.
    ///
    /// # Returns
    /// `Some(Money)` if this is a currency-aware amount, `None` if it's a scalar.
    pub fn as_money(&self) -> Option<Money> {
        match self {
            Self::Amount(money) => Some(*money),
            Self::Scalar(_) => None,
        }
    }
}

impl From<f64> for AmountOrScalar {
    fn from(value: f64) -> Self {
        Self::Scalar(value)
    }
}

impl From<Money> for AmountOrScalar {
    fn from(money: Money) -> Self {
        Self::Amount(money)
    }
}

/// Infer a consistent node value type from a series of values.
///
/// Returns:
/// - `Ok(None)` for an empty series
/// - `Ok(Some(NodeValueType::Scalar))` when all values are scalar
/// - `Ok(Some(NodeValueType::Monetary { .. }))` when all values are monetary and share a currency
/// - `Err(..)` when the series mixes scalars and amounts or mixes currencies
///
/// This inference protects one financial-statement node from silently mixing a
/// money series with a ratio/quantity series. It does not convert currencies
/// or apply FX; callers must convert values before constructing the series.
///
/// # Arguments
///
/// * `values` - Iterable of scalar or monetary node values to inspect; a
///   non-empty series must contain one compatible value type and currency.
///
/// # Errors
///
/// Returns a currency-mismatch error when monetary entries use different
/// currencies, or an invalid-input error when scalar and monetary entries are
/// mixed. Empty input is valid and yields `Ok(None)`.
pub fn infer_series_value_type<'a, I>(values: I) -> Result<Option<NodeValueType>>
where
    I: IntoIterator<Item = &'a AmountOrScalar>,
{
    let mut inferred: Option<NodeValueType> = None;

    for value in values {
        match (inferred, value) {
            (None, AmountOrScalar::Scalar(_)) => {
                inferred = Some(NodeValueType::Scalar);
            }
            (None, AmountOrScalar::Amount(money)) => {
                inferred = Some(NodeValueType::Monetary {
                    currency: money.currency(),
                });
            }
            (Some(NodeValueType::Scalar), AmountOrScalar::Scalar(_)) => {}
            (Some(NodeValueType::Monetary { currency }), AmountOrScalar::Amount(money))
                if money.currency() == currency => {}
            (Some(NodeValueType::Monetary { currency }), AmountOrScalar::Amount(money)) => {
                return Err(Error::currency_mismatch(currency, money.currency()));
            }
            (Some(NodeValueType::Scalar), AmountOrScalar::Amount(money)) => {
                return Err(Error::invalid_input(format!(
                    "Mixed scalar and monetary values in one node; found monetary value in {}",
                    money.currency()
                )));
            }
            (Some(NodeValueType::Monetary { currency }), AmountOrScalar::Scalar(_)) => {
                return Err(Error::invalid_input(format!(
                    "Mixed monetary and scalar values in one node; expected currency {}",
                    currency
                )));
            }
        }
    }

    Ok(inferred)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_value() {
        let scalar = AmountOrScalar::scalar(0.15);
        assert_eq!(scalar.value(), 0.15);
        assert!(scalar.is_scalar());
        assert!(!scalar.is_amount());
        assert_eq!(scalar.currency(), None);
    }

    #[test]
    fn test_amount_value() {
        let amount = AmountOrScalar::amount(1_000_000.0, Currency::USD);
        assert_eq!(amount.value(), 1_000_000.0);
        assert!(amount.is_amount());
        assert!(!amount.is_scalar());
        assert_eq!(amount.currency(), Some(Currency::USD));
    }

    #[test]
    fn test_from_f64() {
        let scalar: AmountOrScalar = 42.0.into();
        assert_eq!(scalar.value(), 42.0);
        assert!(scalar.is_scalar());
    }

    #[test]
    fn test_from_money() {
        let money = Money::new(500.0, Currency::EUR);
        let amount: AmountOrScalar = money.into();
        assert_eq!(amount.value(), 500.0);
        assert_eq!(amount.currency(), Some(Currency::EUR));
    }

    #[test]
    fn test_as_money() {
        let amount = AmountOrScalar::amount(1_000_000.0, Currency::USD);
        let money = amount.as_money().expect("amount should convert to money");
        assert_eq!(money.amount(), 1_000_000.0);
        assert_eq!(money.currency(), Currency::USD);

        let scalar = AmountOrScalar::scalar(42.0);
        assert!(scalar.as_money().is_none());
    }

    #[test]
    fn test_infer_series_value_type_rejects_mixed_currencies() {
        let values = [
            AmountOrScalar::amount(100.0, Currency::USD),
            AmountOrScalar::amount(90.0, Currency::EUR),
        ];

        let err = infer_series_value_type(values.iter())
            .expect_err("mixed currencies should be rejected");
        assert!(err.to_string().contains("Currency mismatch"));
    }
}
