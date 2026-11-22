//! Commodity underlying asset parameters.

use finstack_quant_core::currency::Currency;
use serde::{Deserialize, Serialize};

/// Commodity underlying parameters for forwards, swaps, and options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CommodityUnderlyingParams {
    /// Commodity type (e.g., "Energy", "Metal", "Agricultural")
    pub commodity_type: String,
    /// Commodity symbol label (e.g., "CL", "GC", "NG"); never a market-data key.
    pub underlying_ticker: String,
    /// Unit of measurement (e.g., "BBL", "OZ", "MT", "MMBTU")
    pub unit: String,
    /// Base currency for pricing
    pub currency: Currency,
}

impl CommodityUnderlyingParams {
    /// Create commodity underlying parameters.
    ///
    /// # Arguments
    ///
    /// * `commodity_type` - Commodity class label (e.g. "Energy", "Metal").
    /// * `underlying_ticker` - Commodity symbol label (e.g. "CL"); not a market-data key.
    /// * `unit` - Unit of measurement for one quantity unit (e.g. "BBL", "MMBTU").
    /// * `currency` - Pricing currency of the commodity.
    pub fn new(
        commodity_type: impl Into<String>,
        underlying_ticker: impl Into<String>,
        unit: impl Into<String>,
        currency: Currency,
    ) -> Self {
        Self {
            commodity_type: commodity_type.into(),
            underlying_ticker: underlying_ticker.into(),
            unit: unit.into(),
            currency,
        }
    }

    /// Validate that the commodity identifiers required for market-data routing are present.
    ///
    /// # Arguments
    ///
    /// * `context` - Instrument name included in validation diagnostics.
    pub fn validate(&self, context: &str) -> finstack_quant_core::Result<()> {
        for (field, value) in [
            ("commodity_type", self.commodity_type.as_str()),
            ("underlying_ticker", self.underlying_ticker.as_str()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // schema-rejection-test
    fn commodity_underlying_params_rejects_unknown_fields() {
        let mut json = serde_json::to_value(CommodityUnderlyingParams::new(
            "Energy",
            "CL",
            "BBL",
            Currency::USD,
        ))
        .expect("serialize");
        json["unexpected"] = serde_json::json!(1);
        assert!(serde_json::from_value::<CommodityUnderlyingParams>(json).is_err());
    }
}
