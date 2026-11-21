//! NDF quote conventions.

/// Non-Deliverable Forward (NDF) rate quote convention.
///
/// Determines how fixing and contracted forward rates are expressed and settled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum NdfQuoteConvention {
    /// Rate quoted as base currency per settlement currency (e.g., 7.25 CNY per USD).
    /// Settlement = Notional_base × (1/F_fixing - 1/F_contract)
    #[default]
    BasePerSettlement,
    /// Rate quoted as settlement currency per base currency (e.g., 0.138 USD per CNY).
    /// Settlement = Notional_base × (F_fixing - F_contract)
    SettlementPerBase,
}

impl std::fmt::Display for NdfQuoteConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NdfQuoteConvention::BasePerSettlement => write!(f, "base_per_settlement"),
            NdfQuoteConvention::SettlementPerBase => write!(f, "settlement_per_base"),
        }
    }
}

impl std::str::FromStr for NdfQuoteConvention {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "base_per_settlement" => Ok(NdfQuoteConvention::BasePerSettlement),
            "settlement_per_base" => Ok(NdfQuoteConvention::SettlementPerBase),
            _ => Err(format!("Unknown NDF quote convention: {s}")),
        }
    }
}
