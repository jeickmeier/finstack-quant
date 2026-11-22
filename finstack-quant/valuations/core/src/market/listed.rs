//! Exchange-listed product coverage and valuation routing metadata.
//!
//! [`listed_product_catalog`] maps the liquid product families reviewed on CME,
//! Eurex, Montréal Exchange, and SGX to canonical asset-class instrument types.
//! All product definitions and route settings live in the versioned
//! `data/listed/listed_product_catalog.v1.json` sidecar; Rust only parses,
//! validates, filters, and serializes that data.
//!
//! The catalog is a maintained coverage and routing artifact, not a live
//! security master or liquidity feed: callers must still load active expiries,
//! current multipliers, ticks, calendars, marks, fixings, curves, and
//! volatility quotes from an exchange or market-data vendor.
//!
//! The one material partial route is deliverable government-bond futures:
//! cheapest-to-deliver, conversion-factor, and invoice economics are supported
//! by `BondFuture`, while delivery-day timing and wildcard optionality are not
//! model-valued.
//!
//! [`listed_product_catalog`]: crate::market::listed::listed_product_catalog

use crate::pricer::InstrumentType;

/// Supported exchange catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ListedExchange {
    /// CME Group exchanges (CME, CBOT, NYMEX, COMEX).
    Cme,
    /// Eurex.
    Eurex,
    /// Montréal Exchange.
    Montreal,
    /// Singapore Exchange.
    Sgx,
}

impl std::fmt::Display for ListedExchange {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Cme => "cme",
            Self::Eurex => "eurex",
            Self::Montreal => "montreal",
            Self::Sgx => "sgx",
        })
    }
}

impl std::str::FromStr for ListedExchange {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "cme" => Ok(Self::Cme),
            "eurex" => Ok(Self::Eurex),
            "montreal" => Ok(Self::Montreal),
            "sgx" => Ok(Self::Sgx),
            _ => Err(format!(
                "unknown listed exchange '{value}'; expected cme, eurex, montreal, or sgx"
            )),
        }
    }
}

/// High-level asset class of a listed product family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ListedAssetClass {
    /// Short-term and money-market interest rates.
    Rates,
    /// Government bonds and credit fixed-income products.
    FixedIncome,
    /// Equity indices and single-stock derivatives.
    Equity,
    /// Foreign-exchange futures and options.
    Fx,
    /// Energy, metals, and agricultural commodities.
    Commodity,
    /// Equity and cross-market volatility indices.
    Volatility,
    /// Regulated cryptocurrency derivatives.
    DigitalAssets,
    /// Multi-asset or cross-asset index products.
    CrossAsset,
}

impl std::fmt::Display for ListedAssetClass {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Rates => "rates",
            Self::FixedIncome => "fixed_income",
            Self::Equity => "equity",
            Self::Fx => "fx",
            Self::Commodity => "commodity",
            Self::Volatility => "volatility",
            Self::DigitalAssets => "digital_assets",
            Self::CrossAsset => "cross_asset",
        })
    }
}

impl std::str::FromStr for ListedAssetClass {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "rates" => Ok(Self::Rates),
            "fixed_income" => Ok(Self::FixedIncome),
            "equity" => Ok(Self::Equity),
            "fx" => Ok(Self::Fx),
            "commodity" => Ok(Self::Commodity),
            "volatility" => Ok(Self::Volatility),
            "digital_assets" => Ok(Self::DigitalAssets),
            "cross_asset" => Ok(Self::CrossAsset),
            _ => Err(format!(
                "unknown listed asset class '{value}'; expected rates, fixed_income, equity, fx, commodity, volatility, digital_assets, or cross_asset"
            )),
        }
    }
}

/// High-level listed product form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ListedProductKind {
    /// Exchange future.
    Future,
    /// Option whose exercise delivers or references a future.
    OptionOnFuture,
    /// Cash-index or single-security option.
    Option,
}

/// Readiness of the mapped valuation route.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ListedCoverageStatus {
    /// Direct canonical instrument with complete core pricing and first-order risk.
    Native,
    /// Supported by composing existing canonical instruments.
    Composed,
    /// Core price/risk is supported but a named exchange optionality remains external.
    Partial,
}

/// One liquid exchange product family and its valuation route.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListedProductCoverage {
    /// Exchange venue.
    pub exchange: ListedExchange,
    /// Exchange root symbols, comma-separated where one row covers a close family.
    pub symbols: String,
    /// Human-readable exchange product family.
    pub name: String,
    /// Asset class of the listed product family.
    pub asset_class: ListedAssetClass,
    /// Future, option on future, or direct option.
    pub product_kind: ListedProductKind,
    /// Canonical valuation type selected by the library.
    pub instrument_type: InstrumentType,
    /// Native, composed, or partial coverage.
    pub status: ListedCoverageStatus,
    /// Exchange features exercised by this mapping.
    pub features: Vec<String>,
    /// Residual feature not included in the canonical valuation, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_gap: Option<String>,
    /// Current official exchange page used to verify the family.
    pub source_url: String,
}

const CATALOG_SCHEMA: &str = "finstack_quant.listed_product_catalog/1";
const EMBEDDED_LISTED_PRODUCT_CATALOG: &str =
    include_str!("../../data/listed/listed_product_catalog.v1.json");

static EMBEDDED_CATALOG: finstack_quant_core::embedded_registry::EmbeddedJsonRegistry<
    ListedProductCatalogFile,
> = finstack_quant_core::embedded_registry::EmbeddedJsonRegistry::new(
    EMBEDDED_LISTED_PRODUCT_CATALOG,
    None,
    "listed-product catalog",
);

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ListedProductCatalogFile {
    schema: String,
    products: Vec<ListedProductCoverage>,
}

impl ListedProductCatalogFile {
    fn validate(&self) -> finstack_quant_core::Result<()> {
        if self.schema != CATALOG_SCHEMA {
            return Err(finstack_quant_core::Error::Validation(format!(
                "unsupported listed-product catalog schema '{}'; expected '{CATALOG_SCHEMA}'",
                self.schema
            )));
        }
        if self.products.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "listed-product catalog must contain at least one product".to_string(),
            ));
        }

        let mut seen = std::collections::BTreeSet::new();
        for (index, row) in self.products.iter().enumerate() {
            validate_nonblank(index, "symbols", &row.symbols)?;
            if !row.symbols.split(',').all(|root| {
                !root.is_empty()
                    && root.chars().all(|character| {
                        character.is_ascii_uppercase() || character.is_ascii_digit()
                    })
            }) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "listed-product catalog row {index} symbols must be comma-separated exchange roots"
                )));
            }
            validate_nonblank(index, "name", &row.name)?;
            if row.features.is_empty() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "listed-product catalog row {index} must contain at least one feature"
                )));
            }
            for feature in &row.features {
                validate_nonblank(index, "feature", feature)?;
            }
            if let Some(residual_gap) = &row.residual_gap {
                validate_nonblank(index, "residual_gap", residual_gap)?;
            }
            if !row.source_url.starts_with("https://") {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "listed-product catalog row {index} source_url must use https"
                )));
            }

            let key = format!(
                "{}|{}|{:?}",
                row.exchange,
                row.symbols.trim(),
                row.product_kind
            );
            if !seen.insert(key) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "listed-product catalog contains duplicate route for {} '{}'",
                    row.exchange, row.symbols
                )));
            }
        }
        Ok(())
    }
}

fn validate_nonblank(index: usize, field: &str, value: &str) -> finstack_quant_core::Result<()> {
    if value.trim().is_empty() {
        Err(finstack_quant_core::Error::Validation(format!(
            "listed-product catalog row {index} has blank {field}"
        )))
    } else {
        Ok(())
    }
}

fn validate_catalog(
    catalog: ListedProductCatalogFile,
) -> finstack_quant_core::Result<ListedProductCatalogFile> {
    catalog.validate()?;
    Ok(catalog)
}

#[cfg(test)]
fn parse_catalog_json(raw: &str) -> finstack_quant_core::Result<ListedProductCatalogFile> {
    let catalog = serde_json::from_str::<ListedProductCatalogFile>(raw).map_err(|error| {
        finstack_quant_core::Error::Validation(format!(
            "failed to parse listed-product catalog: {error}"
        ))
    })?;
    validate_catalog(catalog)
}

fn embedded_catalog() -> finstack_quant_core::Result<&'static ListedProductCatalogFile> {
    EMBEDDED_CATALOG.load(validate_catalog)
}

/// Return the maintained coverage map for liquid CME, Eurex, Montréal, and SGX products.
///
/// Definitions are loaded from the versioned
/// `data/listed/listed_product_catalog.v1.json` sidecar. This is not a live
/// listing or liquidity feed: callers must source active contract months and
/// current specifications from the exchange.
///
/// # Arguments
///
/// * `exchange` - Optional venue filter; `None` returns all four exchanges.
///
/// # Errors
///
/// Returns an error if the embedded sidecar cannot be parsed or fails catalog
/// validation.
pub fn listed_product_catalog(
    exchange: Option<ListedExchange>,
) -> finstack_quant_core::Result<Vec<ListedProductCoverage>> {
    let catalog = embedded_catalog()?;
    Ok(catalog
        .products
        .iter()
        .filter(|row| exchange.is_none_or(|venue| row.exchange == venue))
        .cloned()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_exchange_and_main_asset_class_has_a_route() {
        let catalog = listed_product_catalog(None).expect("embedded catalog");
        for exchange in [
            ListedExchange::Cme,
            ListedExchange::Eurex,
            ListedExchange::Montreal,
            ListedExchange::Sgx,
        ] {
            assert!(catalog.iter().any(|row| row.exchange == exchange));
        }
        for asset_class in [
            ListedAssetClass::Rates,
            ListedAssetClass::FixedIncome,
            ListedAssetClass::Equity,
            ListedAssetClass::Fx,
            ListedAssetClass::Commodity,
        ] {
            assert!(catalog.iter().any(|row| row.asset_class == asset_class));
        }
        assert!(catalog.iter().all(|row| !row.symbols.is_empty()
            && !row.features.is_empty()
            && row.source_url.starts_with("https://")));
    }

    #[test]
    fn venue_filter_is_exact() {
        let mx = listed_product_catalog(Some(ListedExchange::Montreal))
            .expect("embedded Montréal catalog");
        assert!(
            mx.iter()
                .all(|row| row.exchange == ListedExchange::Montreal)
        );
        assert!(
            mx.iter()
                .any(|row| row.name.to_lowercase().contains("three-month corra"))
        );
        assert!(
            !mx.iter()
                .any(|row| row.name.to_lowercase().contains("three-month sofr"))
        );
    }

    #[test]
    fn every_option_on_future_routes_to_its_canonical_instrument() {
        for row in listed_product_catalog(None)
            .expect("embedded catalog")
            .into_iter()
            .filter(|row| row.product_kind == ListedProductKind::OptionOnFuture)
        {
            let expected = match row.asset_class {
                ListedAssetClass::Rates | ListedAssetClass::FixedIncome => {
                    Some(InstrumentType::InterestRateFutureOption)
                }
                ListedAssetClass::Equity => Some(InstrumentType::EquityFutureOption),
                ListedAssetClass::Volatility => Some(InstrumentType::VolatilityIndexFutureOption),
                ListedAssetClass::Fx => Some(InstrumentType::FxFutureOption),
                ListedAssetClass::Commodity | ListedAssetClass::DigitalAssets => {
                    Some(InstrumentType::CommodityFutureOption)
                }
                ListedAssetClass::CrossAsset => None,
            };
            assert_eq!(Some(row.instrument_type), expected);
        }
    }

    #[test]
    fn sidecar_is_the_complete_catalog_source() {
        let catalog = parse_catalog_json(EMBEDDED_LISTED_PRODUCT_CATALOG)
            .expect("versioned listed-product sidecar");
        assert_eq!(catalog.products.len(), 46);
        assert_eq!(
            catalog.products,
            listed_product_catalog(None).expect("cached embedded catalog")
        );
    }

    #[test]
    fn sidecar_rejects_unknown_schema_and_invalid_rows() {
        let unknown_schema = EMBEDDED_LISTED_PRODUCT_CATALOG.replacen(
            CATALOG_SCHEMA,
            "finstack_quant.listed_product_catalog/2",
            1,
        );
        assert!(parse_catalog_json(&unknown_schema).is_err());

        let mut invalid_row: serde_json::Value =
            serde_json::from_str(EMBEDDED_LISTED_PRODUCT_CATALOG).expect("valid sidecar JSON");
        invalid_row["products"][0]["symbols"] = serde_json::Value::String(String::new());
        assert!(parse_catalog_json(&invalid_row.to_string()).is_err());

        let mut invalid_asset_class: serde_json::Value =
            serde_json::from_str(EMBEDDED_LISTED_PRODUCT_CATALOG).expect("valid sidecar JSON");
        invalid_asset_class["products"][0]["asset_class"] =
            serde_json::Value::String("unknown".to_string());
        assert!(parse_catalog_json(&invalid_asset_class.to_string()).is_err());
    }

    #[test]
    fn listed_asset_class_display_and_from_str_round_trip() {
        for asset_class in [
            ListedAssetClass::Rates,
            ListedAssetClass::FixedIncome,
            ListedAssetClass::Equity,
            ListedAssetClass::Fx,
            ListedAssetClass::Commodity,
            ListedAssetClass::Volatility,
            ListedAssetClass::DigitalAssets,
            ListedAssetClass::CrossAsset,
        ] {
            let text = asset_class.to_string();
            assert_eq!(text.parse::<ListedAssetClass>(), Ok(asset_class));
        }
        assert!("invalid".parse::<ListedAssetClass>().is_err());
    }
}
