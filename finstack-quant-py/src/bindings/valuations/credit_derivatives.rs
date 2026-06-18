//! Python wrappers for CDS-family instruments.

use super::direct_wrapper::{
    from_json_payload, pretty_json, price_payload_result, validate_payload,
};
use super::PyValuationResult;
use crate::errors::display_to_py;
use finstack_quant_valuations::instruments::credit_derivatives::cds::CreditDefaultSwap;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::CDSIndex;
use finstack_quant_valuations::instruments::credit_derivatives::cds_option::CDSOption;
use finstack_quant_valuations::instruments::credit_derivatives::cds_tranche::CDSTranche;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyList, PyModule};
use serde::Serialize;

fn example_payload<T: Serialize>(type_tag: &str, instrument: &T) -> PyResult<String> {
    let value = serde_json::to_value(instrument).map_err(display_to_py)?;
    finstack_quant_valuations::pricer::canonical_instrument_json(type_tag, value)
        .map_err(display_to_py)
}

macro_rules! credit_derivative_wrapper {
    ($py_name:literal, $py_struct:ident, $rust_ty:ty, $type_tag:literal, $example:expr) => {
        #[pyclass(name = $py_name, module = "finstack_quant.valuations.instruments.credit_derivatives", skip_from_py_object)]
        #[derive(Clone)]
        struct $py_struct {
            json: String,
        }

        #[pymethods]
        impl $py_struct {
            #[staticmethod]
            fn example() -> PyResult<Self> {
                let instrument: $rust_ty = $example.map_err(display_to_py)?;
                Ok(Self {
                    json: example_payload($type_tag, &instrument)?,
                })
            }

            #[staticmethod]
            fn from_json(json: &str) -> PyResult<Self> {
                Ok(Self {
                    json: from_json_payload($type_tag, json)?,
                })
            }

            fn to_json(&self) -> PyResult<String> {
                pretty_json(&self.json)
            }

            fn validate(&self) -> PyResult<()> {
                validate_payload(&self.json)
            }

            fn price(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
            ) -> PyResult<PyValuationResult> {
                // "default" resolves to `Instrument::default_model()` in the
                // pricer JSON layer, so each instrument always prices with its
                // registered model (a hardcoded literal here broke CDSOption
                // when its model moved from Black76 to BloombergCdso).
                let result = price_payload_result(py, &self.json, market, as_of, "default")?;
                Ok(PyValuationResult { inner: result })
            }
        }
    };
}

credit_derivative_wrapper!(
    "CreditDefaultSwap",
    PyCreditDefaultSwap,
    CreditDefaultSwap,
    "credit_default_swap",
    Ok::<CreditDefaultSwap, finstack_quant_core::Error>(CreditDefaultSwap::example())
);

credit_derivative_wrapper!(
    "CDSIndex",
    PyCDSIndex,
    CDSIndex,
    "cds_index",
    Ok::<CDSIndex, finstack_quant_core::Error>(CDSIndex::example())
);

credit_derivative_wrapper!(
    "CDSTranche",
    PyCDSTranche,
    CDSTranche,
    "cds_tranche",
    Ok::<CDSTranche, finstack_quant_core::Error>(CDSTranche::example())
);

credit_derivative_wrapper!(
    "CDSOption",
    PyCDSOption,
    CDSOption,
    "cds_option",
    CDSOption::example()
);

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(py, "credit_derivatives")?;
    module.add_class::<PyCreditDefaultSwap>()?;
    module.add_class::<PyCDSIndex>()?;
    module.add_class::<PyCDSTranche>()?;
    module.add_class::<PyCDSOption>()?;
    let all = PyList::new(
        py,
        ["CreditDefaultSwap", "CDSIndex", "CDSTranche", "CDSOption"],
    )?;
    module.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &module,
        "credit_derivatives",
        "finstack_quant.finstack_quant.valuations",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;
    Ok(())
}
