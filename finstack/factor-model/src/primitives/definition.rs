use super::{FactorId, FactorType};
use finstack_core::currency::Currency;
use finstack_core::market_data::bumps::BumpUnits;
use finstack_core::types::CurveId;
use serde::{Deserialize, Serialize};

/// How a factor movement translates to market-data perturbations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MarketMapping {
    /// Parallel shift to one or more curves.
    CurveParallel {
        /// Curves bumped by the factor.
        curve_ids: Vec<CurveId>,
        /// Units used for the bump magnitude.
        units: BumpUnits,
    },
    /// Bucketed curve shift with tenor weights.
    CurveBucketed {
        /// Curve receiving the bucketed bump.
        curve_id: CurveId,
        /// `(tenor_years, weight)` pairs describing the bucketed shift.
        ///
        /// Weights are typically chosen to sum to `1.0` across the buckets used
        /// by the factor so that the factor move preserves the intended overall
        /// shock size while localizing it along the tenor axis.
        tenor_weights: Vec<(f64, f64)>,
    },
    /// Equity spot move.
    EquitySpot {
        /// Tickers moved by the factor.
        tickers: Vec<String>,
    },
    /// FX rate move.
    FxRate {
        /// Currency pair moved by the factor.
        pair: (Currency, Currency),
    },
    /// Volatility shift.
    VolShift {
        /// Volatility surfaces moved by the factor.
        surface_ids: Vec<String>,
        /// Units used for the bump magnitude.
        units: BumpUnits,
    },
}

/// Complete definition of a risk factor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactorDefinition {
    /// Unique factor identifier.
    pub id: FactorId,
    /// Broad factor classification.
    pub factor_type: FactorType,
    /// Mapping from factor move to market-data perturbation.
    pub market_mapping: MarketMapping,
    /// Optional free-form description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factor_definition_construction() {
        let definition = FactorDefinition {
            id: FactorId::new("USD-Rates"),
            factor_type: FactorType::Rates,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![CurveId::new("USD-OIS")],
                units: BumpUnits::RateBp,
            },
            description: Some("US dollar rates factor".into()),
        };

        assert_eq!(definition.id.as_str(), "USD-Rates");
        assert_eq!(definition.factor_type, FactorType::Rates);
    }

    #[test]
    fn test_market_mapping_curve_parallel_serde() {
        let mapping = MarketMapping::CurveParallel {
            curve_ids: vec![CurveId::new("USD-OIS"), CurveId::new("USD-3M")],
            units: BumpUnits::RateBp,
        };

        let json_result = serde_json::to_string(&mapping);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let back_result: Result<MarketMapping, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };

        let back_json_result = serde_json::to_string(&back);
        assert!(back_json_result.is_ok());
        let Ok(back_json) = back_json_result else {
            return;
        };
        assert_eq!(json, back_json);
    }

    #[test]
    fn test_market_mapping_equity_spot() {
        let mapping = MarketMapping::EquitySpot {
            tickers: vec!["AAPL".into(), "MSFT".into()],
        };

        let json_result = serde_json::to_string(&mapping);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };
        assert!(json.contains("AAPL"));
        assert!(json.contains("MSFT"));
    }

    #[test]
    fn test_market_mapping_fx_rate_serde() {
        let mapping = MarketMapping::FxRate {
            pair: (Currency::USD, Currency::JPY),
        };

        let json_result = serde_json::to_string(&mapping);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let back_result: Result<MarketMapping, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };

        let back_json_result = serde_json::to_string(&back);
        assert!(back_json_result.is_ok());
        let Ok(back_json) = back_json_result else {
            return;
        };
        assert_eq!(json, back_json);
    }
}
