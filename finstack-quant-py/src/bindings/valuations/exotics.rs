//! Direct Python wrappers for exotic valuation instruments.

use super::direct_wrapper::{
    build_from_py, from_json_payload, greeks_dict, metric_value, pretty_json, price_payload,
    price_payload_with_metrics, validate_payload,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

macro_rules! exotic_class {
    ($py_name:literal, $rust_name:ident, $type_tag:literal) => {
        #[pyclass(
            name = $py_name,
            module = "finstack_quant.valuations.exotics",
            skip_from_py_object
        )]
        #[derive(Clone)]
        pub(crate) struct $rust_name {
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
                        "exotic instrument spec",
                        "exotic instrument constructor requires a spec object, JSON string, or keyword fields",
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

macro_rules! exotic_option_class {
    ($py_name:literal, $rust_name:ident, $type_tag:literal) => {
        exotic_class!($py_name, $rust_name, $type_tag);

        #[pymethods]
        impl $rust_name {
            #[pyo3(signature = (market, as_of, model="default"))]
            fn delta(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<f64> {
                metric_value(py, &self.json, market, as_of, model, "delta")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn gamma(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<f64> {
                metric_value(py, &self.json, market, as_of, model, "gamma")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn vega(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<f64> {
                metric_value(py, &self.json, market, as_of, model, "vega")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn theta(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<f64> {
                metric_value(py, &self.json, market, as_of, model, "theta")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn rho(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<f64> {
                metric_value(py, &self.json, market, as_of, model, "rho")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn vanna(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<f64> {
                metric_value(py, &self.json, market, as_of, model, "vanna")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn volga(
                &self,
                py: Python<'_>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<f64> {
                metric_value(py, &self.json, market, as_of, model, "volga")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn greeks<'py>(
                &self,
                py: Python<'py>,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<Bound<'py, PyDict>> {
                greeks_dict(py, &self.json, market, as_of, model)
            }
        }
    };
}

exotic_option_class!("AsianOption", PyAsianOption, "asian_option");
exotic_option_class!("BarrierOption", PyBarrierOption, "barrier_option");
exotic_option_class!("LookbackOption", PyLookbackOption, "lookback_option");
exotic_class!("Basket", PyBasket, "basket");

/// Register the `finstack_quant.valuations.exotics` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "exotics")?;
    m.setattr("__doc__", "Direct exotic valuation instrument wrappers.")?;

    m.add_class::<PyAsianOption>()?;
    m.add_class::<PyBarrierOption>()?;
    m.add_class::<PyLookbackOption>()?;
    m.add_class::<PyBasket>()?;

    let all = PyList::new(
        py,
        ["AsianOption", "BarrierOption", "LookbackOption", "Basket"],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "exotics",
        "finstack_quant.finstack_quant.valuations",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
