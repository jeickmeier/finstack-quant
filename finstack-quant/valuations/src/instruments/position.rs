//! Canonical long/short position direction shared by futures and forwards.

use serde::{Deserialize, Serialize};

/// Position direction for futures and forwards.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Position {
    /// Long position (buyer of the contract).
    #[default]
    #[serde(alias = "buy", alias = "buyer")]
    Long,
    /// Short position (seller of the contract).
    #[serde(alias = "sell", alias = "seller")]
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
        match value.to_ascii_lowercase().as_str() {
            "long" | "buy" | "buyer" => Ok(Self::Long),
            "short" | "sell" | "seller" => Ok(Self::Short),
            other => Err(format!("Unknown position: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Position;

    #[test]
    fn aliases_parse_and_roundtrip_to_canonical_names() {
        for alias in ["long", "buy", "buyer"] {
            assert_eq!(alias.parse::<Position>(), Ok(Position::Long));
            let parsed =
                serde_json::from_str::<Position>(&format!("\"{alias}\"")).expect("long alias");
            assert_eq!(parsed, Position::Long);
        }
        for alias in ["short", "sell", "seller"] {
            assert_eq!(alias.parse::<Position>(), Ok(Position::Short));
            let parsed =
                serde_json::from_str::<Position>(&format!("\"{alias}\"")).expect("short alias");
            assert_eq!(parsed, Position::Short);
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

    #[test]
    fn historical_paths_reexport_the_same_type() {
        let ir: crate::instruments::rates::ir_future::Position = Position::Long;
        let commodity: crate::instruments::commodity::commodity_forward::Position = ir;
        let bond: crate::instruments::fixed_income::bond_future::Position = commodity;
        assert_eq!(bond, Position::Long);
    }
}
