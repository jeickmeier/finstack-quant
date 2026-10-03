//! Shared conversion helpers for statements-analytics template bindings.

use crate::bindings::extract::extract_model_ref;
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::errors::statements_to_py;
use finstack_quant_statements::builder::{ModelBuilder, Ready};
use pyo3::prelude::*;

pub(crate) fn extract_builder(model: &Bound<'_, PyAny>) -> PyResult<ModelBuilder<Ready>> {
    ModelBuilder::from_spec(extract_model_ref(model)?.into_owned()).map_err(statements_to_py)
}

pub(crate) fn finish_builder(builder: ModelBuilder<Ready>) -> PyResult<PyFinancialModelSpec> {
    builder
        .build()
        .map(PyFinancialModelSpec::from_inner)
        .map_err(statements_to_py)
}
