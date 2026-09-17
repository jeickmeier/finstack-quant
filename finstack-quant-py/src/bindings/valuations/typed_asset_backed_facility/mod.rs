//! Typed asset-backed facility (warehouse line) surface: the
//! `AssetBackedFacility` instrument, its builder and the `FacilityProjection`
//! result.

mod facility;

use pyo3::prelude::*;

pub(crate) use facility::{
    PyAssetBackedFacility, PyAssetBackedFacilityBuilder, PyFacilityProjection,
};

/// Register the typed asset-backed facility classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAssetBackedFacility>()?;
    m.add_class::<PyAssetBackedFacilityBuilder>()?;
    m.add_class::<PyFacilityProjection>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
pub(crate) const EXPORTS: &[&str] = &[
    "AssetBackedFacility",
    "AssetBackedFacilityBuilder",
    "FacilityProjection",
];
