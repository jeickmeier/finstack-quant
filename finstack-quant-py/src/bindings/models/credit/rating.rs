//! Moody's WARF factor and rating-factor table Python bindings.

use crate::bindings::core::types::extract_credit_rating;
use crate::bindings::macros::{py_getters, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_models::credit::{
    RatingFactorTable, moodys_warf_factor as rust_moodys_warf_factor,
};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.models.credit._rating", "moodys_warf_factor")]
#[pyfunction]
#[pyo3(text_signature = "(rating)")]
pub(super) fn moodys_warf_factor(rating: &Bound<'_, PyAny>) -> PyResult<f64> {
    let rating = extract_credit_rating(rating)?;
    rust_moodys_warf_factor(rating).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.credit._rating", "RatingFactorTable")]
#[pyclass(
    name = "RatingFactorTable",
    module = "finstack_quant.models.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyRatingFactorTable {
    pub(crate) inner: RatingFactorTable,
}

#[pymethods]
impl PyRatingFactorTable {
    #[doc = pydoc!("finstack_quant.models.credit._rating", "RatingFactorTable.default_calibration")]
    #[staticmethod]
    #[pyo3(text_signature = "()")]
    fn default_calibration() -> PyResult<Self> {
        RatingFactorTable::default_calibration()
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._rating", "RatingFactorTable.from_registry_id")]
    #[staticmethod]
    #[pyo3(text_signature = "(id)")]
    fn from_registry_id(id: &str) -> PyResult<Self> {
        RatingFactorTable::from_registry_id(id)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._rating", "RatingFactorTable.get_factor")]
    #[pyo3(text_signature = "($self, rating)")]
    fn get_factor(&self, rating: &Bound<'_, PyAny>) -> PyResult<f64> {
        let rating = extract_credit_rating(rating)?;
        self.inner.get_factor(rating).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.credit._rating", "RatingFactorTable.__repr__")]
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RatingFactorTable", &self.inner)
    }
}

py_getters!(PyRatingFactorTable |s| {
    #[doc = pydoc!("finstack_quant.models.credit._rating", "RatingFactorTable.agency")]
    agency -> String = s.inner.agency().to_string();
    #[doc = pydoc!("finstack_quant.models.credit._rating", "RatingFactorTable.methodology")]
    methodology -> String = s.inner.methodology().to_string();
    #[doc = pydoc!("finstack_quant.models.credit._rating", "RatingFactorTable.default_factor")]
    default_factor -> f64 = s.inner.default_factor();
});

wire_methods!(
    PyRatingFactorTable,
    RatingFactorTable,
    "RatingFactorTable",
    module = "finstack_quant.models.credit._rating"
);
