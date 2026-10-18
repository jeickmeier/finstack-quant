//! Canonical long/short position direction shared by futures and forwards.

use serde::{Deserialize, Serialize};

/// Position direction for futures and forwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Position {
    /// Long position (buyer of the contract).
    #[default]
    Long,
    /// Short position (seller of the contract).
    Short,
}

impl Position {
    /// Return `1.0` for long positions and `-1.0` for short positions.
    #[inline]
    pub fn sign(self) -> f64 {
        match self {
            Self::Long => 1.0,
            Self::Short => -1.0,
        }
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Long => write!(f, "long"),
            Self::Short => write!(f, "short"),
        }
    }
}

impl std::str::FromStr for Position {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "long" => Ok(Self::Long),
            "short" => Ok(Self::Short),
            other => Err(format!("Unknown position: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Position;

    #[test]
    fn only_canonical_names_parse_and_roundtrip() {
        assert_eq!("long".parse::<Position>(), Ok(Position::Long));
        assert_eq!("short".parse::<Position>(), Ok(Position::Short));
        for rejected in ["buy", "buyer", "sell", "seller", "Long", "Short"] {
            assert!(rejected.parse::<Position>().is_err());
            assert!(serde_json::from_str::<Position>(&format!("\"{rejected}\"")).is_err());
        }
        assert_eq!(
            serde_json::to_string(&Position::Long).expect("serialize long"),
            "\"long\""
        );
        assert_eq!(
            serde_json::to_string(&Position::Short).expect("serialize short"),
            "\"short\""
        );
    }

    #[test]
    fn default_sign_and_display_are_stable() {
        assert_eq!(Position::default(), Position::Long);
        assert_eq!(Position::Long.sign(), 1.0);
        assert_eq!(Position::Short.sign(), -1.0);
        assert_eq!(Position::Long.to_string(), "long");
        assert_eq!(Position::Short.to_string(), "short");
    }
}
