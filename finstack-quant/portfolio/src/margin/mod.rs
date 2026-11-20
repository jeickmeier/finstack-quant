//! Portfolio-level margin aggregation.
//!
//! This module provides netting set management and portfolio-wide margin
//! aggregation, building on the instrument-level margin calculations from
//! the valuations crate.
//!
//! # Overview
//!
//! Margin requirements are typically aggregated at the netting set level,
//! where instruments under the same CSA or CCP can offset each other.
//! This module provides:
//!
//! - **Netting Set Management**: Grouping positions by CSA/CCP
//! - **Portfolio Margin Aggregation**: Combining sensitivities across positions
//! - **Margin Reporting**: Summary views of margin requirements
//!
//! # Conventions
//!
//! - Initial margin is calculated per netting set and then aggregated into the
//!   portfolio base currency.
//! - Variation margin uses netted mark-to-market values, FX-converted into the
//!   base currency when positions are not already denominated in that currency.
//! - Positions that fail sensitivity or mark-to-market extraction are tracked as
//!   degraded outputs rather than silently dropped.
//!
//! # References
//!
//! - `docs/REFERENCES.md#isda-simm`
//!
//! # Usage
//!
//! ```no_run
//! use finstack_quant_portfolio::margin::portfolio_margin;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_portfolio::Portfolio;
//! use time::macros::date;
//!
//! # fn main() -> finstack_quant_portfolio::Result<()> {
//! # let portfolio: Portfolio = unimplemented!("Provide your portfolio");
//! # let market: MarketContext = unimplemented!("Provide market context");
//! let as_of = date!(2025-11-21);
//!
//! // One call groups positions into netting sets and calculates margin.
//! let margin_results = portfolio_margin(
//!     &portfolio,
//!     &market,
//!     as_of,
//!     &finstack_quant_core::FxHashMap::default(),
//! )?;
//!
//! // Get margin by netting set
//! for (netting_set, margin) in &margin_results.by_netting_set {
//!     println!(
//!         "{:?}: IM={:?}, VM={:?}",
//!         netting_set, margin.initial_margin, margin.variation_margin
//!     );
//! }
//! # Ok(())
//! # }
//! ```

mod aggregator;
mod csr_quotes;
mod exposure;
mod frtb;
mod girr_quotes;
mod netting_set;
mod regulatory_quotes;
mod results;
mod wire;

pub use aggregator::portfolio_margin;
pub use csr_quotes::{
    CsrQuoteClassification, CsrQuoteDelta, CsrQuoteRisk, CsrQuoteSource, csr_quote_delta,
};
pub use girr_quotes::{
    GirrQuoteDelta, GirrQuoteRisk, GirrQuoteSource, GirrQuoteTarget, girr_quote_delta,
};
pub use regulatory_quotes::RegulatoryQuoteReprice;
pub use results::{NettingSetMargin, PortfolioMarginResult};

pub use frtb::{
    FrtbUnmappedPosition, OrdinaryDeltaStatus, PortfolioFrtbConfig,
    PortfolioFrtbSensitivitiesResult, portfolio_frtb_sensitivities,
};

pub use exposure::{
    ExposureCashVm, ExposureFactor, ExposureSimulationConfig, instrument_exposure_profile,
    portfolio_exposure_profile,
};
