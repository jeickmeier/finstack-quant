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

//! Vectorized panel feature transforms.

mod cross_sectional;
mod panel;
mod timeseries;
mod types;

pub use cross_sectional::{
    transform_cross_sectional, transform_cross_sectional_with_op, CrossSectionalOp,
};
pub use panel::{
    transform_panel, transform_panel_spec, PanelOperation, PanelTransformColumn,
    PanelTransformResult, PanelTransformSpec,
};
pub use timeseries::{transform_timeseries, transform_timeseries_with_op, TimeSeriesOp};
