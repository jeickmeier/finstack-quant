#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]
#![doc(test(attr(allow(clippy::expect_used))))]

//! # Finstack Quant Statements Analytics
//!
//! Higher-level analysis, reporting, and extension implementations that build
//! on the core [`finstack_quant_statements`] evaluation engine.
//!
//! This crate provides:
//!
//! - **Analysis** — sensitivity, scenario sets, variance, DCF, goal seek,
//!   covenants, backtesting, Monte Carlo, and introspection
//! - **Extensions** — concrete analytics extensions (corkscrew, credit
//!   scorecard) called directly via inherent methods
//! - **Templates** — real estate, roll-forward, and vintage model builders
//!
//! # Module Guide
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`analysis`] | DCF valuation, scenario sets, sensitivity, goal seek, backtesting, introspection, reports, covenant forecasting, ECL |
//! | [`extensions`] | Corkscrew roll-forward validation and credit scorecard rating assignment |
//! | [`templates`] | Real estate, roll-forward, and vintage model builders |
//!
//! # Quick Start
//!
//! [`CorporateAnalysisBuilder`] evaluates a model once and optionally adds DCF
//! equity valuation and per-instrument credit context:
//!
//! ```no_run
//! use finstack_quant_core::dates::PeriodId;
//! use finstack_quant_statements::builder::ModelBuilder;
//! use finstack_quant_statements::types::AmountOrScalar;
//! use finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder;
//! use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let model = ModelBuilder::new("lbo-demo")
//!     .periods("2025Q1..Q4", None)?
//!     .value(
//!         "revenue",
//!         &[
//!             (PeriodId::quarter(2025, 1), AmountOrScalar::scalar(10_000_000.0)),
//!             (PeriodId::quarter(2025, 2), AmountOrScalar::scalar(10_500_000.0)),
//!             (PeriodId::quarter(2025, 3), AmountOrScalar::scalar(11_000_000.0)),
//!             (PeriodId::quarter(2025, 4), AmountOrScalar::scalar(11_500_000.0)),
//!         ],
//!     )
//!     .compute("ebitda", "revenue * 0.25")?
//!     .compute("ufcf", "ebitda * 0.6")?
//!     .with_meta("currency", serde_json::json!("USD"))
//!     .build()?;
//!
//! let analysis = CorporateAnalysisBuilder::new(model)
//!     .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
//!     .net_debt_override(20_000_000.0)
//!     .coverage_node("ebitda")
//!     .analyze()?;
//!
//! if let Some(equity) = &analysis.equity {
//!     println!("Equity value: {}", equity.equity_value);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Conventions
//!
//! - Ratios (DSCR, coverage, leverage, valuation multiples) are returned as
//!   plain scalars: `2.0` means `2.0x`.
//! - Percentage-style inputs (WACC, growth) follow the decimal convention:
//!   `0.10` means `10%`.
//! - Scenario overrides are deterministic full-period scalar overrides unless a
//!   lower-level API states otherwise.

/// Analysis tools for financial statement models.
pub mod analysis;

/// Concrete extension implementations (corkscrew, credit scorecard).
pub mod extensions;

/// Templates for common financial model structures.
pub mod templates;
