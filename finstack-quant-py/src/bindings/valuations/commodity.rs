//! Direct Python wrappers for commodity valuation instruments.

use super::direct_wrapper::{
    build_from_py, from_json_payload, pretty_json, price_payload, price_payload_with_metrics,
    validate_payload,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList, PyModule};

macro_rules! commodity_class {
    ($py_name:literal, $rust_name:ident, $type_tag:literal) => {
        #[pyclass(
            name = $py_name,
            module = "finstack_quant.valuations.instruments.commodity",
            skip_from_py_object
        )]
        #[derive(Clone)]
        struct $rust_name {
            json: String,
        }

        #[pymethods]
        impl $rust_name {
            #[new]
            #[pyo3(signature = (spec=None, **kwargs))]
            fn new(
                py: Python<'_>,
                spec: Option<&Bound<'_, PyAny>>,
                kwargs: Option<&Bound<'_, PyDict>>,
            ) -> PyResult<Self> {
                Ok(Self {
                    json: build_from_py(
                        py,
                        $type_tag,
                        spec,
                        kwargs,
                        "commodity instrument spec",
                        "commodity instrument constructor requires a spec object, JSON string, or keyword fields",
                    )?,
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

            #[pyo3(signature = (market, as_of, model="default"))]
            fn price(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<String> {
                price_payload(py, &self.json, market, as_of, model)
            }

            #[pyo3(signature = (market, as_of, model="default", metrics=vec![], pricing_options=None, market_history=None))]
            // PyO3 binding: the argument list mirrors the Python
            // keyword-argument API, so it cannot be collapsed into a
            // parameter struct without changing that API.
            #[allow(clippy::too_many_arguments)]
            fn price_with_metrics(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
                metrics: Vec<String>,
                pricing_options: Option<&str>,
                market_history: Option<&str>,
            ) -> PyResult<String> {
                price_payload_with_metrics(
                    py,
                    &self.json,
                    market,
                    as_of,
                    model,
                    metrics,
                    pricing_options,
                    market_history,
                )
            }

            fn __repr__(&self) -> String {
                concat!($py_name, "(...)").to_string()
            }
        }
    };
}

commodity_class!("CommodityOption", PyCommodityOption, "commodity_option");
commodity_class!(
    "CommodityAsianOption",
    PyCommodityAsianOption,
    "commodity_asian_option"
);
commodity_class!("CommodityForward", PyCommodityForward, "commodity_forward");
commodity_class!("CommoditySwap", PyCommoditySwap, "commodity_swap");
commodity_class!(
    "CommoditySwaption",
    PyCommoditySwaption,
    "commodity_swaption"
);
commodity_class!(
    "CommoditySpreadOption",
    PyCommoditySpreadOption,
    "commodity_spread_option"
);

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "commodity")?;
    m.setattr("__doc__", "Direct commodity valuation instrument wrappers.")?;

    m.add_class::<PyCommodityOption>()?;
    m.add_class::<PyCommodityAsianOption>()?;
    m.add_class::<PyCommodityForward>()?;
    m.add_class::<PyCommoditySwap>()?;
    m.add_class::<PyCommoditySwaption>()?;
    m.add_class::<PyCommoditySpreadOption>()?;

    let all = PyList::new(
        py,
        [
            "CommodityOption",
            "CommodityAsianOption",
            "CommodityForward",
            "CommoditySwap",
            "CommoditySwaption",
            "CommoditySpreadOption",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "commodity",
        "finstack_quant.finstack_quant.valuations",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
