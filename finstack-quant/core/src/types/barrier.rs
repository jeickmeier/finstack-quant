//! Canonical barrier classification, direction and hit-payment timing.
//!
//! These closed enums carry no level, spot, amount or monitoring state and
//! execute no comparison. They have no `Default`: every product that uses them
//! must select its classification and timing explicitly.

use serde::{Deserialize, Serialize};

/// Four-state barrier option classification.
///
/// In/out describes whether a qualifying hit deactivates or activates the
/// option. It does not describe payment entitlement or the option's current
/// state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BarrierType {
    /// Up-and-out: knocked out by an eligible observation at or above the barrier.
    UpAndOut,
    /// Up-and-in: activated by an eligible observation at or above the barrier.
    UpAndIn,
    /// Down-and-out: knocked out by an eligible observation at or below the barrier.
    DownAndOut,
    /// Down-and-in: activated by an eligible observation at or below the barrier.
    DownAndIn,
}

impl BarrierType {
    /// Return whether the barrier deactivates the option when touched.
    pub fn is_knock_out(self) -> bool {
        matches!(self, Self::UpAndOut | Self::DownAndOut)
    }

    /// Return whether the barrier activates the option when touched.
    pub fn is_knock_in(self) -> bool {
        !self.is_knock_out()
    }

    /// Return whether this is an up barrier.
    pub fn is_up(self) -> bool {
        matches!(self, Self::UpAndOut | Self::UpAndIn)
    }

    /// Return the inequality sense of the barrier condition.
    pub fn direction(self) -> BarrierDirection {
        if self.is_up() {
            BarrierDirection::Up
        } else {
            BarrierDirection::Down
        }
    }
}

/// Inequality sense of an inclusive barrier touch.
///
/// At a contractually eligible observation `x` of barrier `H` (finite, same
/// units), `Up` is hit when `x >= H` and `Down` when `x <= H`. Equality is a
/// hit. The direction does not assert where the barrier sits relative to
/// current spot: an observation already at or beyond the barrier is a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BarrierDirection {
    /// Hit when an eligible observation is at or above the barrier.
    Up,
    /// Hit when an eligible observation is at or below the barrier.
    Down,
}

/// When an otherwise entitled barrier-conditional cash amount is paid.
///
/// The timing schedules a payment; it does not grant entitlement, choose the
/// amount or currency, or set a settlement lag, curve or clock. Each product
/// validates which timings its payoff supports. For example, a no-hit
/// (knock-in) rebate is only determined at expiry, so products reject
/// `AtHit` for it rather than silently replacing the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PayoutTiming {
    /// Paid at the first qualifying recognized hit.
    AtHit,
    /// Paid at the product's specified expiry.
    AtExpiry,
}

#[cfg(test)]
mod tests {
    use super::{BarrierDirection, BarrierType, PayoutTiming};
    use serde::de::DeserializeOwned;

    fn assert_rejects<T: DeserializeOwned + std::fmt::Debug>(json: &str) {
        assert!(
            serde_json::from_str::<T>(json).is_err(),
            "expected rejection of {json}"
        );
    }

    #[test]
    fn serde_accepts_only_canonical_snake_case() {
        for (variant, canonical, rejected) in [
            (BarrierType::UpAndOut, "up_and_out", "UpAndOut"),
            (BarrierType::UpAndIn, "up_and_in", "UpAndIn"),
            (BarrierType::DownAndOut, "down_and_out", "DownAndOut"),
            (BarrierType::DownAndIn, "down_and_in", "DownAndIn"),
        ] {
            assert_eq!(
                serde_json::to_string(&variant).expect("serialize barrier"),
                format!("\"{canonical}\"")
            );
            assert_eq!(
                serde_json::from_str::<BarrierType>(&format!("\"{canonical}\""))
                    .expect("canonical"),
                variant
            );
            assert_rejects::<BarrierType>(&format!("\"{rejected}\""));
            assert_rejects::<BarrierType>(&format!("\" {canonical}\""));
            assert_rejects::<BarrierType>(&format!("\"{}\"", canonical.to_uppercase()));
        }
        assert_rejects::<BarrierType>("0");
        assert_rejects::<BarrierType>("null");
    }

    #[test]
    fn direction_and_timing_wire_labels_are_exact() {
        for (variant, canonical) in [
            (BarrierDirection::Up, "up"),
            (BarrierDirection::Down, "down"),
        ] {
            assert_eq!(
                serde_json::to_string(&variant).expect("serialize direction"),
                format!("\"{canonical}\"")
            );
            assert_eq!(
                serde_json::from_str::<BarrierDirection>(&format!("\"{canonical}\""))
                    .expect("canonical"),
                variant
            );
            assert_rejects::<BarrierDirection>(&format!("\"{}\"", canonical.to_uppercase()));
            assert_rejects::<BarrierDirection>(&format!("\"{canonical} \""));
        }
        for (variant, canonical) in [
            (PayoutTiming::AtHit, "at_hit"),
            (PayoutTiming::AtExpiry, "at_expiry"),
        ] {
            assert_eq!(
                serde_json::to_string(&variant).expect("serialize timing"),
                format!("\"{canonical}\"")
            );
            assert_eq!(
                serde_json::from_str::<PayoutTiming>(&format!("\"{canonical}\""))
                    .expect("canonical"),
                variant
            );
            assert_rejects::<PayoutTiming>(&format!("\"{}\"", canonical.to_uppercase()));
            assert_rejects::<PayoutTiming>(&format!("\" {canonical}\""));
        }
        for json in ["0", "null", "\"AtHit\"", "\"hit\""] {
            assert_rejects::<BarrierDirection>(json);
            assert_rejects::<PayoutTiming>(json);
        }
    }

    #[test]
    fn predicates_match_accepted_truth_table() {
        for (variant, knock_out, up, direction) in [
            (BarrierType::UpAndOut, true, true, BarrierDirection::Up),
            (BarrierType::UpAndIn, false, true, BarrierDirection::Up),
            (BarrierType::DownAndOut, true, false, BarrierDirection::Down),
            (BarrierType::DownAndIn, false, false, BarrierDirection::Down),
        ] {
            assert_eq!(variant.is_knock_out(), knock_out);
            assert_eq!(variant.is_knock_in(), !knock_out);
            assert_eq!(variant.is_up(), up);
            assert_eq!(variant.direction(), direction);
        }
    }
}
