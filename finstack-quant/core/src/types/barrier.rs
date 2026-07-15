//! Canonical barrier direction and activation classification.

use serde::{Deserialize, Serialize};

/// Four-state barrier option classification.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BarrierType {
    /// Up-and-out: knocked out when spot touches or rises above the barrier.
    #[default]
    #[serde(alias = "UpAndOut")]
    UpAndOut,
    /// Up-and-in: activated when spot touches or rises above the barrier.
    #[serde(alias = "UpAndIn")]
    UpAndIn,
    /// Down-and-out: knocked out when spot touches or falls below the barrier.
    #[serde(alias = "DownAndOut")]
    DownAndOut,
    /// Down-and-in: activated when spot touches or falls below the barrier.
    #[serde(alias = "DownAndIn")]
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
        let normalized = value
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "up_and_out" | "upandout" => Ok(Self::UpAndOut),
            "up_and_in" | "upandin" => Ok(Self::UpAndIn),
            "down_and_out" | "downandout" => Ok(Self::DownAndOut),
            "down_and_in" | "downandin" => Ok(Self::DownAndIn),
            other => Err(format!(
                "Unknown barrier type: '{other}'. Valid: up_and_in, up_and_out, down_and_in, down_and_out"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BarrierType;

    #[test]
    fn serde_emits_snake_case_and_accepts_legacy_pascal_case() {
        for (variant, canonical, legacy) in [
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
                serde_json::from_str::<BarrierType>(&format!("\"{legacy}\""))
                    .expect("legacy barrier"),
                variant
            );
        }
    }
}
