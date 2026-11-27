//! Python bindings for vectorized panel feature transforms.
//!
//! Every transform takes row-aligned Python lists, releases the GIL for the
//! Rust kernel, and returns a list of ``float | None`` aligned to the input.
//! Key columns (``entity``, ``order``, ``time_key``, ``groups``) accept
//! strings and date-like objects only. Datetimes and pandas timestamps are
//! formatted by the Rust key policy (``datetime_order_key``: aware values
//! normalize to UTC; naive values keep wall time; fixed nanosecond precision);
//! dates use ``isoformat()``. Ints, floats, bools, durations (``pd.Timedelta``)
//! and other objects raise ``TypeError`` (as in WASM, where keys are strings);
//! convert integer period keys to zero-padded strings. Mixed aware/naive datetime keys fail; opaque
//! strings retain caller ordering.

mod ops;
mod panel;
mod parse;
mod transforms;

use ops::{PyCrossSectionalOp, PyPairwiseOp, PyTimeSeriesOp};
use panel::{PyPanelTransformResult, PyPanelTransformSpec, transform_panel};
use pyo3::prelude::*;
use pyo3::types::PyList;
use transforms::{
    neutralize, neutralize_and_zscore, rank_to_weights, risk_scaled_weights,
    rolling_regression_residual, transform_cross_sectional, transform_cross_sectional_grouped,
    transform_timeseries, transform_timeseries_pairwise,
};

/// Register the features submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "features")?;
    m.setattr("__doc__", "Vectorized panel feature transforms.")?;
    m.add_class::<PyCrossSectionalOp>()?;
    m.add_class::<PyPairwiseOp>()?;
    m.add_class::<PyPanelTransformResult>()?;
    m.add_class::<PyPanelTransformSpec>()?;
    m.add_class::<PyTimeSeriesOp>()?;
    m.add_function(wrap_pyfunction!(transform_timeseries, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_cross_sectional, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_cross_sectional_grouped, &m)?)?;
    m.add_function(wrap_pyfunction!(neutralize, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_timeseries_pairwise, &m)?)?;
    m.add_function(wrap_pyfunction!(rolling_regression_residual, &m)?)?;
    m.add_function(wrap_pyfunction!(risk_scaled_weights, &m)?)?;
    m.add_function(wrap_pyfunction!(rank_to_weights, &m)?)?;
    m.add_function(wrap_pyfunction!(neutralize_and_zscore, &m)?)?;
    m.add_function(wrap_pyfunction!(transform_panel, &m)?)?;
    let all = PyList::new(
        py,
        [
            "CrossSectionalOp",
            "PairwiseOp",
            "PanelTransformResult",
            "PanelTransformSpec",
            "TimeSeriesOp",
            "neutralize",
            "neutralize_and_zscore",
            "rank_to_weights",
            "risk_scaled_weights",
            "rolling_regression_residual",
            "transform_cross_sectional",
            "transform_cross_sectional_grouped",
            "transform_panel",
            "transform_timeseries",
            "transform_timeseries_pairwise",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )
}
