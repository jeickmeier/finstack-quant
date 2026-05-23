use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Unique identifier for a risk factor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FactorId(String);

impl FactorId {
    /// Create a factor identifier from any string-like value.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the underlying factor identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FactorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Broad classification of a risk factor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FactorType {
    /// Interest-rate factor.
    Rates,
    /// Credit-spread or hazard factor.
    Credit,
    /// Equity price factor.
    Equity,
    /// Foreign-exchange factor.
    FX,
    /// Volatility factor.
    Volatility,
    /// Commodity factor.
    Commodity,
    /// Inflation factor.
    Inflation,
    /// User-defined factor bucket.
    Custom(String),
}

impl fmt::Display for FactorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rates => write!(f, "rates"),
            Self::Credit => write!(f, "credit"),
            Self::Equity => write!(f, "equity"),
            Self::FX => write!(f, "fx"),
            Self::Volatility => write!(f, "volatility"),
            Self::Commodity => write!(f, "commodity"),
            Self::Inflation => write!(f, "inflation"),
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

impl FromStr for FactorType {
    type Err = finstack_core::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let n = crate::parse::normalize_label(s);
        if n.starts_with("custom:") || n.starts_with("custom_") {
            let name = s
                .split_once(':')
                .or_else(|| s.split_once('_'))
                .map(|(_, v)| v.trim())
                .unwrap_or("");
            return Ok(Self::Custom(name.to_string()));
        }
        match n.as_str() {
            "rates" | "rate" | "ir" => Ok(Self::Rates),
            "credit" => Ok(Self::Credit),
            "equity" => Ok(Self::Equity),
            "fx" => Ok(Self::FX),
            "volatility" | "vol" => Ok(Self::Volatility),
            "commodity" => Ok(Self::Commodity),
            "inflation" => Ok(Self::Inflation),
            _ => Err(finstack_core::InputError::Invalid.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_parses_to(label: &str, expected: FactorType) {
        assert!(matches!(label.parse::<FactorType>(), Ok(value) if value == expected));
    }

    #[test]
    fn test_factor_id_from_string() {
        let id = FactorId::new("USD-Rates");
        assert_eq!(id.as_str(), "USD-Rates");
    }

    #[test]
    fn test_factor_id_equality() {
        let a = FactorId::new("USD-Rates");
        let b = FactorId::new("USD-Rates");
        assert_eq!(a, b);
    }

    #[test]
    fn test_factor_id_display() {
        let id = FactorId::new("NA-Energy-CCC");
        assert_eq!(format!("{id}"), "NA-Energy-CCC");
    }

    #[test]
    fn test_factor_id_serde_roundtrip() {
        let id = FactorId::new("USD-Rates");
        let json_result = serde_json::to_string(&id);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };
        assert_eq!(json, "\"USD-Rates\"");

        let back_result: Result<FactorId, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };
        assert_eq!(id, back);
    }

    #[test]
    fn test_factor_type_serde() {
        let ft = FactorType::Credit;
        let json_result = serde_json::to_string(&ft);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let back_result: Result<FactorType, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };
        assert_eq!(ft, back);
    }

    #[test]
    fn test_factor_type_custom() {
        let ft = FactorType::Custom("Weather".into());
        let json_result = serde_json::to_string(&ft);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let back_result: Result<FactorType, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };
        assert_eq!(ft, back);
    }

    #[test]
    fn test_factor_type_fromstr_display_roundtrip() {
        for (input, expected) in [
            ("rates", FactorType::Rates),
            ("rate", FactorType::Rates),
            ("ir", FactorType::Rates),
            ("credit", FactorType::Credit),
            ("equity", FactorType::Equity),
            ("fx", FactorType::FX),
            ("volatility", FactorType::Volatility),
            ("vol", FactorType::Volatility),
            ("commodity", FactorType::Commodity),
            ("inflation", FactorType::Inflation),
            ("custom:Weather", FactorType::Custom("Weather".into())),
        ] {
            assert_parses_to(input, expected);
        }

        // Display -> FromStr roundtrip for non-Custom variants
        for variant in [
            FactorType::Rates,
            FactorType::Credit,
            FactorType::Equity,
            FactorType::FX,
            FactorType::Volatility,
            FactorType::Commodity,
            FactorType::Inflation,
        ] {
            let display = variant.to_string();
            assert!(matches!(display.parse::<FactorType>(), Ok(value) if value == variant));
        }

        // Custom roundtrip
        let custom = FactorType::Custom("Weather".into());
        let display = custom.to_string();
        assert_eq!(display, "custom:Weather");
        assert!(matches!(display.parse::<FactorType>(), Ok(value) if value == custom));
    }

    #[test]
    fn test_factor_type_fromstr_rejects_unknown() {
        assert!("unknown".parse::<FactorType>().is_err());
    }
}
