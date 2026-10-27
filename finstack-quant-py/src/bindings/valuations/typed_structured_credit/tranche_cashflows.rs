//! Typed projected cashflows of one tranche (`TrancheCashflows`).

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::money_to_py;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheCashflows;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TrancheCashflows",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTrancheCashflows {
    /// Inner canonical Rust cashflows.
    pub(crate) inner: TrancheCashflows,
}

crate::bindings::macros::wire_methods!(
    PyTrancheCashflows,
    TrancheCashflows,
    "TrancheCashflows",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows",
    to_dict
);

/// Convert dated flows to ``(datetime.date, Money)`` pairs.
fn flows_to_py<'py>(
    py: Python<'py>,
    flows: &[(Date, Money)],
) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
    flows
        .iter()
        .map(|(date, amount)| Ok((date_to_py(py, *date)?, money_to_py(*amount))))
        .collect()
}

#[pymethods]
impl PyTrancheCashflows {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.cashflows")]
    #[getter]
    fn cashflows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.cashflows)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.interest_flows")]
    #[getter]
    fn interest_flows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.interest_flows)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.principal_flows")]
    #[getter]
    fn principal_flows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.principal_flows)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.pik_flows")]
    #[getter]
    fn pik_flows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.pik_flows)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.deferred_flows")]
    #[getter]
    fn deferred_flows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.deferred_flows)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.writedown_flows")]
    #[getter]
    fn writedown_flows<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        flows_to_py(py, &self.inner.writedown_flows)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.accrual_periods")]
    #[getter]
    fn accrual_periods<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.accrual_periods)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.detailed_flows")]
    #[getter]
    fn detailed_flows<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.detailed_flows)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::core::table::envelope_to_pandas(
            py,
            &self.inner.to_table().map_err(crate::errors::core_to_py)?,
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "TrancheCashflows(tranche_id={:?}, payments={}, total_interest={}, total_principal={})",
            self.inner.tranche_id,
            self.inner.cashflows.len(),
            self.inner.total_interest.amount(),
            self.inner.total_principal.amount()
        )
    }
}

crate::bindings::macros::py_getters!(PyTrancheCashflows |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.tranche_id")]
    tranche_id -> String = s.inner.tranche_id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.final_balance")]
    final_balance -> PyMoney = money_to_py(s.inner.final_balance);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.total_interest")]
    total_interest -> PyMoney = money_to_py(s.inner.total_interest);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.total_principal")]
    total_principal -> PyMoney = money_to_py(s.inner.total_principal);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.total_pik")]
    total_pik -> PyMoney = money_to_py(s.inner.total_pik);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.total_deferred")]
    total_deferred -> PyMoney = money_to_py(s.inner.total_deferred);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.tranche_cashflows", "TrancheCashflows.total_writedown")]
    total_writedown -> PyMoney = money_to_py(s.inner.total_writedown);
});
