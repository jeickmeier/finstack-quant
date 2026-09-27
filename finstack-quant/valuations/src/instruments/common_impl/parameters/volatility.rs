//! Volatility model and volatility-parameter types shared across instruments.

/// Volatility convention (Black lognormal or Bachelier normal) used to price
/// a swaption. Wire values: `black`, `normal`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum VolatilityModel {
    /// Black (lognormal).
    #[default]
    Black,
    /// Bachelier / normal model.
    Normal,
}

impl std::fmt::Display for VolatilityModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VolatilityModel::Black => write!(f, "black"),
            VolatilityModel::Normal => write!(f, "normal"),
        }
    }
}

impl std::str::FromStr for VolatilityModel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "black" => Ok(Self::Black),
            "normal" => Ok(Self::Normal),
            _ => Err(format!(
                "Unknown volatility model: '{}'. Valid: black, normal",
                s
            )),
        }
    }
}
