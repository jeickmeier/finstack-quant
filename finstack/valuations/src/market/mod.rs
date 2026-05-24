//! Market data inputs, conventions, and quote-to-instrument construction.
//!
//! This module provides the foundation for market data representation and instrument construction
//! in Finstack. It encompasses three main areas:
//!
//! 1. **Market Quotes** (`quotes/`): Stable schemas for market quotes across rates, credit,
//!    inflation, and volatility instruments. Quotes are serializable and include identifiers
//!    for calibration workflows.
//!
//! 2. **Conventions** (`conventions/`): Market convention registries loaded from embedded JSON
//!    data. Conventions define day count, business day adjustments, payment frequencies, and
//!    other market-standard parameters required for instrument construction.
//!
//! 3. **Builders** (`build/`): Quote-to-instrument construction logic that resolves conventions,
//!    calculates dates, and creates concrete instrument instances ready for pricing.
//!
//! # Documentation Rules For Market APIs
//!
//! Market-facing docs should explicitly call out:
//!
//! - quote units and quote conventions (decimal vs bp, clean vs dirty, par vs spread)
//! - day count, calendar, spot lag, and settlement assumptions when conventions are resolved
//! - which curve-role mappings are required versus which are convention-derived fallbacks
//! - whether the API is schema-only, convention lookup, or actual quote-to-instrument construction
//!
//! # Features
//!
//! - **Stable quote schemas**: All quote types use strict serde names for long-lived pipelines
//! - **Convention registry**: Singleton registry with embedded JSON data for all market conventions
//! - **Quote-to-instrument builders**: Deterministic construction with explicit error handling
//! - **Prepared quotes**: Envelopes combining quotes with instruments and precomputed pillar times
//!   for calibration solvers
//!
//! # Quick Example
//!
//! ```rust
//! use finstack_valuations::market::{BuildCtx, build_rate_instrument};
//! use finstack_valuations::market::quotes::ids::{Pillar, QuoteId};
//! use finstack_valuations::market::quotes::rates::RateQuote;
//! use finstack_core::types::IndexId;
//! use finstack_valuations::market::conventions::ConventionRegistry;
//! use finstack_core::dates::Date;
//! use finstack_core::HashMap;
//!
//! # fn example() -> finstack_core::Result<()> {
//! // Ensure conventions are loaded
//! let _registry = ConventionRegistry::try_global()?;
//!
//! // Create build context
//! let as_of = Date::from_calendar_date(2024, time::Month::January, 2).unwrap();
//! let ctx = BuildCtx::new(as_of, 1_000_000.0, HashMap::default());
//!
//! // Create a deposit quote
//! let quote = RateQuote::Deposit {
//!     id: QuoteId::new("USD-SOFR-DEP-1M"),
//!     index: IndexId::new("USD-SOFR-1M"),
//!     pillar: Pillar::Tenor("1M".parse().unwrap()),
//!     rate: 0.0525,
//! };
//!
//! // Build the instrument
//! let instrument = build_rate_instrument(&quote, &ctx)?;
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`crate::market::BuildCtx`] for build context configuration
//! - [`crate::market::conventions::ConventionRegistry`] for convention lookups
//! - [`crate::market::quotes::market_quote::MarketQuote`] for the unified quote enum
//!
//! # References
//!
//! - Day-count and business-day conventions: `docs/REFERENCES.md#isda-2006-definitions`
//! - Bond-market conventions: `docs/REFERENCES.md#icma-rule-book`
//! - FX volatility and market conventions: `docs/REFERENCES.md#clark-fx-options`

/// Quote-to-instrument builders and prepared quotes.
pub(crate) mod build;
/// Market conventions and registries.
pub mod conventions;
/// Market quote schemas.
pub mod quotes;

pub use build::cds::build_cds_instrument;
pub use build::cds_tranche::{build_cds_tranche_instrument, CDSTrancheBuildOverrides};
pub use build::context::BuildCtx;
pub use build::rates::build_rate_instrument;
pub use build::xccy::build_xccy_instrument;
