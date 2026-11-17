//! The state of one statement cell: a node in a period.

use finstack_quant_core::money::Money;
use serde::{Deserialize, Serialize};

/// Why a cell has no value although its node and period are valid.
///
/// An unavailable cell is not an error: the model is well formed and the run
/// continues. Invalid arithmetic, an invalid mask and unrepresentable money
/// are errors instead and fail the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    /// The node's `where` predicate had no value for the period, so whether
    /// the cell is masked is unknown.
    MissingMask,
    /// The explicit observation was published after the observation cutoff
    /// and the node has no forecast or formula for the period.
    HiddenByCutoff,
    /// The formula or forecast that supplies the cell had no defined result,
    /// because an input it needs is missing, masked or itself unavailable.
    MissingInput,
    /// A change, growth rate or ratio has no defined value for its base: an
    /// exactly zero base for a percentage change, a non-positive base or a
    /// negative ending value for a growth rate, and a non-positive base for
    /// `ratio`.
    UndefinedBase,
}

impl UnavailableReason {
    /// Stable snake_case label, identical to the serde wire form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingMask => "missing_mask",
            Self::HiddenByCutoff => "hidden_by_cutoff",
            Self::MissingInput => "missing_input",
            Self::UndefinedBase => "undefined_base",
        }
    }
}

/// One evaluated cell.
///
/// A zero is a value: [`Cell::Scalar`]`(0.0)` or a zero [`Cell::Money`]. A
/// cell without a value says why it has none, and is never read as zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Cell {
    /// A finite unitless value.
    Scalar(f64),
    /// An exact amount in the node's currency.
    Money(Money),
    /// No observation, forecast or formula supplies the period.
    Missing,
    /// The node's `where` predicate is false for the period.
    Masked,
    /// The cell has no value for the stated reason.
    Unavailable(UnavailableReason),
}

impl Cell {
    /// The cell's value as a float, or `None` when it has no value.
    pub(crate) fn numeric(self) -> Option<f64> {
        match self {
            Self::Scalar(value) => Some(value),
            Self::Money(money) => Some(money.amount()),
            Self::Missing | Self::Masked | Self::Unavailable(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;

    #[test]
    fn a_zero_is_a_value_and_other_states_have_none() {
        assert_eq!(Cell::Scalar(0.0).numeric(), Some(0.0));
        assert_eq!(Cell::Money(Money::zero(Currency::USD)).numeric(), Some(0.0));
        for cell in [
            Cell::Missing,
            Cell::Masked,
            Cell::Unavailable(UnavailableReason::MissingMask),
        ] {
            assert_eq!(cell.numeric(), None);
        }
    }

    #[test]
    fn reason_labels_match_the_wire_form() {
        for reason in [
            UnavailableReason::MissingMask,
            UnavailableReason::HiddenByCutoff,
            UnavailableReason::MissingInput,
        ] {
            assert_eq!(
                serde_json::to_value(reason).expect("serialize"),
                serde_json::Value::String(reason.as_str().to_string())
            );
        }
    }
}
