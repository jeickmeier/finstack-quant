//! Python bindings for `finstack_core::market_data::term_structures` curve types.

mod credit;
mod discount;
mod forward;
mod hazard;
mod helpers;
mod inflation;
mod price;
mod surfaces;

pub use credit::{PyBaseCorrelationCurve, PyCreditIndexData};
pub use discount::PyDiscountCurve;
pub use forward::PyForwardCurve;
pub use hazard::PyHazardCurve;
pub use inflation::PyInflationCurve;
pub use price::PyPriceCurve;
pub use surfaces::{PyFxDeltaVolSurface, PyVolCube, PyVolSurface, PyVolatilityIndexCurve};

use pyo3::prelude::*;
use pyo3::types::PyList;

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub(super) const EXPORTS: &[&str] = &[
    "BaseCorrelationCurve",
    "CreditIndexData",
    "DiscountCurve",
    "ForwardCurve",
    "FxDeltaVolSurface",
    "HazardCurve",
    "InflationCurve",
    "PriceCurve",
    "VolSurface",
    "VolCube",
    "VolatilityIndexCurve",
];

/// Register the `finstack.core.market_data.curves` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "curves")?;
    m.setattr(
        "__doc__",
        "Market-data bindings: discount, forward, hazard, inflation, price, vol surface, vol cube, and vol-index.",
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
    m.add_class::<PyVolCube>()?;
    m.add_class::<PyVolatilityIndexCurve>()?;

    let all = PyList::new(py, EXPORTS)?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "curves",
        "finstack.core.market_data",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
