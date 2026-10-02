//! Unified MarketQuote enum.
//!
//! This module defines the top-level enum for all supported market quotes. The `MarketQuote`
//! enum provides a unified interface for working with quotes across all instrument types,
//! enabling generic calibration workflows and quote processing.

use super::cds::CdsQuote;
use super::cds_tranche::CdsTrancheQuote;
use super::inflation::InflationQuote;
use super::rates::RateQuote;
use super::vol::VolQuote;
use super::xccy::XccyQuote;
use finstack_quant_core::{Error, Result};

/// Polymorphic container for all supported market quote types.
///
/// This enum unifies all quote types into a single type, enabling generic quote processing,
/// serialization, and calibration workflows. Each variant wraps a specific quote type.
///
/// # Examples
///
/// Creating a rates quote:
/// ```rust
/// use finstack_quant_calibration::quotes::market_quote::MarketQuote;
/// use finstack_quant_calibration::quotes::rates::RateQuote;
/// use finstack_quant_calibration::quotes::ids::{Pillar, QuoteId};
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "class", rename_all = "snake_case", deny_unknown_fields)]
pub enum MarketQuote {
    /// Interest rate instruments
    Rates(RateQuote),
    /// Credit default swaps
    Cds(CdsQuote),
    /// CDS Tranches
    CdsTranche(CdsTrancheQuote),
    /// Inflation instruments
    Inflation(InflationQuote),
    /// Volatility instruments
    Vol(VolQuote),
    /// Cross-currency swap instruments
    Xccy(XccyQuote),
}

impl MarketQuote {
    /// Identifier shared by every quote class.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Rates(quote) => quote.id().as_str(),
            Self::Cds(quote) => quote.id().as_str(),
            Self::CdsTranche(quote) => quote.id().as_str(),
            Self::Inflation(quote) => quote.id().as_str(),
            Self::Vol(quote) => quote.id().as_str(),
            Self::Xccy(quote) => quote.id().as_str(),
        }
    }

    /// Validate quote-domain invariants before calibration.
    pub fn validate(&self) -> Result<()> {
        if self.id().trim().is_empty() {
            return Err(Error::Validation(
                crate::quotes::ids::EMPTY_QUOTE_ID.to_string(),
            ));
        }

        match self {
            Self::Rates(quote) => quote.validate(),
            Self::Cds(quote) => quote.validate(),
            Self::CdsTranche(quote) => quote.validate(),
            Self::Inflation(quote) => quote.validate(),
            Self::Vol(quote) => quote.validate(),
            Self::Xccy(quote) => quote.validate(),
        }
    }
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
impl_extract_quotes!(CdsTrancheQuote, CdsTranche);
impl_extract_quotes!(InflationQuote, Inflation);
impl_extract_quotes!(XccyQuote, Xccy);
