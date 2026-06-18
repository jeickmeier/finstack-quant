//! Direct Python wrappers for rates valuation instruments.

use super::direct_wrapper::{
    build_from_py, from_json_payload, pretty_json, price_payload, price_payload_with_metrics,
    validate_payload,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList, PyModule};

macro_rules! rates_class {
    ($py_name:literal, $rust_name:ident, $type_tag:literal) => {
        #[pyclass(
            name = $py_name,
            module = "finstack_quant.valuations.instruments.rates",
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
                        "rates instrument spec",
                        "rates instrument constructor requires a spec object, JSON string, or keyword fields",
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

rates_class!("InterestRateSwap", PyInterestRateSwap, "interest_rate_swap");
rates_class!("BasisSwap", PyBasisSwap, "basis_swap");
rates_class!("XccySwap", PyXccySwap, "xccy_swap");
rates_class!("InflationSwap", PyInflationSwap, "inflation_swap");
rates_class!("YoYInflationSwap", PyYoYInflationSwap, "yoy_inflation_swap");
rates_class!(
    "InflationCapFloor",
    PyInflationCapFloor,
    "inflation_cap_floor"
);
rates_class!(
    "ForwardRateAgreement",
    PyForwardRateAgreement,
    "forward_rate_agreement"
);
rates_class!("Swaption", PySwaption, "swaption");
rates_class!("BermudanSwaption", PyBermudanSwaption, "bermudan_swaption");
rates_class!(
    "InterestRateFuture",
    PyInterestRateFuture,
    "interest_rate_future"
);
rates_class!("CapFloor", PyCapFloor, "cap_floor");
rates_class!("CmsSwap", PyCmsSwap, "cms_swap");
rates_class!("CmsOption", PyCmsOption, "cms_option");
rates_class!("IrFutureOption", PyIrFutureOption, "ir_future_option");
rates_class!("Deposit", PyDeposit, "deposit");
rates_class!("Repo", PyRepo, "repo");
rates_class!("RangeAccrual", PyRangeAccrual, "range_accrual");
rates_class!("Tarn", PyTarn, "tarn");
rates_class!("Snowball", PySnowball, "snowball");
rates_class!("CmsSpreadOption", PyCmsSpreadOption, "cms_spread_option");
rates_class!(
    "CallableRangeAccrual",
    PyCallableRangeAccrual,
    "callable_range_accrual"
);

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "rates")?;
    m.setattr("__doc__", "Direct rates valuation instrument wrappers.")?;

    m.add_class::<PyInterestRateSwap>()?;
    m.add_class::<PyBasisSwap>()?;
    m.add_class::<PyXccySwap>()?;
    m.add_class::<PyInflationSwap>()?;
    m.add_class::<PyYoYInflationSwap>()?;
    m.add_class::<PyInflationCapFloor>()?;
    m.add_class::<PyForwardRateAgreement>()?;
    m.add_class::<PySwaption>()?;
    m.add_class::<PyBermudanSwaption>()?;
    m.add_class::<PyInterestRateFuture>()?;
    m.add_class::<PyCapFloor>()?;
    m.add_class::<PyCmsSwap>()?;
    m.add_class::<PyCmsOption>()?;
    m.add_class::<PyIrFutureOption>()?;
    m.add_class::<PyDeposit>()?;
    m.add_class::<PyRepo>()?;
    m.add_class::<PyRangeAccrual>()?;
    m.add_class::<PyTarn>()?;
    m.add_class::<PySnowball>()?;
    m.add_class::<PyCmsSpreadOption>()?;
    m.add_class::<PyCallableRangeAccrual>()?;

    let all = PyList::new(
        py,
        [
            "InterestRateSwap",
            "BasisSwap",
            "XccySwap",
            "InflationSwap",
            "YoYInflationSwap",
            "InflationCapFloor",
            "ForwardRateAgreement",
            "Swaption",
            "BermudanSwaption",
            "InterestRateFuture",
            "CapFloor",
            "CmsSwap",
            "CmsOption",
            "IrFutureOption",
            "Deposit",
            "Repo",
            "RangeAccrual",
            "Tarn",
            "Snowball",
            "CmsSpreadOption",
            "CallableRangeAccrual",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "rates",
        "finstack_quant.finstack_quant.valuations",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
