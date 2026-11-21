//! Market parameter types for instrument pricing.

use serde::{Deserialize, Serialize};

/// Settlement type for options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SettlementType {
    /// Physical delivery
    Physical,
    /// Cash settlement
    Cash,
}

impl std::fmt::Display for SettlementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettlementType::Physical => write!(f, "physical"),
            SettlementType::Cash => write!(f, "cash"),
        }
    }
}

impl std::str::FromStr for SettlementType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "physical" => Ok(SettlementType::Physical),
            "cash" => Ok(SettlementType::Cash),
            _ => Err(format!("Unknown settlement type: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_models::types::{ExerciseStyle, OptionType};

    #[test]
    fn enum_parsing_and_display_use_canonical_values() {
        assert_eq!(OptionType::Call.to_string(), "call");
        assert_eq!("call".parse::<OptionType>(), Ok(OptionType::Call));
        assert_eq!("put".parse::<OptionType>(), Ok(OptionType::Put));
        assert!("buy".parse::<OptionType>().is_err());
        assert!("sell_protection".parse::<OptionType>().is_err());
        assert!("weird".parse::<OptionType>().is_err());

        assert_eq!(ExerciseStyle::default(), ExerciseStyle::European);
        assert_eq!(
            "american".parse::<ExerciseStyle>(),
            Ok(ExerciseStyle::American)
        );
        assert_eq!(ExerciseStyle::Bermudan.to_string(), "bermudan");
        assert!("odd".parse::<ExerciseStyle>().is_err());

        assert_eq!(SettlementType::Cash.to_string(), "cash");
        assert_eq!(
            "physical".parse::<SettlementType>(),
            Ok(SettlementType::Physical)
        );
        assert!("gross".parse::<SettlementType>().is_err());
    }
}
