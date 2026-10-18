//! Valuation-owned market conventions and pricing-time volatility resolution.
//!
//! This module contains:
//!
//! 1. **Conventions** (`conventions/`): Market convention registries loaded from embedded JSON
//!    data. Conventions define day count, business day adjustments, payment frequencies, and
//!    other market-standard parameters required for instrument construction.
//!
//! 2. **Listed catalog** (`listed/`): Maintained exchange product-family coverage and routing
//!    metadata for canonical asset-class instruments.
//!
//! 3. **Volatility resolution**: Pricing-time selection of already-built
//!    volatility inputs.
//!
//! # Documentation Rules For Market APIs
//!
//! Market-facing docs should explicitly call out:
//!
//! - day count, calendar, spot lag, and settlement assumptions when conventions are resolved
//! - which curve-role mappings are required versus which are convention-derived fallbacks
//! - whether the API is convention lookup or pricing-time market resolution
//!
//! Raw quote DTOs and quote-to-instrument construction live in
//! `finstack-quant-calibration`.
//!
//! # References
//!
//! - Day-count and business-day conventions: `docs/REFERENCES.md#isda-2006-definitions`
//! - Bond-market conventions: `docs/REFERENCES.md#icma-rule-book`
//! - FX volatility and market conventions: `docs/REFERENCES.md#clark-fx-options`

/// Market conventions and registries.
pub mod conventions;
/// Exchange-listed product-family coverage and valuation routes.
pub mod listed;
pub mod volatility;

pub use volatility::resolve_vol_source;
