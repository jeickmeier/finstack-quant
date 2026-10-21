//! Principal-exchange policy and explicit principal events for
//! [`CashFlowBuilder`](crate::builder::CashFlowBuilder).

use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;

/// Whether the builder emits issue-funding and maturity-redemption notionals.
///
/// Outstanding still starts at the configured initial principal for coupon
/// math. Scheduled [`AmortizationSpec`](super::AmortizationSpec) payments and
/// explicit principal events still emit. This is not the cross-currency
/// `NotionalExchange` policy (no final-only or MTM-resetting variants).
///
/// # Variants
///
/// - **`None`**: track outstanding only; no `CFKind::Notional` issue or
///   redemption flows. Vanilla IRS and basis swaps use this.
/// - **`InitialAndFinal`** (default): emit issue funding and the maturity
///   balloon on the lagged redemption date.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_cashflows::builder::PrincipalExchange;
///
/// assert_eq!(
///     PrincipalExchange::default(),
///     PrincipalExchange::InitialAndFinal
/// );
/// ```
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalExchange {
    /// Do not emit issue or redemption `CFKind::Notional` flows.
    None,
    /// Emit issue funding and maturity redemption (default).
    #[default]
    InitialAndFinal,
}

impl PrincipalExchange {
    /// Returns `true` for the default [`Self::InitialAndFinal`] variant.
    #[must_use]
    pub fn is_initial_and_final(&self) -> bool {
        matches!(self, Self::InitialAndFinal)
    }
}

/// JSON representation of an explicit principal event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PrincipalEventSpec {
    /// Event date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub date: Date,
    /// Cash settlement date, independently adjusted from the economic event date.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub payment_date: Date,
    /// Outstanding balance delta. Positive increases outstanding, negative repays.
    pub delta: Money,
    /// Optional settlement amount before classification-dependent sign handling.
    /// When omitted, amortization repayments use `-delta` and draws use `delta`.
    /// Amortization emits this amount as a receipt; other kinds negate it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cash: Option<Money>,
    /// Cashflow classification to emit.
    pub kind: CFKind,
}

impl PrincipalEventSpec {
    /// Settlement amount before classification-dependent sign handling.
    ///
    /// When `cash` is omitted, amortization repayments use `-delta` and all
    /// other kinds use `delta`.
    #[must_use]
    pub(crate) fn resolved_cash(&self) -> Money {
        self.cash.unwrap_or(match self.kind {
            CFKind::Amortization => self.delta.checked_neg(),
            _ => self.delta,
        })
    }
}
