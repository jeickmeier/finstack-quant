//! Python wrapper for the statement-result rollup specification.
//!
//! The rollup itself is ``StatementResult.rollup`` (see `evaluator.rs`),
//! mirroring Rust's `StatementResult::rollup`.

use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, serde_json_to_py};
use finstack_quant_core::dates::PeriodKind;
use finstack_quant_statements::evaluator::{RollupMethod, RollupSpec as RustRollupSpec};
use indexmap::IndexMap;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.statements._rollup", "RollupSpec")]
#[pyclass(
    name = "RollupSpec",
    module = "finstack_quant.statements",
    from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyRollupSpec {
    pub(crate) inner: RustRollupSpec,
}

wire_methods!(
    PyRollupSpec,
    RustRollupSpec,
    "RollupSpec",
    module = "finstack_quant.statements._rollup"
);

#[pymethods]
impl PyRollupSpec {
    #[new]
    #[pyo3(signature = (frequency, methods=None))]
    fn new(frequency: &str, methods: Option<IndexMap<String, String>>) -> PyResult<Self> {
        let frequency: PeriodKind = frequency.parse().map_err(core_to_py)?;
        let methods = methods
            .unwrap_or_default()
            .into_iter()
            .map(|(node_id, method)| {
                let method: RollupMethod =
                    finstack_quant_core::wire::serde_parse(&method).map_err(core_to_py)?;
                Ok((node_id, method))
            })
            .collect::<PyResult<_>>()?;
        Ok(Self {
            inner: RustRollupSpec { frequency, methods },
        })
    }

    #[doc = pydoc!("finstack_quant.statements._rollup", "RollupSpec.frequency")]
    #[getter]
    fn frequency(&self) -> String {
        self.inner.frequency.to_string()
    }

    #[doc = pydoc!("finstack_quant.statements._rollup", "RollupSpec.methods")]
    #[getter]
    fn methods(&self) -> PyResult<IndexMap<String, String>> {
        self.inner
            .methods
            .iter()
            .map(|(node_id, method)| {
                let label = finstack_quant_core::wire::serde_label(method).map_err(core_to_py)?;
                Ok((node_id.clone(), label))
            })
            .collect()
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("RollupSpec", &self.inner)
    }
}

/// Accept a typed ``RollupSpec`` or its JSON serialization.
pub(crate) fn extract_rollup_spec(value: &Bound<'_, PyAny>) -> PyResult<RustRollupSpec> {
    if let Ok(spec) = value.extract::<PyRef<'_, PyRollupSpec>>() {
        return Ok(spec.inner.clone());
    }
    serde_json::from_str(value.extract::<&str>()?)
        .map_err(|e| serde_json_to_py(e, "invalid RollupSpec JSON"))
}

pub fn register(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRollupSpec>()?;
    Ok(())
}
