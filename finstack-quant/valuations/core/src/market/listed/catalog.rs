//! Maintained coverage catalog for liquid listed derivatives.
//!
//! The catalog maps exchange product families to canonical valuation types. It
//! is intentionally a coverage and routing artifact, not a live security
//! master: contract months, trading status, prices, and exchange calendars must
//! still come from current market/reference data.

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
    /// Rates, fixed income, equity, FX, commodity, volatility, or digital assets.
    pub asset_class: String,
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
    include_str!("../../../data/listed/listed_product_catalog.v1.json");

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
            validate_nonblank(index, "asset_class", &row.asset_class)?;
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

/// Serialize the maintained listed-product coverage catalog as stable JSON.
///
/// # Arguments
///
/// * `exchange` - Optional venue filter; `None` returns all four exchanges.
///
/// # Errors
///
/// Returns an error if the embedded sidecar cannot be loaded or the filtered
/// rows cannot be serialized.
pub fn listed_product_catalog_json(
    exchange: Option<ListedExchange>,
) -> finstack_quant_core::Result<String> {
    let rows = listed_product_catalog(exchange)?;
    serde_json::to_string(&rows).map_err(|error| {
        finstack_quant_core::Error::Validation(format!(
            "failed to serialize listed-product catalog: {error}"
        ))
    })
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
        for asset_class in ["rates", "fixed_income", "equity", "fx", "commodity"] {
            assert!(catalog.iter().any(|row| row.asset_class == asset_class));
        }
        assert!(catalog.iter().all(|row| !row.symbols.is_empty()
            && !row.features.is_empty()
            && row.source_url.starts_with("https://")));
    }

    #[test]
    fn venue_filter_is_exact_and_json_is_stable() {
        let mx = listed_product_catalog(Some(ListedExchange::Montreal))
            .expect("embedded Montréal catalog");
        assert!(mx
            .iter()
            .all(|row| row.exchange == ListedExchange::Montreal));
        let json = listed_product_catalog_json(Some(ListedExchange::Montreal))
            .expect("json")
            .to_lowercase();
        assert!(json.contains("three-month corra"));
        assert!(!json.contains("three-month sofr"));
    }

    #[test]
    fn every_option_on_future_routes_to_its_canonical_instrument() {
        for row in listed_product_catalog(None)
            .expect("embedded catalog")
            .into_iter()
            .filter(|row| row.product_kind == ListedProductKind::OptionOnFuture)
        {
            let expected = match row.asset_class.as_str() {
                "rates" | "fixed_income" => InstrumentType::InterestRateFutureOption,
                "equity" => InstrumentType::EquityFutureOption,
                "volatility" => InstrumentType::VolatilityIndexFutureOption,
                "fx" => InstrumentType::FxFutureOption,
                "commodity" | "digital_assets" => InstrumentType::CommodityFutureOption,
                other => panic!("unowned futures-option asset class: {other}"),
            };
            assert_eq!(row.instrument_type, expected);
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
    }
}
