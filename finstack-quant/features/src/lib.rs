//! Vectorized panel feature transforms for finstack-quant.
//!
//! This crate turns a flat value column plus grouping keys into derived feature
//! columns, either backward-looking per entity (time-series) or partitioned per
//! timestamp (cross-sectional). Values are `Option<f64>`; `None` and non-finite
//! inputs are excluded. Rolling aggregates may emit at a missing row when
//! the trailing row window has enough finite observations. Current-value
//! transforms require a finite current input; fill/mask ops handle missing rows.
//!
//! # Module Guide
//!
//! - **Time-series** — Backward-looking transforms per entity (returns, rolling
//!   stats, EWMA, drawdown, Hampel filter, …). Entry point:
//!   [`transform_timeseries`] / [`transform_timeseries_with_op`].
//! - **Cross-sectional** — Transforms across entities within each time
//!   partition (rank, z-score, normalize, winsorize, …). Entry point:
//!   [`transform_cross_sectional`] / [`transform_cross_sectional_with_op`].
//! - **Panel** — JSON-specified or typed-spec pipeline of named time-series and
//!   cross-sectional operations. Entry point: [`transform_panel_json`] /
//!   [`transform_panel`].
//! - **Multi** — Pairwise and grouped transforms, signal cleaning, and
//!   neutralization helpers. Entry point: [`neutralize`],
//!   [`transform_timeseries_pairwise`], [`transform_cross_sectional_grouped`].
//!
//! # Quick Start
//!
//! ```rust
//! use finstack_quant_features::{transform_timeseries_with_op, TimeSeriesOp};
//!
//! let values = vec![Some(100.0), Some(102.0), Some(101.0), Some(105.0)];
//! let entity = vec!["A".to_string(), "A".to_string(), "A".to_string(), "A".to_string()];
//! let order = vec!["1".to_string(), "2".to_string(), "3".to_string(), "4".to_string()];
//! let result = transform_timeseries_with_op(
//!     &values, &entity, &order,
//!     TimeSeriesOp::Returns, None,
//! )?;
//! assert_eq!(result[0], None);
//! assert!((result[1].unwrap() - 0.02).abs() < 1e-12);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```
//!
//! # Conventions
//!
//! - Keys are opaque; time order is lexicographic (use ISO-8601 for calendar
//!   chronology, with one timezone and fixed precision).
//! - `periods` (`returns`, `log_returns`, `diff`, `lag`) counts finite
//!   observations (observation time): a `None` row never advances the lag, so
//!   `v_t` is compared with the `periods`-th finite value before it. Missing
//!   rows do not decay EWMA or half-life. Rolling `window`s span the trailing
//!   `window` rows of the entity; only finite rows inside the window
//!   contribute and `min_periods` of them are required (`min_periods` cannot
//!   exceed `window`). Rolling aggregates can still emit at a missing current
//!   row.
//! - `params` are strict: a key the operation does not read (for example
//!   `"windows"`) is rejected instead of silently falling back to the default.
//! - Outputs preserve input order and length; element `i` of the output
//!   corresponds to element `i` of `values`. Non-finite arithmetic results
//!   are validation errors on every surface.
//! - EWMA requires `span >= 1` and uses centered biased variance. Volatility
//!   is missing on the first finite observation, then zero for constant data.
//! - `cap_weights` enforces final caps, zero net and unit gross; infeasible
//!   sign-preserving allocations fail. `neutralize_and_zscore` requires an intercept.
//! - `drawdown` takes a level series; `rolling_sharpe` is a period feature,
//!   not the `analytics` Sharpe. `transform_panel_json` is sequential.
//! - String/JSON entry points are retained for Python and WASM bindings; Rust
//!   callers should use the typed-op variants (`TimeSeriesOp`,
//!   `CrossSectionalOp`, `PairwiseOp`, `PanelTransformSpec`).

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

mod cross_sectional;
mod index;
mod multi;
mod panel;
mod timeseries;
mod types;

pub use cross_sectional::{
    transform_cross_sectional, transform_cross_sectional_with_op, CrossSectionalOp,
};
pub use multi::{
    neutralize, neutralize_and_zscore, rank_to_weights, risk_scaled_weights,
    rolling_regression_residual, transform_cross_sectional_grouped,
    transform_cross_sectional_grouped_with_op, transform_timeseries_pairwise,
    transform_timeseries_pairwise_with_op, PairwiseOp,
};
pub use panel::{
    transform_panel, transform_panel_json, PanelOperation, PanelTransformColumn,
    PanelTransformResult, PanelTransformSpec,
};
pub use timeseries::{transform_timeseries, transform_timeseries_with_op, TimeSeriesOp};

/// Compiles the crate `README.md` Rust samples as doctests.
///
/// The README is *not* included in the rendered crate documentation — this
/// item exists only under `cfg(doctest)` so that every ` ```rust ` block in the
/// README is compiled and run by `cargo test --doc`. Without it those samples
/// are dead text and rot silently on any API change.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
