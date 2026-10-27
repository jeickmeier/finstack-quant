//! Deal-level accounting of a deterministic structured-credit simulation.

use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, serde_to_py};
use crate::bindings::valuations::convert::money_to_py;
use crate::errors::{display_to_py, serde_json_to_py};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::SimulationDiagnostics;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "SimulationDiagnostics",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PySimulationDiagnostics {
    /// Inner canonical Rust diagnostics.
    pub(crate) inner: SimulationDiagnostics,
}

#[pymethods]
impl PySimulationDiagnostics {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| serde_json_to_py(err, "invalid SimulationDiagnostics JSON"))?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.reserve_balance_path")]
    #[getter]
    fn reserve_balance_path<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .reserve_balance_path
            .iter()
            .map(|(date, amount)| Ok((date_to_py(py, *date)?, money_to_py(*amount))))
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.reserve_interest_paid")]
    #[getter]
    fn reserve_interest_paid<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .reserve_interest_paid
            .iter()
            .map(|(date, amount)| Ok((date_to_py(py, *date)?, money_to_py(*amount))))
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.early_amortization_date")]
    #[getter]
    fn early_amortization_date<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .early_amortization_date
            .map(|date| date_to_py(py, date))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.tranche_draws")]
    #[getter]
    fn tranche_draws<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(String, Bound<'py, PyAny>, PyMoney)>> {
        self.inner
            .tranche_draws
            .iter()
            .map(|(id, date, amount)| {
                Ok((id.clone(), date_to_py(py, *date)?, money_to_py(*amount)))
            })
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.periods")]
    #[getter]
    fn periods<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.periods)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::table_to_dataframe(
            py,
            &self.inner.to_table().map_err(crate::errors::core_to_py)?,
        )
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.coverage_tests_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn coverage_tests_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .periods
            .iter()
            .flat_map(|period| {
                period.coverage_tests.iter().map(|test| {
                    serde_json::json!({
                        "date": period.payment_date.to_string(),
                        "test_id": test.test_id,
                        "ratio": test.ratio,
                        "trigger_level": test.trigger_level,
                        "cushion": test.cushion,
                        "passing": test.passing,
                    })
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("date", "str"),
                ("test_id", "str"),
                ("ratio", "float64"),
                ("trigger_level", "float64"),
                ("cushion", "float64"),
                ("passing", "bool"),
            ],
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "SimulationDiagnostics(periods={}, draws_from_reserve={}, unfunded_draws={})",
            self.inner.periods.len(),
            self.inner.draws_from_reserve.amount(),
            self.inner.unfunded_draws.amount()
        )
    }
}

crate::bindings::macros::py_getters!(PySimulationDiagnostics |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.draws_from_reserve")]
    draws_from_reserve -> PyMoney = money_to_py(s.inner.draws_from_reserve);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.draws_from_principal")]
    draws_from_principal -> PyMoney = money_to_py(s.inner.draws_from_principal);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.unfunded_draws")]
    unfunded_draws -> PyMoney = money_to_py(s.inner.unfunded_draws);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.diagnostics", "SimulationDiagnostics.reserve_replenished")]
    reserve_replenished -> PyMoney = money_to_py(s.inner.reserve_replenished);
});
