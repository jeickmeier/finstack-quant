//! Canonical barrier classification, direction and hit-payment timing.

use serde::{Deserialize, Serialize};

/// Four-state barrier option classification.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BarrierType {
    /// Up-and-out: knocked out when spot touches or rises above the barrier.
    #[default]
    UpAndOut,
    /// Up-and-in: activated when spot touches or rises above the barrier.
    UpAndIn,
    /// Down-and-out: knocked out when spot touches or falls below the barrier.
    DownAndOut,
    /// Down-and-in: activated when spot touches or falls below the barrier.
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

    /// Return the side of spot on which the barrier sits.
    pub fn direction(self) -> BarrierDirection {
        if self.is_up() {
            BarrierDirection::Up
        } else {
            BarrierDirection::Down
        }
    }
}

/// Side of spot on which a barrier sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BarrierDirection {
    /// Barrier is above spot: hit when spot touches or rises above it.
    Up,
    /// Barrier is below spot: hit when spot touches or falls below it.
    Down,
}

/// When a barrier-triggered cash amount is paid.
///
/// Used for a knock-out rebate (`rebate_timing`) and for a one-touch payout
/// (`payout_timing`). At-expiry payments are discounted from expiry; at-hit
/// payments are discounted from the first-passage time. Knock-in rebates
/// always pay at expiry (only then is it known that no hit occurred), so this
/// setting does not affect them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PayoutTiming {
    /// Paid the moment the barrier is hit (market standard).
    #[default]
    AtHit,
    /// Paid at expiry regardless of when the barrier is hit.
    AtExpiry,
}

#[cfg(test)]
mod tests {
    use super::BarrierType;

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
            assert!(serde_json::from_str::<BarrierType>(&format!("\"{rejected}\"")).is_err());
        }
    }
}
