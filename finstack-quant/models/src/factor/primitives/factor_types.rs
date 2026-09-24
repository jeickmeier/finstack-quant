//! Core identifiers and definitions used by the factor model.
//!
use finstack_quant_core::market_data::bumps::BumpUnits;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a risk factor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FactorType {
    /// Interest-rate factor.
    Rates,
    /// Credit-spread or hazard factor.
    Credit,
    /// Equity price factor.
    Equity,
    /// Foreign-exchange factor.
    Fx,
    /// Volatility factor.
    Volatility,
    /// Commodity factor.
    Commodity,
    /// Inflation factor.
    Inflation,
    /// User-defined factor bucket.
    Custom(String),
}

impl FactorType {
    /// Canonical unit of a bump magnitude for this factor type.
    ///
    /// The unit matches the [`crate::factor::BumpSizeConfig`] field that sizes
    /// the bump:
    ///
    /// * Rates / Credit / Inflation / Custom → [`BumpUnits::RateBp`]
    ///   (`rates_bp`, `credit_bp`; `1.0` = 1 bp = `0.0001`).
    /// * Equity / Commodity / FX → [`BumpUnits::Percent`] (`equity_pct`,
    ///   `fx_pct`; `1.0` = 1 % = `0.01` of the base).
    /// * Volatility → [`BumpUnits::Percent`] (`vol_points`; an additive
    ///   percent bump on a vol surface is one vol point = `0.01` absolute vol).
    #[must_use]
    pub fn bump_units(&self) -> BumpUnits {
        match self {
            Self::Rates | Self::Credit | Self::Inflation | Self::Custom(_) => BumpUnits::RateBp,
            Self::Equity | Self::Commodity | Self::Fx | Self::Volatility => BumpUnits::Percent,
        }
    }
}

impl fmt::Display for FactorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rates => write!(f, "rates"),
            Self::Credit => write!(f, "credit"),
            Self::Equity => write!(f, "equity"),
            Self::Fx => write!(f, "fx"),
            Self::Volatility => write!(f, "volatility"),
            Self::Commodity => write!(f, "commodity"),
            Self::Inflation => write!(f, "inflation"),
            Self::Custom(name) => write!(f, "custom:{name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
