//! Unified MarketQuote enum.
//!
//! This module defines the top-level enum for all supported market quotes. The `MarketQuote`
//! enum provides a unified interface for working with quotes across all instrument types,
//! enabling generic calibration workflows and quote processing.

use super::bond::BondQuote;
use super::cds::CdsQuote;
use super::cds_tranche::CDSTrancheQuote;
use super::fx::FxQuote;
use super::inflation::InflationQuote;
use super::rates::RateQuote;
use super::vol::VolQuote;
use super::xccy::XccyQuote;
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// Polymorphic container for all supported market quote types.
///
/// This enum unifies all quote types into a single type, enabling generic quote processing,
/// serialization, and calibration workflows. Each variant wraps a specific quote type.
///
/// # Examples
///
/// Creating a rates quote:
/// ```rust
/// use finstack_quant_valuations::market::quotes::market_quote::MarketQuote;
/// use finstack_quant_valuations::market::quotes::rates::RateQuote;
/// use finstack_quant_valuations::market::quotes::ids::{Pillar, QuoteId};
/// use finstack_quant_core::types::IndexId;
///
/// # fn example() -> finstack_quant_core::Result<()> {
/// let rate_quote = RateQuote::Deposit {
///     id: QuoteId::new("USD-SOFR-DEP-1M"),
///     index: IndexId::new("USD-SOFR-1M"),
///     pillar: Pillar::Tenor("1M".parse()?),
///     rate: 0.0525,
/// };
///
/// let market_quote = MarketQuote::Rates(rate_quote);
/// # Ok(())
/// # }
/// ```
///
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "class", rename_all = "snake_case", deny_unknown_fields)]
pub enum MarketQuote {
    /// Bond instruments
    Bond(BondQuote),
    /// Interest rate instruments
    Rates(RateQuote),
    /// Credit default swaps
    Cds(CdsQuote),
    /// CDS Tranches
    #[serde(rename = "cds_tranche")]
    CDSTranche(CDSTrancheQuote),
    /// FX instruments
    Fx(FxQuote),
    /// Inflation instruments
    Inflation(InflationQuote),
    /// Volatility instruments
    Vol(VolQuote),
    /// Cross-currency swap instruments
    Xccy(XccyQuote),
}

/// Trait for filtering quote collections into specific types (owned).
pub(crate) trait ExtractQuotes<T> {
    fn extract_quotes(&self) -> Vec<T>;
}

macro_rules! impl_extract_quotes {
    ($quote_type:ty, $variant:ident) => {
        impl ExtractQuotes<$quote_type> for [MarketQuote] {
            fn extract_quotes(&self) -> Vec<$quote_type> {
                self.iter()
                    .filter_map(|q| match q {
                        MarketQuote::$variant(inner) => Some(inner.clone()),
                        _ => None,
                    })
                    .collect()
            }
        }
    };
}

impl_extract_quotes!(RateQuote, Rates);
impl_extract_quotes!(CdsQuote, Cds);
impl_extract_quotes!(CDSTrancheQuote, CDSTranche);
impl_extract_quotes!(InflationQuote, Inflation);
impl_extract_quotes!(XccyQuote, Xccy);
