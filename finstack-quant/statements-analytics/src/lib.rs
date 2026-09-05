#![forbid(unsafe_code)]
#![warn(clippy::float_cmp)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
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
//!   covenants, backtesting, and introspection
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
//! [`analysis::CorporateAnalysisBuilder`] evaluates a model once and optionally adds DCF
//! equity valuation and per-instrument credit context:
//!
//! ```no_run
//! use finstack_quant_core::{currency::Currency, dates::PeriodId, money::Money};
//! use finstack_quant_statements::builder::ModelBuilder;
//! use finstack_quant_statements::checks::{builtins::NonFiniteCheck, CheckSuite};
//! use finstack_quant_statements_analytics::analysis::CorporateAnalysisBuilder;
//! use finstack_quant_valuations::instruments::equity::dcf_equity::TerminalValueSpec;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let model = ModelBuilder::new("lbo-demo")
//!     .periods("2025Q1..Q4", None)?
//!     .value_money(
//!         "revenue",
//!         &[
//!             (PeriodId::quarter(2025, 1).expect("valid period fixture"), Money::from((10_000_000_i64, Currency::USD))),
//!             (PeriodId::quarter(2025, 2).expect("valid period fixture"), Money::from((10_500_000_i64, Currency::USD))),
//!             (PeriodId::quarter(2025, 3).expect("valid period fixture"), Money::from((11_000_000_i64, Currency::USD))),
//!             (PeriodId::quarter(2025, 4).expect("valid period fixture"), Money::from((11_500_000_i64, Currency::USD))),
//!         ],
//!     )
//!     .compute("ebitda", "revenue * 0.25")?
//!     .compute("ufcf", "ebitda * 0.6")?
//!     .with_meta("currency", serde_json::json!("USD"))
//!     .build()?;
//!
//! let checks = CheckSuite::builder("corporate")
//!     .add_check(NonFiniteCheck { nodes: vec![] })
//!     .build();
//!
//! let analysis = CorporateAnalysisBuilder::new(model)
//!     .dcf(0.10, TerminalValueSpec::GordonGrowth { growth_rate: 0.02 })
//!     .net_debt_override(20_000_000.0)
//!     .checks(checks)
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
//!
//! # References
//!
//! - Discounting and DCF context: `docs/REFERENCES.md#hull-options-futures`
//! - Coverage and leverage interpretation: `docs/REFERENCES.md#tuckman-serrat-fixed-income`

/// Analysis tools for financial statement models.
pub mod analysis;

/// Concrete extension implementations (corkscrew, credit scorecard).
pub mod extensions;

/// Templates for common financial model structures.
pub mod templates;

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
