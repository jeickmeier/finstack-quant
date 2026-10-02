//! Core identifiers and definitions used by the factor model.
//!
use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::CurveId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Classification of a curve dependency's role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CurveType {
    /// Discounting curve.
    Discount,
    /// Forward projection curve.
    Forward,
    /// Credit or hazard curve.
    Hazard,
    /// Inflation curve.
    Inflation,
    /// Base-correlation surface-backed curve.
    BaseCorrelation,
}

impl fmt::Display for CurveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discount => write!(f, "discount"),
            Self::Forward => write!(f, "forward"),
            Self::Hazard => write!(f, "hazard"),
            Self::Inflation => write!(f, "inflation"),
            Self::BaseCorrelation => write!(f, "base_correlation"),
        }
    }
}

/// Classification used by dependency filters and declarative matching config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    /// Discounting curve dependency.
    Discount,
    /// Forward projection curve dependency.
    Forward,
    /// Credit or hazard curve dependency.
    Credit,
    /// Equity or commodity spot dependency.
    Spot,
    /// Volatility surface dependency.
    Vol,
    /// FX pair dependency.
    Fx,
    /// Time-series dependency.
    Series,
}

impl fmt::Display for DependencyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discount => write!(f, "discount"),
            Self::Forward => write!(f, "forward"),
            Self::Credit => write!(f, "credit"),
            Self::Spot => write!(f, "spot"),
            Self::Vol => write!(f, "vol"),
            Self::Fx => write!(f, "fx"),
            Self::Series => write!(f, "series"),
        }
    }
}

/// A single market dependency extracted from an instrument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum MarketDependency {
    /// Discount, forward, or other rate curve.
    Curve {
        /// Curve identifier.
        id: CurveId,
        /// Role played by the curve.
        curve_type: CurveType,
    },
    /// Credit or hazard curve.
    CreditCurve {
        /// Curve identifier.
        id: CurveId,
    },
    /// Composite credit-index market data.
    ///
    /// The identifier resolves through `MarketContext::get_credit_index` and
    /// represents the aggregate hazard curve, base correlation, and optional
    /// issuer-level curves as one pricing dependency.
    CreditIndex {
        /// Credit-index aggregate identifier.
        id: CurveId,
    },
    /// Equity or commodity spot.
    Spot {
        /// Spot identifier or ticker.
        id: String,
    },
    /// Volatility surface identifier.
    VolSurface {
        /// Surface identifier.
        id: String,
    },
    /// FX pair dependency.
    FxPair {
        /// Base currency.
        base: Currency,
        /// Quote currency.
        quote: Currency,
    },
    /// Time-series dependency.
    Series {
        /// Series identifier.
        id: String,
    },
}

impl MarketDependency {
    /// Returns whether this dependency matches the requested dependency class.
    #[must_use]
    pub fn matches_dependency_type(&self, dependency_type: DependencyType) -> bool {
        match (self, dependency_type) {
            (Self::Curve { curve_type, .. }, DependencyType::Discount) => {
                *curve_type == CurveType::Discount
            }
            (Self::Curve { curve_type, .. }, DependencyType::Forward) => {
                *curve_type == CurveType::Forward
            }
            (Self::Curve { curve_type, .. }, DependencyType::Credit) => {
                *curve_type == CurveType::Hazard
            }
            (Self::CreditCurve { .. } | Self::CreditIndex { .. }, DependencyType::Credit)
            | (Self::Spot { .. }, DependencyType::Spot)
            | (Self::VolSurface { .. }, DependencyType::Vol)
            | (Self::FxPair { .. }, DependencyType::Fx)
            | (Self::Series { .. }, DependencyType::Series) => true,
            _ => false,
        }
    }

    /// Returns whether this dependency matches the requested curve type.
    ///
    /// `CreditCurve` is the canonical dedicated credit/hazard dependency
    /// variant, but `Curve { curve_type: CurveType::Hazard, .. }` also matches
    /// so callers can treat both representations consistently.
    #[must_use]
    pub fn matches_curve_type(&self, curve_type: CurveType) -> bool {
        match (self, curve_type) {
            (
                Self::Curve {
                    curve_type: actual, ..
                },
                expected,
            ) => *actual == expected,
            (Self::CreditCurve { .. }, CurveType::Hazard) => true,
            _ => false,
        }
    }

    /// Returns whether the dependency matches the provided identifier.
    ///
    /// FX pairs use the canonical `BASE/QUOTE` form, for example `USD/EUR`.
    #[must_use]
    pub fn matches_id(&self, expected_id: &str) -> bool {
        match self {
            Self::Curve { id, .. } | Self::CreditCurve { id } | Self::CreditIndex { id } => {
                id.as_ref() == expected_id
            }
            Self::Spot { id } | Self::VolSurface { id } | Self::Series { id } => id == expected_id,
            Self::FxPair { base, quote } => format!("{base}/{quote}") == expected_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_dependency_curve() {
        let dep = MarketDependency::Curve {
            id: CurveId::new("USD-OIS"),
            curve_type: CurveType::Discount,
        };

        match &dep {
            MarketDependency::Curve { id, curve_type } => {
                assert_eq!(id.as_ref(), "USD-OIS");
                assert_eq!(*curve_type, CurveType::Discount);
            }
            _ => unreachable!("expected curve dependency"),
        }
    }

    #[test]
    fn test_market_dependency_credit_curve() {
        let dep = MarketDependency::CreditCurve {
            id: CurveId::new("ACME-HAZARD"),
        };

        match &dep {
            MarketDependency::CreditCurve { id } => {
                assert_eq!(id.as_ref(), "ACME-HAZARD");
            }
            _ => unreachable!("expected credit curve dependency"),
        }
    }

    #[test]
    fn test_market_dependency_serde_roundtrip() {
        let dep = MarketDependency::Spot {
            id: "AAPL".to_string(),
        };
        let json_result = serde_json::to_string(&dep);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let back_result: Result<MarketDependency, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };
        assert_eq!(dep, back);
    }

    #[test]
    fn test_curve_type_all_variants_serde() {
        for curve_type in [
            CurveType::Discount,
            CurveType::Forward,
            CurveType::Hazard,
            CurveType::Inflation,
            CurveType::BaseCorrelation,
        ] {
            let json_result = serde_json::to_string(&curve_type);
            assert!(json_result.is_ok());
            let Ok(json) = json_result else {
                return;
            };

            let back_result: Result<CurveType, _> = serde_json::from_str(&json);
            assert!(back_result.is_ok());
            let Ok(back) = back_result else {
                return;
            };
            assert_eq!(curve_type, back);
        }
    }

    #[test]
    fn test_dependency_type_all_variants_serde() {
        for dependency_type in [
            DependencyType::Discount,
            DependencyType::Forward,
            DependencyType::Credit,
            DependencyType::Spot,
            DependencyType::Vol,
            DependencyType::Fx,
            DependencyType::Series,
        ] {
            let json_result = serde_json::to_string(&dependency_type);
            assert!(json_result.is_ok());
            let Ok(json) = json_result else {
                return;
            };

            let back_result: Result<DependencyType, _> = serde_json::from_str(&json);
            assert!(back_result.is_ok());
            let Ok(back) = back_result else {
                return;
            };
            assert_eq!(dependency_type, back);
        }
    }

    #[test]
    fn test_market_dependency_matches_dependency_type() {
        let discount = MarketDependency::Curve {
            id: CurveId::new("USD-OIS"),
            curve_type: CurveType::Discount,
        };
        let hazard_curve = MarketDependency::Curve {
            id: CurveId::new("ACME-HAZARD-CURVE"),
            curve_type: CurveType::Hazard,
        };
        let credit = MarketDependency::CreditCurve {
            id: CurveId::new("ACME-HAZARD"),
        };
        let spot = MarketDependency::Spot { id: "AAPL".into() };
        let vol = MarketDependency::VolSurface {
            id: "AAPL-VOL".into(),
        };
        let fx = MarketDependency::FxPair {
            base: Currency::USD,
            quote: Currency::EUR,
        };
        let series = MarketDependency::Series {
            id: "CPI-US".into(),
        };

        assert!(discount.matches_dependency_type(DependencyType::Discount));
        assert!(hazard_curve.matches_dependency_type(DependencyType::Credit));
        assert!(credit.matches_dependency_type(DependencyType::Credit));
        assert!(spot.matches_dependency_type(DependencyType::Spot));
        assert!(vol.matches_dependency_type(DependencyType::Vol));
        assert!(fx.matches_dependency_type(DependencyType::Fx));
        assert!(series.matches_dependency_type(DependencyType::Series));
        assert!(!spot.matches_dependency_type(DependencyType::Credit));
    }

    #[test]
    fn test_market_dependency_matches_curve_type() {
        let rate = MarketDependency::Curve {
            id: CurveId::new("USD-OIS"),
            curve_type: CurveType::Discount,
        };
        let hazard_curve = MarketDependency::Curve {
            id: CurveId::new("ACME-HAZARD-CURVE"),
            curve_type: CurveType::Hazard,
        };
        let credit = MarketDependency::CreditCurve {
            id: CurveId::new("ACME-HAZARD"),
        };

        assert!(rate.matches_curve_type(CurveType::Discount));
        assert!(!rate.matches_curve_type(CurveType::Hazard));
        assert!(hazard_curve.matches_curve_type(CurveType::Hazard));
        assert!(credit.matches_curve_type(CurveType::Hazard));
    }

    #[test]
    fn test_market_dependency_matches_id() {
        let curve = MarketDependency::Curve {
            id: CurveId::new("USD-OIS"),
            curve_type: CurveType::Discount,
        };
        let spot = MarketDependency::Spot { id: "AAPL".into() };
        let fx = MarketDependency::FxPair {
            base: Currency::USD,
            quote: Currency::EUR,
        };

        assert!(curve.matches_id("USD-OIS"));
        assert!(spot.matches_id("AAPL"));
        assert!(fx.matches_id("USD/EUR"));
        assert!(!fx.matches_id("EUR/USD"));
    }
}
