//! Direct Python wrappers for FX valuation instruments.

use super::direct_wrapper::{
    build_from_py, from_json_payload, greeks_dict, metric_value, pretty_json, price_payload,
    price_payload_with_metrics,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};

macro_rules! fx_class {
    ($py_name:literal, $rust_name:ident, $type_tag:literal) => {
        #[pyclass(name = $py_name, module = "finstack.valuations.fx", skip_from_py_object)]
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
                        "FX instrument spec",
                        "FX instrument constructor requires a spec object, JSON string, or keyword fields",
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

            #[pyo3(signature = (market, as_of, model="default"))]
            fn price(
                &self,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<String> {
                price_payload(&self.json, market, as_of, model)
            }

            #[pyo3(signature = (market, as_of, model="default", metrics=vec![], pricing_options=None))]
            fn price_with_metrics(
                &self,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
                metrics: Vec<String>,
                pricing_options: Option<&str>,
            ) -> PyResult<String> {
                price_payload_with_metrics(
                    &self.json,
                    market,
                    as_of,
                    model,
                    metrics,
                    pricing_options,
                )
            }

            fn __repr__(&self) -> String {
                concat!($py_name, "(...)").to_string()
            }
        }
    };
}

macro_rules! fx_option_class {
    ($py_name:literal, $rust_name:ident, $type_tag:literal) => {
        fx_class!($py_name, $rust_name, $type_tag);

        #[pymethods]
        impl $rust_name {
            #[pyo3(signature = (market, as_of, model="default"))]
            fn delta(&self, market: &Bound<'_, PyAny>, as_of: &str, model: &str) -> PyResult<f64> {
                metric_value(&self.json, market, as_of, model, "delta")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn gamma(&self, market: &Bound<'_, PyAny>, as_of: &str, model: &str) -> PyResult<f64> {
                metric_value(&self.json, market, as_of, model, "gamma")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn vega(&self, market: &Bound<'_, PyAny>, as_of: &str, model: &str) -> PyResult<f64> {
                metric_value(&self.json, market, as_of, model, "vega")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn theta(&self, market: &Bound<'_, PyAny>, as_of: &str, model: &str) -> PyResult<f64> {
                metric_value(&self.json, market, as_of, model, "theta")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn rho(&self, market: &Bound<'_, PyAny>, as_of: &str, model: &str) -> PyResult<f64> {
                metric_value(&self.json, market, as_of, model, "rho")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn foreign_rho(
                &self,
                market: &Bound<'_, PyAny>,
                as_of: &str,
                model: &str,
            ) -> PyResult<f64> {
                metric_value(&self.json, market, as_of, model, "foreign_rho")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn vanna(&self, market: &Bound<'_, PyAny>, as_of: &str, model: &str) -> PyResult<f64> {
                metric_value(&self.json, market, as_of, model, "vanna")
            }

            #[pyo3(signature = (market, as_of, model="default"))]
            fn volga(&self, market: &Bound<'_, PyAny>, as_of: &str, model: &str) -> PyResult<f64> {
                metric_value(&self.json, market, as_of, model, "volga")
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

fx_class!("FxSpot", PyFxSpot, "fx_spot");
fx_class!("FxForward", PyFxForward, "fx_forward");
fx_class!("FxSwap", PyFxSwap, "fx_swap");
fx_class!("Ndf", PyNdf, "ndf");
fx_option_class!("FxOption", PyFxOption, "fx_option");
fx_option_class!("FxDigitalOption", PyFxDigitalOption, "fx_digital_option");
fx_option_class!("FxTouchOption", PyFxTouchOption, "fx_touch_option");
fx_option_class!("FxBarrierOption", PyFxBarrierOption, "fx_barrier_option");
fx_class!("FxVarianceSwap", PyFxVarianceSwap, "fx_variance_swap");
fx_option_class!("QuantoOption", PyQuantoOption, "quanto_option");

/// Register the `finstack.valuations.fx` submodule.
pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "fx")?;
    m.setattr("__doc__", "Direct FX valuation instrument wrappers.")?;

    m.add_class::<PyFxSpot>()?;
    m.add_class::<PyFxForward>()?;
    m.add_class::<PyFxSwap>()?;
    m.add_class::<PyNdf>()?;
    m.add_class::<PyFxOption>()?;
    m.add_class::<PyFxDigitalOption>()?;
    m.add_class::<PyFxTouchOption>()?;
    m.add_class::<PyFxBarrierOption>()?;
    m.add_class::<PyFxVarianceSwap>()?;
    m.add_class::<PyQuantoOption>()?;

    let all = PyList::new(
        py,
        [
            "FxSpot",
            "FxForward",
            "FxSwap",
            "Ndf",
            "FxOption",
            "FxDigitalOption",
            "FxTouchOption",
            "FxBarrierOption",
            "FxVarianceSwap",
            "QuantoOption",
        ],
    )?;
    m.setattr("__all__", all)?;

    parent.add_submodule(&m)?;
    parent.add("fx", &m)?;

    // Register the submodule under its fully-qualified path in `sys.modules`
    // so that `import finstack.valuations.fx` works the same as attribute
    // access on the parent. PyO3 sets `parent.__name__`; surface any failure
    // to read it instead of silently using a wrong fallback.
    let parent_name: String = parent.getattr("__name__")?.extract()?;
    let qual = format!("{parent_name}.fx");
    m.setattr("__package__", &qual)?;
    let sys = PyModule::import(py, "sys")?;
    sys.getattr("modules")?.set_item(&qual, &m)?;

    Ok(())
}
