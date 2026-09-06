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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MarginCallType {
    /// Initial margin posting requirement
    ///
    /// Collateral to be posted to cover potential future exposure.
    InitialMargin,

    /// Variation margin paid by the desk
    ///
    /// Includes new collateral posted and excess collateral returned.
    VariationMarginPost,

    /// Variation margin received by the desk
    ///
    /// Includes new collateral collected and return of collateral previously posted.
    VariationMarginCollect,

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
            MarginCallType::VariationMarginPost => write!(f, "variation_margin_post"),
            MarginCallType::VariationMarginCollect => write!(f, "variation_margin_collect"),
            MarginCallType::TopUp => write!(f, "top_up"),
            MarginCallType::Substitution => write!(f, "substitution"),
        }
    }
}

impl std::str::FromStr for MarginCallType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "initial_margin" => Ok(MarginCallType::InitialMargin),
            "variation_margin_post" => Ok(MarginCallType::VariationMarginPost),
            "variation_margin_collect" => Ok(MarginCallType::VariationMarginCollect),
            "top_up" => Ok(MarginCallType::TopUp),
            "substitution" => Ok(MarginCallType::Substitution),
            other => Err(format!("Unknown margin call type: {}", other)),
        }
    }
}

/// Margin call event.
///
/// Represents a single margin call with all relevant details for
/// processing and settlement.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MarginCall {
    /// Date the margin call is issued
    #[cfg_attr(feature = "json-schema", schemars(with = "String", extend("format" = "date")))]
    pub call_date: Date,

    /// Settlement date for the margin transfer
    #[cfg_attr(feature = "json-schema", schemars(with = "String", extend("format" = "date")))]
    pub settlement_date: Date,

    /// Type of margin call
    pub call_type: MarginCallType,

    /// Nonnegative transfer amount; call_type specifies the desk cashflow direction.
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
    /// Create a variation margin payment by the desk.
    #[must_use]
    pub fn vm_post(
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
            call_type: MarginCallType::VariationMarginPost,
            amount,
            collateral_type: None,
            mtm_trigger,
            threshold,
            mta_applied: mta,
        }
    }

    /// Create a variation margin collection by the desk.
    #[must_use]
    pub fn vm_collect(
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
            call_type: MarginCallType::VariationMarginCollect,
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
            mtm_trigger: Money::from((0_i64, currency)),
            threshold: Money::from((0_i64, currency)),
            mta_applied: Money::from((0_i64, currency)),
        }
    }

    /// Check if this is a posting call.
    #[must_use]
    pub fn is_post(&self) -> bool {
        matches!(
            self.call_type,
            MarginCallType::InitialMargin
                | MarginCallType::VariationMarginPost
                | MarginCallType::TopUp
        )
    }

    /// Check if the desk receives variation margin.
    #[must_use]
    pub fn is_collect(&self) -> bool {
        matches!(self.call_type, MarginCallType::VariationMarginCollect)
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
            MarginCallType::VariationMarginPost.to_string(),
            "variation_margin_post"
        );
        assert_eq!(
            "variation_margin_collect"
                .parse::<MarginCallType>()
                .expect("canonical margin call type"),
            MarginCallType::VariationMarginCollect
        );
        for noncanonical in ["vm_post", "IM", "topup", "variation-margin-return"] {
            assert!(noncanonical.parse::<MarginCallType>().is_err());
        }
    }

    #[test]
    fn vm_post_call() {
        let call = MarginCall::vm_post(
            test_date(2025, 1, 15),
            test_date(2025, 1, 16),
            Money::from((1_000_000_i64, Currency::USD)),
            Money::from((5_000_000_i64, Currency::USD)),
            Money::from((1_000_000_i64, Currency::USD)),
            Money::from((500_000_i64, Currency::USD)),
        );

        assert!(call.is_post());
        assert!(!call.is_collect());
        assert_eq!(call.call_type, MarginCallType::VariationMarginPost);
        assert_eq!(call.days_to_settle(), 1);
    }

    #[test]
    fn vm_collect_call() {
        let call = MarginCall::vm_collect(
            test_date(2025, 1, 15),
            test_date(2025, 1, 16),
            Money::from((500_000_i64, Currency::USD)),
            Money::from((2_000_000_i64, Currency::USD)),
            Money::from((1_000_000_i64, Currency::USD)),
            Money::from((500_000_i64, Currency::USD)),
        );

        assert!(!call.is_post());
        assert!(call.is_collect());
    }

    #[test]
    fn initial_margin_call() {
        let call = MarginCall::initial_margin(
            test_date(2025, 1, 15),
            test_date(2025, 1, 17),
            Money::from((10_000_000_i64, Currency::USD)),
            Some(CollateralAssetClass::Cash),
        );

        assert!(call.is_post());
        assert_eq!(call.call_type, MarginCallType::InitialMargin);
        assert_eq!(call.collateral_type, Some(CollateralAssetClass::Cash));
        assert_eq!(call.days_to_settle(), 2);
    }
}
