//! Underlying parameter types for different asset classes.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::types::IndexId;
use finstack_quant_core::types::PriceId;

use serde::{Deserialize, Serialize};

/// FX underlying parameters used by FX options and FX swaps.
///
/// This struct encapsulates the market data curve identifiers and
/// currency pair information needed for pricing FX-related instruments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct FxUnderlyingParams {
    /// Base currency (being priced)
    pub base_currency: Currency,
    /// Quote currency (pricing currency)
    pub quote_currency: Currency,
    /// Domestic discount curve ID (quote currency)
    pub domestic_discount_curve_id: CurveId,
    /// Foreign discount curve ID (base currency)
    pub foreign_discount_curve_id: CurveId,
}

impl FxUnderlyingParams {
    /// Create FX underlying parameters
    pub fn new(
        base_currency: Currency,
        quote_currency: Currency,
        domestic_discount_curve_id: impl Into<CurveId>,
        foreign_discount_curve_id: impl Into<CurveId>,
    ) -> Self {
        Self {
            base_currency,
            quote_currency,
            domestic_discount_curve_id: domestic_discount_curve_id.into(),
            foreign_discount_curve_id: foreign_discount_curve_id.into(),
        }
    }

    /// Standard USD/EUR pair
    pub fn usd_eur() -> Self {
        Self::new(Currency::EUR, Currency::USD, "USD-OIS", "EUR-OIS")
    }

    /// Standard GBP/USD pair
    pub fn gbp_usd() -> Self {
        Self::new(Currency::GBP, Currency::USD, "USD-OIS", "GBP-OIS")
    }

    /// Standard USD/JPY pair
    pub fn usd_jpy() -> Self {
        Self::new(Currency::JPY, Currency::USD, "USD-OIS", "JPY-OIS")
    }
}

/// Equity underlying parameters for options and equity-linked swaps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EquityUnderlyingParams {
    /// Underlying ticker/identifier
    pub ticker: String,
    /// Spot price identifier in market data
    pub spot_id: PriceId,
    /// Optional dividend yield identifier
    pub div_yield_id: Option<PriceId>,
    /// Contract size (shares per contract)
    pub contract_size: f64,
    /// Base currency for pricing
    pub currency: Currency,
}

impl EquityUnderlyingParams {
    /// Create equity underlying parameters
    pub fn new(ticker: impl Into<String>, spot_id: impl Into<PriceId>, currency: Currency) -> Self {
        Self {
            ticker: ticker.into(),
            spot_id: spot_id.into(),
            div_yield_id: None,
            contract_size: 1.0,
            currency,
        }
    }

    /// Set dividend yield identifier
    pub fn with_dividend_yield(mut self, div_yield_id: impl Into<PriceId>) -> Self {
        self.div_yield_id = Some(div_yield_id.into());
        self
    }

    /// Set contract size
    pub fn with_contract_size(mut self, size: f64) -> Self {
        self.contract_size = size;
        self
    }

    /// Validate identifiers, contract scale, and market-data routing fields.
    ///
    /// # Arguments
    ///
    /// * `context` - Instrument name included in validation diagnostics.
    pub(crate) fn validate(&self, context: &str) -> finstack_quant_core::Result<()> {
        if self.ticker.trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires a non-empty equity ticker"
            )));
        }
        if self.spot_id.as_str().trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires a non-empty equity spot_id"
            )));
        }
        if self
            .div_yield_id
            .as_ref()
            .is_some_and(|id| id.as_str().trim().is_empty())
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} dividend-yield identifier cannot be empty"
            )));
        }
        if !self.contract_size.is_finite() || self.contract_size < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} contract_size must be non-negative and finite"
            )));
        }
        Ok(())
    }
}

/// Commodity underlying parameters for forwards, swaps, and options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct CommodityUnderlyingParams {
    /// Commodity type (e.g., "Energy", "Metal", "Agricultural")
    pub commodity_type: String,
    /// Ticker/identifier for market data lookup (e.g., "CL", "GC", "NG")
    pub ticker: String,
    /// Unit of measurement (e.g., "BBL", "OZ", "MT", "MMBTU")
    pub unit: String,
    /// Base currency for pricing
    pub currency: Currency,
}

impl CommodityUnderlyingParams {
    /// Create commodity underlying parameters.
    pub fn new(
        commodity_type: impl Into<String>,
        ticker: impl Into<String>,
        unit: impl Into<String>,
        currency: Currency,
    ) -> Self {
        Self {
            commodity_type: commodity_type.into(),
            ticker: ticker.into(),
            unit: unit.into(),
            currency,
        }
    }

    /// Validate that the commodity identifiers required for market-data routing are present.
    ///
    /// # Arguments
    ///
    /// * `context` - Instrument name included in validation diagnostics.
    pub(crate) fn validate(&self, context: &str) -> finstack_quant_core::Result<()> {
        for (field, value) in [
            ("commodity_type", self.commodity_type.as_str()),
            ("ticker", self.ticker.as_str()),
            ("unit", self.unit.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} requires a non-empty commodity {field}"
                )));
            }
        }
        Ok(())
    }
}

/// Index underlying parameters for total return swaps and index-linked instruments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IndexUnderlyingParams {
    /// Index identifier (e.g., "CDX.IG", "HY.BOND.INDEX")
    pub index_id: IndexId,
    /// Base currency of the index
    pub base_currency: Currency,
    /// Optional yield curve/scalar identifier for carry calculation
    pub yield_id: Option<String>,
    /// Market scalar identifier for signed index duration in years. Required when
    /// requesting FI TRS duration risk; the scalar must be unitless and finite. No duration is inferred from index name or maturity.
    pub duration_id: Option<String>,
}

impl IndexUnderlyingParams {
    /// Create index underlying parameters
    pub fn new(index_id: impl Into<String>, base_currency: Currency) -> Self {
        Self {
            index_id: IndexId::new(index_id),
            base_currency,
            yield_id: None,
            duration_id: None,
        }
    }

    /// Set yield identifier for carry calculation
    pub fn with_yield(mut self, yield_id: impl Into<String>) -> Self {
        self.yield_id = Some(yield_id.into());
        self
    }

    /// Set duration identifier for risk calculations
    pub fn with_duration(mut self, duration_id: impl Into<String>) -> Self {
        self.duration_id = Some(duration_id.into());
        self
    }

    /// Validate the index identifier and optional market-data identifiers.
    ///
    /// # Arguments
    ///
    /// * `context` - Instrument name included in validation diagnostics.
    pub(crate) fn validate(&self, context: &str) -> finstack_quant_core::Result<()> {
        if self.index_id.as_str().trim().is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context} requires a non-empty index_id"
            )));
        }
        for (field, id) in [
            ("yield_id", self.yield_id.as_deref()),
            ("duration_id", self.duration_id.as_deref()),
        ] {
            if id.is_some_and(|value| value.trim().is_empty()) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "{context} {field} cannot be empty"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_underlying_rejects_removed_convexity_id() {
        let legacy = serde_json::json!({
            "index_id": "US-CORP-INDEX",
            "base_currency": "USD",
            "yield_id": "US-CORP-YIELD",
            "duration_id": "US-CORP-DURATION",
            "convexity_id": "US-CORP-CONVEXITY",
        });

        assert!(serde_json::from_value::<IndexUnderlyingParams>(legacy).is_err());
    }

    /// `contract_size` scaled only the total-return leg, never financing, so
    /// it was removed; wire payloads carrying it are rejected.
    #[test]
    fn index_underlying_rejects_removed_contract_size() {
        let legacy = serde_json::json!({
            "index_id": "US-CORP-INDEX",
            "base_currency": "USD",
            "yield_id": "US-CORP-YIELD",
            "duration_id": "US-CORP-DURATION",
            "contract_size": 1.0,
        });
        assert!(serde_json::from_value::<IndexUnderlyingParams>(legacy).is_err());
    }
}
