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
//! ```ignore
//! use finstack_quant_portfolio::PortfolioMarginAggregator;
//! use finstack_quant_core::market_data::context::MarketContext;
//! use finstack_quant_portfolio::Portfolio;
//! use time::macros::date;
//!
//! // Create margin aggregator from portfolio
//! # fn main() -> finstack_quant_portfolio::Result<()> {
//! # let portfolio: Portfolio = unimplemented!("Provide your portfolio");
//! # let market: MarketContext = unimplemented!("Provide market context");
//! let as_of = date!(2025-11-21);
//! let mut aggregator = PortfolioMarginAggregator::from_portfolio(&portfolio);
//!
//! // Calculate margin requirements
//! let margin_results = aggregator.calculate(&portfolio, &market, as_of)?;
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
mod netting_set;
mod results;
mod wire;

pub use aggregator::PortfolioMarginAggregator;
pub use netting_set::{NettingSet, NettingSetManager};
pub use results::{CurrencyMismatchError, NettingSetMargin, PortfolioMarginResult};
