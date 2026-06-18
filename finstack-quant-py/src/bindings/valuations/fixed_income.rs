//! Direct Python wrappers for fixed-income valuation instruments.

use super::direct_wrapper::{
    build_from_py, from_json_payload, pretty_json, price_payload, price_payload_with_metrics,
    validate_payload,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList, PyModule};

macro_rules! fixed_income_class {
    ($py_name:literal, $rust_name:ident, $type_tag:literal) => {
        #[pyclass(
            name = $py_name,
            module = "finstack_quant.valuations.instruments.fixed_income",
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
                        "fixed-income instrument spec",
                        "fixed-income instrument constructor requires a spec object, JSON string, or keyword fields",
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

fixed_income_class!("Bond", PyBond, "bond");
fixed_income_class!("ConvertibleBond", PyConvertibleBond, "convertible_bond");
fixed_income_class!(
    "InflationLinkedBond",
    PyInflationLinkedBond,
    "inflation_linked_bond"
);
fixed_income_class!("TermLoan", PyTermLoan, "term_loan");
fixed_income_class!("RevolvingCredit", PyRevolvingCredit, "revolving_credit");
fixed_income_class!("BondFuture", PyBondFuture, "bond_future");
fixed_income_class!(
    "AgencyMbsPassthrough",
    PyAgencyMbsPassthrough,
    "agency_mbs_passthrough"
);
fixed_income_class!("AgencyTba", PyAgencyTba, "agency_tba");
fixed_income_class!("AgencyCmo", PyAgencyCmo, "agency_cmo");
fixed_income_class!("DollarRoll", PyDollarRoll, "dollar_roll");
fixed_income_class!(
    "FIIndexTotalReturnSwap",
    PyFIIndexTotalReturnSwap,
    "trs_fixed_income_index"
);
fixed_income_class!("StructuredCredit", PyStructuredCredit, "structured_credit");

pub(crate) fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "fixed_income")?;
    m.setattr(
        "__doc__",
        "Direct fixed-income valuation instrument wrappers.",
    )?;

    m.add_class::<PyBond>()?;
    m.add_class::<PyConvertibleBond>()?;
    m.add_class::<PyInflationLinkedBond>()?;
    m.add_class::<PyTermLoan>()?;
    m.add_class::<PyRevolvingCredit>()?;
    m.add_class::<PyBondFuture>()?;
    m.add_class::<PyAgencyMbsPassthrough>()?;
    m.add_class::<PyAgencyTba>()?;
    m.add_class::<PyAgencyCmo>()?;
    m.add_class::<PyDollarRoll>()?;
    m.add_class::<PyFIIndexTotalReturnSwap>()?;
    m.add_class::<PyStructuredCredit>()?;

    let all = PyList::new(
        py,
        [
            "Bond",
            "ConvertibleBond",
            "InflationLinkedBond",
            "TermLoan",
            "RevolvingCredit",
            "BondFuture",
            "AgencyMbsPassthrough",
            "AgencyTba",
            "AgencyCmo",
            "DollarRoll",
            "FIIndexTotalReturnSwap",
            "StructuredCredit",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "fixed_income",
        "finstack_quant.finstack_quant.valuations",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
