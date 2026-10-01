//! Python bindings for `finstack_quant_core::market_data::term_structures` curve types.

pub mod credit;
mod cube_expiry_slice;
pub mod discount;
pub mod forward;
pub mod hazard;
pub(crate) mod helpers;
pub mod inflation;
pub mod price;
pub mod surfaces;

pub use credit::{PyBaseCorrelationCurve, PyCreditIndexData};
pub use cube_expiry_slice::PyVolCubeExpirySlice;
pub use discount::PyDiscountCurve;
pub use forward::PyForwardCurve;
pub use hazard::PyHazardCurve;
pub use inflation::PyInflationCurve;
pub use price::PyPriceCurve;
pub use surfaces::{PyFxDeltaVolSurface, PySabrParameterData, PyVolCube, PyVolSurface};

use pyo3::prelude::*;
use pyo3::types::PyList;

pub(super) const EXPORTS: &[&str] = &[
    "BaseCorrelationCurve",
    "CreditIndexData",
    "DiscountCurve",
    "ForwardCurve",
    "FxDeltaVolSurface",
    "HazardCurve",
    "InflationCurve",
    "PriceCurve",
    "SabrParameterData",
    "VolCube",
    "VolCubeExpirySlice",
    "VolSurface",
];

/// Register the `finstack_quant.core.market_data.curves` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "curves")?;
    m.setattr(
        "__doc__",
        "Market-data bindings: discount, forward, hazard, inflation, price (incl. vol-index) curves, vol surfaces, SABR cubes and FX delta surfaces.",
    )?;

    m.add_class::<PyDiscountCurve>()?;
    m.add_class::<PyForwardCurve>()?;
    m.add_class::<PyHazardCurve>()?;
    m.add_class::<PyBaseCorrelationCurve>()?;
    m.add_class::<PyCreditIndexData>()?;
    m.add_class::<PyInflationCurve>()?;
    m.add_class::<PyPriceCurve>()?;
    m.add_class::<PyVolSurface>()?;
    m.add_class::<PyFxDeltaVolSurface>()?;
    m.add_class::<PySabrParameterData>()?;
    m.add_class::<PyVolCube>()?;
    m.add_class::<PyVolCubeExpirySlice>()?;

    let all = PyList::new(py, EXPORTS)?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
