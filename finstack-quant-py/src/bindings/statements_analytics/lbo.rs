//! Dated leveraged buyout authoring and results.

use crate::bindings::core::money::PyMoney;
use crate::bindings::extract::{extract_market_opt, extract_model_ref};
use crate::bindings::module_utils::fields_from_py;
use crate::errors::statements_to_py;
use finstack_quant_statements_analytics::analysis as native;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyModule};

#[pyclass(
    name = "LboEarnings",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboEarnings")]
#[derive(Clone)]
struct PyLboEarnings {
    inner: native::LboEarnings,
}

#[pymethods]
impl PyLboEarnings {
    #[new]
    #[pyo3(signature = (**fields), text_signature = "(*, node, period, multiple, basis)")]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboEarnings.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        Ok(Self {
            inner: fields_from_py(py, fields, "LboEarnings")?,
        })
    }
}

crate::bindings::macros::wire_methods!(
    PyLboEarnings,
    native::LboEarnings,
    "LboEarnings",
    module = "finstack_quant.statements_analytics._lbo"
);

#[pyclass(
    name = "LboDebtFunding",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboDebtFunding")]
#[derive(Clone)]
struct PyLboDebtFunding {
    inner: native::LboDebtFunding,
}

#[pymethods]
impl PyLboDebtFunding {
    #[new]
    #[pyo3(signature = (**fields), text_signature = "(*, name, cash_proceeds, opening_face)")]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboDebtFunding.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        Ok(Self {
            inner: fields_from_py(py, fields, "LboDebtFunding")?,
        })
    }
}

crate::bindings::macros::wire_methods!(
    PyLboDebtFunding,
    native::LboDebtFunding,
    "LboDebtFunding",
    module = "finstack_quant.statements_analytics._lbo"
);

#[pyclass(
    name = "LboFunding",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboFunding")]
#[derive(Clone)]
struct PyLboFunding {
    inner: native::LboFunding,
}

#[pymethods]
impl PyLboFunding {
    #[new]
    #[pyo3(signature = (**fields), text_signature = "(*, date, acquisition, transaction_fees, closing_credits, debt, sponsor_commitment=None)")]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboFunding.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        Ok(Self {
            inner: fields_from_py(py, fields, "LboFunding")?,
        })
    }
}

crate::bindings::macros::wire_methods!(
    PyLboFunding,
    native::LboFunding,
    "LboFunding",
    module = "finstack_quant.statements_analytics._lbo"
);

#[pyclass(
    name = "DatedLboRequest",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboRequest")]
#[derive(Clone)]
struct PyDatedLboRequest {
    inner: native::DatedLboRequest,
}

#[pymethods]
impl PyDatedLboRequest {
    #[new]
    #[pyo3(signature = (**fields), text_signature = "(*, cut, exit_valuation_date, entry, funding, exit, exit_debt_node, exit_cash_node, exit_claims_node=None, checks=None)")]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboRequest.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        Ok(Self {
            inner: fields_from_py(py, fields, "DatedLboRequest")?,
        })
    }
}

crate::bindings::macros::wire_methods!(
    PyDatedLboRequest,
    native::DatedLboRequest,
    "DatedLboRequest",
    module = "finstack_quant.statements_analytics._lbo"
);

#[pyclass(
    name = "LboFundingResult",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboFundingResult")]
#[derive(Clone)]
struct PyLboFundingResult {
    inner: native::LboFundingResult,
}

#[pymethods]
impl PyLboFundingResult {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboFundingResult.cash_uses")]
    fn cash_uses(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.cash_uses)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboFundingResult.debt_cash_proceeds")]
    fn debt_cash_proceeds(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.debt_cash_proceeds)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboFundingResult.opening_debt_face")]
    fn opening_debt_face(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.opening_debt_face)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboFundingResult.required_sponsor_cash")]
    fn required_sponsor_cash(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.required_sponsor_cash)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboFundingResult.committed_cash_sources")]
    fn committed_cash_sources(&self) -> Option<PyMoney> {
        self.inner.committed_cash_sources.map(PyMoney::from_inner)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboFundingResult.funding_gap")]
    fn funding_gap(&self) -> Option<PyMoney> {
        self.inner.funding_gap.map(PyMoney::from_inner)
    }
}

#[pyclass(
    name = "LboExitBridge",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboExitBridge")]
#[derive(Clone)]
struct PyLboExitBridge {
    inner: native::LboExitBridge,
}

#[pymethods]
impl PyLboExitBridge {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboExitBridge.enterprise_value")]
    fn enterprise_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.enterprise_value)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboExitBridge.net_debt")]
    fn net_debt(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.net_debt)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboExitBridge.other_claims")]
    fn other_claims(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.other_claims)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboExitBridge.common_equity_residual")]
    fn common_equity_residual(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.common_equity_residual)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboExitBridge.sponsor_proceeds")]
    fn sponsor_proceeds(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.sponsor_proceeds)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboExitBridge.sized_moic")]
    fn sized_moic(&self) -> Option<f64> {
        self.inner.sized_moic
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "LboExitBridge.funded_moic")]
    fn funded_moic(&self) -> Option<f64> {
        self.inner.funded_moic
    }
}

#[pyclass(
    name = "DatedLboResult",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboResult")]
#[derive(Clone)]
struct PyDatedLboResult {
    inner: native::DatedLboResult,
}

#[pymethods]
impl PyDatedLboResult {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboResult.entry_date")]
    fn entry_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.entry_date)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboResult.exit_valuation_date")]
    fn exit_valuation_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.exit_valuation_date)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboResult.run")]
    fn run<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.run)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboResult.checks")]
    fn checks(&self) -> Option<crate::bindings::statements::checks::PyCheckReport> {
        self.inner
            .checks
            .clone()
            .map(crate::bindings::statements::checks::PyCheckReport::from_inner)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboResult.entry_earnings")]
    fn entry_earnings(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.entry_earnings)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboResult.entry_enterprise_value")]
    fn entry_enterprise_value(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.entry_enterprise_value)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboResult.funding")]
    fn funding(&self) -> PyLboFundingResult {
        PyLboFundingResult {
            inner: self.inner.funding.clone(),
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboResult.exit_earnings")]
    fn exit_earnings(&self) -> PyMoney {
        PyMoney::from_inner(self.inner.exit_earnings)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "DatedLboResult.exit")]
    fn exit(&self) -> PyLboExitBridge {
        PyLboExitBridge {
            inner: self.inner.exit.clone(),
        }
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._lbo", "evaluate_dated_lbo")]
#[pyfunction]
#[pyo3(signature = (model, request, market=None))]
fn evaluate_dated_lbo<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
    request: &PyDatedLboRequest,
    market: Option<&Bound<'py, PyAny>>,
) -> PyResult<PyDatedLboResult> {
    let model = extract_model_ref(model)?.into_owned();
    let request = request.inner.clone();
    let market = extract_market_opt(py, market)?;
    let inner = py
        .detach(move || native::evaluate_dated_lbo(&model, &request, market.as_ref()))
        .map_err(statements_to_py)?;
    Ok(PyDatedLboResult { inner })
}

/// Register dated LBO types and pricing.
pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLboEarnings>()?;
    m.add_class::<PyLboDebtFunding>()?;
    m.add_class::<PyLboFunding>()?;
    m.add_class::<PyDatedLboRequest>()?;
    m.add_class::<PyLboFundingResult>()?;
    m.add_class::<PyLboExitBridge>()?;
    m.add_class::<PyDatedLboResult>()?;
    m.add_function(wrap_pyfunction!(evaluate_dated_lbo, m)?)
}
