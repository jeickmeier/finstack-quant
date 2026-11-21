//! Index underlying asset parameters for fixed-income index total return swaps.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::{IndexId, PriceId};
use serde::{Deserialize, Serialize};

/// Index underlying parameters for total return swaps and index-linked instruments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct IndexUnderlyingParams {
    /// Index identifier (e.g., "CDX.IG", "HY.BOND.INDEX")
    pub index_id: IndexId,
    /// Currency the index (and so the swap notional) is denominated in.
    pub currency: Currency,
    /// Optional yield curve/scalar identifier for carry calculation
    pub yield_id: Option<PriceId>,
    /// Market scalar identifier for signed index duration in years. Required when
    /// requesting FI TRS duration risk; the scalar must be unitless and finite.
    pub duration_id: Option<PriceId>,
}

impl IndexUnderlyingParams {
    /// Create index underlying parameters with no yield or duration scalar.
    ///
    /// # Arguments
    ///
    /// * `index_id` - Identifier of the reference index (e.g. `"US-CORP-INDEX"`).
    /// * `currency` - ISO-4217 currency the index is denominated in; the swap
    ///   notional must use the same currency.
    pub fn new(index_id: impl Into<String>, currency: Currency) -> Self {
        Self {
            index_id: IndexId::new(index_id),
            currency,
            yield_id: None,
            duration_id: None,
        }
    }

    /// Set the market-scalar id of the index yield used for carry.
    ///
    /// # Arguments
    ///
    /// * `yield_id` - `MarketContext::get_price` id of a unitless decimal
    ///   yield scalar (0.05 = 5% per annum).
    pub fn with_yield_id(mut self, yield_id: impl Into<PriceId>) -> Self {
        self.yield_id = Some(yield_id.into());
        self
    }

    /// Set the market-scalar id of the index duration used for risk.
    ///
    /// # Arguments
    ///
    /// * `duration_id` - `MarketContext::get_price` id of a unitless, finite
    ///   signed index duration in years.
    pub fn with_duration_id(mut self, duration_id: impl Into<PriceId>) -> Self {
        self.duration_id = Some(duration_id.into());
        self
    }

    /// Validate the index identifier and optional market-data identifiers.
    ///
    /// # Arguments
    ///
    /// * `context` - Instrument name included in validation diagnostics.
    pub fn validate(&self, context: &str) -> finstack_quant_core::Result<()> {
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
            "currency": "USD",
            "yield_id": "US-CORP-YIELD",
            "duration_id": "US-CORP-DURATION",
            "convexity_id": "US-CORP-CONVEXITY",
        });

        assert!(serde_json::from_value::<IndexUnderlyingParams>(legacy).is_err());
    }

    #[test]
    fn index_underlying_rejects_removed_contract_size() {
        let legacy = serde_json::json!({
            "index_id": "US-CORP-INDEX",
            "currency": "USD",
            "yield_id": "US-CORP-YIELD",
            "duration_id": "US-CORP-DURATION",
            "contract_size": 1.0,
        });
        assert!(serde_json::from_value::<IndexUnderlyingParams>(legacy).is_err());
    }

    #[test]
    // schema-rejection-test
    fn index_underlying_rejects_retired_base_currency_key() {
        let legacy = serde_json::json!({
            "index_id": "US-CORP-INDEX",
            "base_currency": "USD",
        });
        assert!(serde_json::from_value::<IndexUnderlyingParams>(legacy).is_err());
    }
}
