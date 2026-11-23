//! Python wrappers for margin domain types and enums.

mod csa;
mod enums;

pub use csa::{PyCsaSpec, PyEligibleCollateralSchedule};
pub(crate) use csa::{
    PyCsaSpecBuilder, PyOtcMarginSpec, PySimmCreditClassification, extract_otc_margin_spec,
};
pub use enums::{
    PyClearingStatus, PyCollateralAssetClass, PyImCollateralResult, PyImMethodology,
    PyMarginCallType, PyMarginTenor, PyNettingSetId,
};
pub(crate) use enums::{extract_asset_class, extract_netting_set_id};
use finstack_quant_margin as fm;
use pyo3::prelude::*;

/// Register all types in this module.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyImMethodology>()?;
    m.add_class::<PyMarginTenor>()?;
    m.add_class::<PyMarginCallType>()?;
    m.add_class::<PyClearingStatus>()?;
    m.add_class::<PyCollateralAssetClass>()?;
    m.add_class::<PyNettingSetId>()?;
    m.add_class::<PyCsaSpec>()?;
    m.add_class::<PyCsaSpecBuilder>()?;
    m.add_class::<PyOtcMarginSpec>()?;
    m.add_class::<PySimmCreditClassification>()?;
    m.add_class::<PyImCollateralResult>()?;
    m.add_class::<PyEligibleCollateralSchedule>()?;

    // `CONSTANTS` is the Rust `MarginConstants::current()` value: the crate
    // constants plus the registry/calculator constants hosts need to
    // interpret results.
    let constants =
        crate::bindings::json_bridge::serde_to_py(py, &fm::constants::MarginConstants::current())?;
    m.add("CONSTANTS", constants)?;

    Ok(())
}
