//! Vectorized panel feature transforms for finstack-quant.
//!
//! This crate turns a flat value column plus grouping keys into derived feature
//! columns, either backward-looking per entity (time-series) or partitioned per
//! timestamp (cross-sectional). Values are `Option<f64>`; `None` and non-finite
//! inputs are skipped and produce `None` outputs, so callers can carry missing
//! data through a pipeline without sentinel values.
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
//!   cross-sectional operations. Entry point: [`transform_panel`] /
//!   [`transform_panel_spec`].
//! - **Multi** — Pairwise and grouped transforms, signal cleaning, and
//!   neutralization helpers. Entry point: [`neutralize`], [`clean_signal`],
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
//! - Outputs preserve input order and length; element `i` of the output
//!   corresponds to element `i` of `values`.
//! - `None` and non-finite inputs are skipped; they produce `None` outputs.
//! - Rolling operations require `min_periods` finite points in the window
//!   before emitting a value.
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
mod multi;
mod panel;
mod timeseries;
mod types;

pub use cross_sectional::{
    transform_cross_sectional, transform_cross_sectional_with_op, CrossSectionalOp,
};
pub use multi::{
    clean_signal, neutralize, neutralize_and_zscore, normalize_signal, rank_to_weights,
    risk_scaled_weights, rolling_regression_residual, transform_cross_sectional_grouped,
    transform_cross_sectional_grouped_with_op, transform_timeseries_pairwise,
    transform_timeseries_pairwise_with_op, PairwiseOp,
};
pub use panel::{
    transform_panel, transform_panel_spec, PanelOperation, PanelTransformColumn,
    PanelTransformResult, PanelTransformSpec,
};
pub use timeseries::{transform_timeseries, transform_timeseries_with_op, TimeSeriesOp};
