//! Underlying parameter types for different asset classes.

use finstack_core::currency::Currency;
use finstack_core::types::CurveId;
use finstack_core::types::IndexId;
use finstack_core::types::PriceId;

use serde::{Deserialize, Serialize};

/// Map a currency to the canonical OIS discount curve ID for that ccy.
///
/// Returns an explicit `Validation` error for currencies that are not in the
/// supported set rather than silently falling back to `"USD-OIS"`. Callers that
/// price cross-border / EM-currency books must surface the unsupported currency
/// to the user rather than silently mispricing against the wrong curve.
pub fn default_ois_curve_id(currency: Currency) -> finstack_core::Result<&'static str> {
    match currency {
        Currency::USD => Ok("USD-OIS"),
        Currency::EUR => Ok("EUR-OIS"),
        Currency::GBP => Ok("GBP-OIS"),
        Currency::JPY => Ok("JPY-OIS"),
        Currency::CHF => Ok("CHF-OIS"),
        Currency::CAD => Ok("CAD-OIS"),
        Currency::AUD => Ok("AUD-OIS"),
        Currency::NZD => Ok("NZD-OIS"),
        other => Err(finstack_core::Error::Validation(format!(
            "no default OIS curve for currency {other}; supply the discount curve ID explicitly"
        ))),
    }
}

/// Base trait for underlying parameters to enable polymorphic behavior
pub trait UnderlyingParams {
    /// Get the base currency for pricing
    fn base_currency(&self) -> Currency;

    /// Get the primary curve identifier.
    ///
    /// Returns `Err` when the underlying's currency does not have a registered
    /// default discount curve. Implementations must not silently substitute
    /// USD-OIS for unknown currencies — that is a known mispricing trap.
    fn primary_curve_id(&self) -> finstack_core::Result<&str>;
}

/// FX underlying parameters used by FX options and FX swaps.
///
/// This struct encapsulates the market data curve identifiers and
/// currency pair information needed for pricing FX-related instruments.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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

impl UnderlyingParams for FxUnderlyingParams {
    fn base_currency(&self) -> Currency {
        self.base_currency
    }

    fn primary_curve_id(&self) -> finstack_core::Result<&str> {
        Ok(self.domestic_discount_curve_id.as_ref())
    }
}

/// Equity underlying parameters for options and equity-linked swaps.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EquityUnderlyingParams {
    /// Underlying ticker/identifier
    pub ticker: String,
    /// Spot price identifier in market data
    pub spot_id: PriceId,
    /// Optional dividend yield identifier
    pub div_yield_id: Option<CurveId>,
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
    pub fn with_dividend_yield(mut self, div_yield_id: impl Into<CurveId>) -> Self {
        self.div_yield_id = Some(div_yield_id.into());
        self
    }

    /// Set contract size
    pub fn with_contract_size(mut self, size: f64) -> Self {
        self.contract_size = size;
        self
    }
}

impl UnderlyingParams for EquityUnderlyingParams {
    fn base_currency(&self) -> Currency {
        self.currency
    }

    fn primary_curve_id(&self) -> finstack_core::Result<&str> {
        default_ois_curve_id(self.currency)
    }
}

/// Commodity underlying parameters for forwards, swaps, and options.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
}

impl UnderlyingParams for CommodityUnderlyingParams {
    fn base_currency(&self) -> Currency {
        self.currency
    }

    fn primary_curve_id(&self) -> finstack_core::Result<&str> {
        default_ois_curve_id(self.currency)
    }
}

/// Index underlying parameters for total return swaps and index-linked instruments.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct IndexUnderlyingParams {
    /// Index identifier (e.g., "CDX.IG", "HY.BOND.INDEX")
    pub index_id: IndexId,
    /// Base currency of the index
    pub base_currency: Currency,
    /// Optional yield curve/scalar identifier for carry calculation
    pub yield_id: Option<String>,
    /// Optional duration identifier for risk calculations
    pub duration_id: Option<String>,
    /// Optional convexity identifier for risk calculations.
    ///
    /// **Note**: Currently unused in pricing. Convexity adjustment to the forward
    /// price is not yet implemented. This field is reserved for future enhancement
    /// (e.g., second-order yield sensitivity for long-dated TRS).
    pub convexity_id: Option<String>,
    /// Contract size (index units per contract, defaults to 1.0)
    pub contract_size: f64,
}

impl IndexUnderlyingParams {
    /// Create index underlying parameters
    pub fn new(index_id: impl Into<String>, base_currency: Currency) -> Self {
        Self {
            index_id: IndexId::new(index_id),
            base_currency,
            yield_id: None,
            duration_id: None,
            convexity_id: None,
            contract_size: 1.0,
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

    /// Set convexity identifier for risk calculations
    pub fn with_convexity(mut self, convexity_id: impl Into<String>) -> Self {
        self.convexity_id = Some(convexity_id.into());
        self
    }

    /// Set contract size
    pub fn with_contract_size(mut self, size: f64) -> Self {
        self.contract_size = size;
        self
    }
}

impl UnderlyingParams for IndexUnderlyingParams {
    fn base_currency(&self) -> Currency {
        self.base_currency
    }

    fn primary_curve_id(&self) -> finstack_core::Result<&str> {
        default_ois_curve_id(self.base_currency)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ois_curve_id_returns_known_curves() {
        assert_eq!(default_ois_curve_id(Currency::USD).unwrap(), "USD-OIS");
        assert_eq!(default_ois_curve_id(Currency::EUR).unwrap(), "EUR-OIS");
        assert_eq!(default_ois_curve_id(Currency::JPY).unwrap(), "JPY-OIS");
    }

    #[test]
    fn default_ois_curve_id_errors_on_unknown_currency() {
        // No silent USD-OIS fallback for unsupported currencies — this used to
        // be the source of cross-border mispricings.
        let err = default_ois_curve_id(Currency::SEK).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("no default OIS curve") && msg.contains("SEK"),
            "expected diagnostic mentioning the unsupported currency, got: {msg}"
        );
    }

    #[test]
    fn equity_underlying_primary_curve_id_propagates_unsupported_ccy() {
        let params = EquityUnderlyingParams::new("X.AB", PriceId::new("X.AB.SPOT"), Currency::SEK);
        assert!(params.primary_curve_id().is_err());
    }
}
