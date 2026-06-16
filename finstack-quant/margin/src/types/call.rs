//! Margin call event types.
//!
//! Defines margin call events and their classification.

use super::collateral::CollateralAssetClass;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use std::fmt;

/// Type of margin call.
///
/// Classifies the nature of a margin call for proper processing
/// and accounting treatment.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum MarginCallType {
    /// Initial margin posting requirement
    ///
    /// Collateral to be posted to cover potential future exposure.
    InitialMargin,

    /// Variation margin delivery (margin to be posted)
    ///
    /// Mark-to-market payment when exposure has increased.
    VariationMarginDelivery,

    /// Variation margin return (margin to be received back)
    ///
    /// Return of excess collateral when exposure has decreased.
    VariationMarginReturn,

    /// Top-up margin call
    ///
    /// Additional IM required due to increased exposure or threshold breach.
    TopUp,

    /// Collateral substitution request
    ///
    /// Request to substitute one form of eligible collateral for another.
    Substitution,
}

impl fmt::Display for MarginCallType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarginCallType::InitialMargin => write!(f, "initial_margin"),
            MarginCallType::VariationMarginDelivery => write!(f, "vm_delivery"),
            MarginCallType::VariationMarginReturn => write!(f, "vm_return"),
            MarginCallType::TopUp => write!(f, "top_up"),
            MarginCallType::Substitution => write!(f, "substitution"),
        }
    }
}

impl std::str::FromStr for MarginCallType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().replace('-', "_").as_str() {
            "initial_margin" | "im" => Ok(MarginCallType::InitialMargin),
            "vm_delivery" | "vmdelivery" | "variation_margin_delivery" => {
                Ok(MarginCallType::VariationMarginDelivery)
            }
            "vm_return" | "vmreturn" | "variation_margin_return" => {
                Ok(MarginCallType::VariationMarginReturn)
            }
            "top_up" | "topup" => Ok(MarginCallType::TopUp),
            "substitution" | "sub" => Ok(MarginCallType::Substitution),
            other => Err(format!("Unknown margin call type: {}", other)),
        }
    }
}

/// Margin call event.
///
/// Represents a single margin call with all relevant details for
/// processing and settlement.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarginCall {
    /// Date the margin call is issued
    #[schemars(with = "String")]
    pub call_date: Date,

    /// Settlement date for the margin transfer
    #[schemars(with = "String")]
    pub settlement_date: Date,

    /// Type of margin call
    pub call_type: MarginCallType,

    /// Amount of margin required (positive = delivery, negative = return)
    pub amount: Money,

    /// Specific collateral type requested (if applicable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collateral_type: Option<CollateralAssetClass>,

    /// Mark-to-market value that triggered the call
    pub mtm_trigger: Money,

    /// Threshold in effect at time of call
    pub threshold: Money,

    /// MTA applied (may have reduced the call amount)
    pub mta_applied: Money,
}

impl MarginCall {
    /// Create a new variation margin delivery call.
    #[must_use]
    pub fn vm_delivery(
        call_date: Date,
        settlement_date: Date,
        amount: Money,
        mtm_trigger: Money,
        threshold: Money,
        mta: Money,
    ) -> Self {
        Self {
            call_date,
            settlement_date,
            call_type: MarginCallType::VariationMarginDelivery,
            amount,
            collateral_type: None,
            mtm_trigger,
            threshold,
            mta_applied: mta,
        }
    }

    /// Create a new variation margin return call.
    #[must_use]
    pub fn vm_return(
        call_date: Date,
        settlement_date: Date,
        amount: Money,
        mtm_trigger: Money,
        threshold: Money,
        mta: Money,
    ) -> Self {
        Self {
            call_date,
            settlement_date,
            call_type: MarginCallType::VariationMarginReturn,
            amount,
            collateral_type: None,
            mtm_trigger,
            threshold,
            mta_applied: mta,
        }
    }

    /// Create a new initial margin call.
    #[must_use]
    pub fn initial_margin(
        call_date: Date,
        settlement_date: Date,
        amount: Money,
        collateral_type: Option<CollateralAssetClass>,
    ) -> Self {
        let currency = amount.currency();
        Self {
            call_date,
            settlement_date,
            call_type: MarginCallType::InitialMargin,
            amount,
            collateral_type,
            mtm_trigger: Money::new(0.0, currency),
            threshold: Money::new(0.0, currency),
            mta_applied: Money::new(0.0, currency),
        }
    }

    /// Check if this is a delivery (posting) call.
    #[must_use]
    pub fn is_delivery(&self) -> bool {
        matches!(
            self.call_type,
            MarginCallType::InitialMargin
                | MarginCallType::VariationMarginDelivery
                | MarginCallType::TopUp
        )
    }

    /// Check if this is a return call.
    #[must_use]
    pub fn is_return(&self) -> bool {
        matches!(self.call_type, MarginCallType::VariationMarginReturn)
    }

    /// Get the number of business days until settlement.
    #[must_use]
    pub fn days_to_settle(&self) -> i64 {
        (self.settlement_date - self.call_date).whole_days()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use time::Month;

    fn test_date(y: i32, m: u8, d: u8) -> Date {
        Date::from_calendar_date(y, Month::try_from(m).expect("valid month"), d)
            .expect("valid date")
    }

    #[test]
    fn margin_call_type_display() {
        assert_eq!(MarginCallType::InitialMargin.to_string(), "initial_margin");
        assert_eq!(
            MarginCallType::VariationMarginDelivery.to_string(),
            "vm_delivery"
        );
    }

    #[test]
    fn vm_delivery_call() {
        let call = MarginCall::vm_delivery(
            test_date(2025, 1, 15),
            test_date(2025, 1, 16),
            Money::new(1_000_000.0, Currency::USD),
            Money::new(5_000_000.0, Currency::USD),
            Money::new(1_000_000.0, Currency::USD),
            Money::new(500_000.0, Currency::USD),
        );

        assert!(call.is_delivery());
        assert!(!call.is_return());
        assert_eq!(call.call_type, MarginCallType::VariationMarginDelivery);
        assert_eq!(call.days_to_settle(), 1);
    }

    #[test]
    fn vm_return_call() {
        let call = MarginCall::vm_return(
            test_date(2025, 1, 15),
            test_date(2025, 1, 16),
            Money::new(500_000.0, Currency::USD),
            Money::new(2_000_000.0, Currency::USD),
            Money::new(1_000_000.0, Currency::USD),
            Money::new(500_000.0, Currency::USD),
        );

        assert!(!call.is_delivery());
        assert!(call.is_return());
    }

    #[test]
    fn initial_margin_call() {
        let call = MarginCall::initial_margin(
            test_date(2025, 1, 15),
            test_date(2025, 1, 17),
            Money::new(10_000_000.0, Currency::USD),
            Some(CollateralAssetClass::Cash),
        );

        assert!(call.is_delivery());
        assert_eq!(call.call_type, MarginCallType::InitialMargin);
        assert_eq!(call.collateral_type, Some(CollateralAssetClass::Cash));
        assert_eq!(call.days_to_settle(), 2);
    }
}
