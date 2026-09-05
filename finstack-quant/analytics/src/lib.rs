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

//! Performance analytics on numeric slices and `finstack_quant_core::dates::Date`.
//!
//! [`Performance`] is the main entry point. Construct it from a price or
//! return panel and every performance analytic — return / risk scalars,
//! drawdown statistics, rolling windows, periodic returns (MTD / QTD / YTD /
//! FYTD), benchmark alpha / beta, basic factor models — is a method on the
//! resulting instance.
//!
//! Result and config types ([`PeriodStats`], [`DrawdownEpisode`],
//! [`BetaResult`], [`GreeksResult`], [`RollingGreeks`], [`MultiFactorResult`],
//! [`ReturnKind`], [`DatedSeries`], [`LookbackReturns`]) are re-exported here because
//! `Performance` returns them.
//!
//! Freestanding public exceptions are intentionally narrow:
//! - [`scalar`] exposes [`sharpe`], [`sortino`], [`volatility`] and
//!   [`max_drawdown`] over a single return slice for callers that do not
//!   need a panel.
//! - [`beta`] is kept public for cross-crate regression use.
//! - [`correlation`] owns shared row-major correlation-matrix validation and
//!   repair infrastructure used by valuations and factor-model crates.
//! - [`regression`] owns [`regression::constrained_least_squares`], an equality-constrained
//!   least-squares solver consumed by `finstack-quant-portfolio` for
//!   factor-Brinson attribution.
//!
//! Key conventions:
//! - returns are simple decimal returns
//! - annualization is derived from `finstack_quant_core::dates::PeriodKind`
//! - drawdown depths are non-positive fractions such as `-0.25` for a 25% loss
//! - benchmark inputs are assumed pre-aligned to the panel's date grid
//! - rolling series are right-labeled: each output value is dated by the last
//!   observation in its window
//!
//! # Quick start
//!
//! ```
//! use finstack_quant_analytics::Performance;
//! use finstack_quant_core::dates::{Date, Month, PeriodKind};
//!
//! let dates: Vec<Date> = (1..=10)
//!     .map(|d| Date::from_calendar_date(2025, Month::January, d).unwrap())
//!     .collect();
//! let prices = vec![(0..10).map(|i| 100.0 + i as f64).collect::<Vec<_>>()];
//! let perf = Performance::new(
//!     dates,
//!     prices,
//!     vec!["SPY".into()],
//!     None,
//!     PeriodKind::Daily,
//! ).unwrap();
//! assert_eq!(perf.ticker_names(), &["SPY"]);
//! ```
//!
//! # References
//!
//! - Sharpe ratio: `docs/REFERENCES.md#sharpe1966`
//! - Expected shortfall: `docs/REFERENCES.md#artzner1999CoherentRisk`
//! - Active-portfolio context: `docs/REFERENCES.md#grinoldKahn1999ActivePortfolio`

pub(crate) use finstack_quant_core::{dates, error, math};

pub(crate) type Result<T> = finstack_quant_core::Result<T>;

pub(crate) mod aggregation;
pub(crate) mod benchmark;
pub mod correlation;
pub(crate) mod drawdown;
pub(crate) mod lookback;
pub(crate) mod performance;
pub mod regression;
pub(crate) mod returns;
pub(crate) mod risk_metrics;
pub mod scalar;

pub use aggregation::PeriodStats;
pub use benchmark::{beta, BetaResult, GreeksResult, MultiFactorResult, ReturnKind, RollingGreeks};
pub use drawdown::DrawdownEpisode;
pub use performance::{LookbackReturns, Performance};
pub use risk_metrics::{CagrDayCount, DatedSeries};
pub use scalar::{max_drawdown, sharpe, sortino, volatility};

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
