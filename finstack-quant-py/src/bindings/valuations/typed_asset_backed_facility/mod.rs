//! Typed asset-backed facility (warehouse line) surface: the
//! `AssetBackedFacility` instrument, its builder and the `FacilityProjection`
//! result.

mod facility;
mod facility_terms;

use pyo3::prelude::*;

pub(crate) use facility::{
    PyAssetBackedFacility, PyAssetBackedFacilityBuilder, PyFacilityProjection,
};
pub(crate) use facility_terms::{PyAmortizationEvent, PyTermOutSpec};

/// Register the typed asset-backed facility classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAssetBackedFacility>()?;
    m.add_class::<PyAssetBackedFacilityBuilder>()?;
    m.add_class::<PyFacilityProjection>()?;
    m.add_class::<PyTermOutSpec>()?;
    m.add_class::<PyAmortizationEvent>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &[
    "AmortizationEvent",
    "AssetBackedFacility",
    "AssetBackedFacilityBuilder",
    "FacilityProjection",
    "TermOutSpec",
];
