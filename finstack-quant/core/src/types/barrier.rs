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

impl std::fmt::Display for BarrierType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UpAndOut => write!(f, "up_and_out"),
            Self::UpAndIn => write!(f, "up_and_in"),
            Self::DownAndOut => write!(f, "down_and_out"),
            Self::DownAndIn => write!(f, "down_and_in"),
        }
    }
}

impl std::str::FromStr for BarrierType {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "up_and_out" => Ok(Self::UpAndOut),
            "up_and_in" => Ok(Self::UpAndIn),
            "down_and_out" => Ok(Self::DownAndOut),
            "down_and_in" => Ok(Self::DownAndIn),
            other => Err(format!(
                "Unknown barrier type: '{other}'. Valid: up_and_in, up_and_out, down_and_in, down_and_out"
            )),
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

impl std::fmt::Display for BarrierDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Up => write!(f, "up"),
            Self::Down => write!(f, "down"),
        }
    }
}

impl std::str::FromStr for BarrierDirection {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            other => Err(format!(
                "Unknown barrier direction: '{other}'. Valid: up, down"
            )),
        }
    }
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

impl std::fmt::Display for PayoutTiming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AtHit => write!(f, "at_hit"),
            Self::AtExpiry => write!(f, "at_expiry"),
        }
    }
}

impl std::str::FromStr for PayoutTiming {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "at_hit" => Ok(Self::AtHit),
            "at_expiry" => Ok(Self::AtExpiry),
            other => Err(format!(
                "Unknown payout timing: '{other}'. Valid: at_hit, at_expiry"
            )),
        }
    }
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
