//! Direct Python wrappers for equity valuation instruments.

use super::direct_wrapper::{
    build_from_py, from_json_payload, pretty_json, price_payload, price_payload_with_metrics,
    validate_payload,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList, PyModule};

macro_rules! equity_class {
    ($py_name:literal, $rust_name:ident, $type_tag:literal) => {
        #[pyclass(
            name = $py_name,
            module = "finstack_quant.valuations.instruments.equity",
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
                        "equity instrument spec",
                        "equity instrument constructor requires a spec object, JSON string, or keyword fields",
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

equity_class!("Equity", PyEquity, "equity");
equity_class!("EquityOption", PyEquityOption, "equity_option");
equity_class!("VarianceSwap", PyVarianceSwap, "variance_swap");
equity_class!(
    "EquityIndexFuture",
    PyEquityIndexFuture,
    "equity_index_future"
);
equity_class!(
    "VolatilityIndexFuture",
    PyVolatilityIndexFuture,
    "volatility_index_future"
);
equity_class!(
    "VolatilityIndexOption",
    PyVolatilityIndexOption,
    "volatility_index_option"
);
equity_class!("Autocallable", PyAutocallable, "autocallable");
equity_class!("CliquetOption", PyCliquetOption, "cliquet_option");
equity_class!(
    "EquityTotalReturnSwap",
    PyEquityTotalReturnSwap,
    "trs_equity"
);
equity_class!(
    "PrivateMarketsFund",
    PyPrivateMarketsFund,
    "private_markets_fund"
);
equity_class!("RealEstateAsset", PyRealEstateAsset, "real_estate_asset");
equity_class!(
    "LeveredRealEstateEquity",
    PyLeveredRealEstateEquity,
    "levered_real_estate_equity"
);
equity_class!(
    "DiscountedCashFlow",
    PyDiscountedCashFlow,
    "discounted_cash_flow"
);

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "equity")?;
    m.setattr("__doc__", "Direct equity valuation instrument wrappers.")?;

    m.add_class::<PyEquity>()?;
    m.add_class::<PyEquityOption>()?;
    m.add_class::<PyVarianceSwap>()?;
    m.add_class::<PyEquityIndexFuture>()?;
    m.add_class::<PyVolatilityIndexFuture>()?;
    m.add_class::<PyVolatilityIndexOption>()?;
    m.add_class::<PyAutocallable>()?;
    m.add_class::<PyCliquetOption>()?;
    m.add_class::<PyEquityTotalReturnSwap>()?;
    m.add_class::<PyPrivateMarketsFund>()?;
    m.add_class::<PyRealEstateAsset>()?;
    m.add_class::<PyLeveredRealEstateEquity>()?;
    m.add_class::<PyDiscountedCashFlow>()?;

    let all = PyList::new(
        py,
        [
            "Equity",
            "EquityOption",
            "VarianceSwap",
            "EquityIndexFuture",
            "VolatilityIndexFuture",
            "VolatilityIndexOption",
            "Autocallable",
            "CliquetOption",
            "EquityTotalReturnSwap",
            "PrivateMarketsFund",
            "RealEstateAsset",
            "LeveredRealEstateEquity",
            "DiscountedCashFlow",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "equity",
        "finstack_quant.finstack_quant.valuations",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
