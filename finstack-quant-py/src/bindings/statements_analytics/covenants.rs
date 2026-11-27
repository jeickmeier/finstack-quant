//! Covenant evaluation and breach forecasting over evaluated statement results.
//!
//! Wraps `finstack_quant_statements_analytics::analysis::{evaluate_covenants,
//! forecast_breaches}`: the statement results come in as a typed object or
//! JSON, the engine and forecast configuration as the
//! `finstack_quant.covenants` classes.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::covenants::{
    PyCovenantEngine, PyCovenantForecastConfig, PyFutureBreach, dated_reports_table,
};
use crate::bindings::macros::tabular_methods;
use crate::bindings::pydoc::pydoc;
use crate::bindings::statements::evaluator::PyStatementResult;
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::core_to_py;
use finstack_quant_covenants::CovenantForecastConfig;
use pyo3::prelude::*;
use std::collections::BTreeMap;

#[doc = pydoc!("finstack_quant.statements_analytics._covenants", "evaluate_covenants")]
#[pyfunction]
#[pyo3(signature = (covenants, results, node_mapping=None))]
fn evaluate_covenants(
    covenants: PyRef<'_, PyCovenantEngine>,
    results: PyRef<'_, PyStatementResult>,
    node_mapping: Option<BTreeMap<String, String>>,
) -> PyResult<PyArrowTable> {
    let results = &results.inner;
    let series = finstack_quant_statements_analytics::analysis::evaluate_covenants(
        &covenants.inner,
        results,
        node_mapping.as_ref(),
    )
    .map_err(core_to_py)?;
    dated_reports_table(series)
}

#[doc = pydoc!("finstack_quant.statements_analytics._covenants", "forecast_breaches")]
#[pyfunction]
#[pyo3(
    signature = (covenants, results, config=None, node_mapping=None)
)]
fn forecast_breaches(
    covenants: PyRef<'_, PyCovenantEngine>,
    results: PyRef<'_, PyStatementResult>,
    config: Option<PyRef<'_, PyCovenantForecastConfig>>,
    node_mapping: Option<BTreeMap<String, String>>,
) -> PyResult<Vec<PyFutureBreach>> {
    let results = &results.inner;
    let config = config.map_or_else(CovenantForecastConfig::default, |c| c.inner.clone());
    finstack_quant_statements_analytics::analysis::forecast_breaches(
        &covenants.inner,
        results,
        config,
        node_mapping.as_ref(),
    )
    .map(|breaches| {
        breaches
            .into_iter()
            .map(PyFutureBreach::from_inner)
            .collect()
    })
    .map_err(core_to_py)
}

#[pyclass(
    name = "StatementCovenantSpec",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = pydoc!("finstack_quant.statements_analytics._covenants", "StatementCovenantSpec")]
#[derive(Clone)]
struct PyStatementCovenantSpec {
    inner: finstack_quant_statements_analytics::analysis::StatementCovenantSpec,
}

#[pymethods]
impl PyStatementCovenantSpec {
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, scope, covenant_ids, periods, observed_through, bindings)"
    )]
    #[doc = pydoc!("finstack_quant.statements_analytics._covenants", "StatementCovenantSpec.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, pyo3::types::PyDict>>) -> PyResult<Self> {
        let inner =
            crate::bindings::module_utils::fields_from_py(py, fields, "StatementCovenantSpec")?;
        Ok(Self { inner })
    }
}

#[pyclass(
    name = "StatementCovenantAssessment",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = pydoc!("finstack_quant.statements_analytics._covenants", "StatementCovenantAssessment")]
#[derive(Clone)]
struct PyStatementCovenantAssessment {
    inner: finstack_quant_statements_analytics::analysis::StatementCovenantAssessment,
}

#[pymethods]
impl PyStatementCovenantAssessment {
    #[getter]
    #[doc = pydoc!("finstack_quant.statements_analytics._covenants", "StatementCovenantAssessment.provenance")]
    fn provenance<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.provenance)
    }
}

impl PyStatementCovenantAssessment {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        const COLUMNS: [(&str, &str); 9] = [
            ("period", "str"),
            ("date", "str"),
            ("is_actual", "bool"),
            ("covenant_id", "str"),
            ("state", "str"),
            ("passed", "boolean"),
            ("threshold", "float64"),
            ("headroom_raw", "float64"),
            ("missing_metric", "str"),
        ];
        let rows = self
            .inner
            .periods
            .iter()
            .flat_map(|period| {
                period.assessments.iter().map(|test| {
                    serde_json::json!({
                        "period": period.period.to_string(),
                        "date": period.date.to_string(),
                        "is_actual": period.is_actual,
                        "covenant_id": test.covenant_id,
                        "state": test.state,
                        "passed": test.report.as_ref().map(|report| report.passed),
                        "threshold": test.effective_threshold,
                        "headroom_raw": test.report.as_ref().and_then(|report| report.headroom_raw),
                        "missing_metric": test.missing_metric,
                    })
                })
            })
            .collect::<Vec<_>>();
        serde_rows_to_arrow_table(&rows, &COLUMNS)
    }
}

tabular_methods!(
    PyStatementCovenantAssessment,
    "StatementCovenantAssessment",
    module = "finstack_quant.statements_analytics._covenants"
);

crate::bindings::macros::wire_methods!(
    PyStatementCovenantAssessment,
    finstack_quant_statements_analytics::analysis::StatementCovenantAssessment,
    "StatementCovenantAssessment",
    module = "finstack_quant.statements_analytics._covenants"
);

#[pyclass(
    name = "StatementCovenantForecast",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = pydoc!("finstack_quant.statements_analytics._covenants", "StatementCovenantForecast")]
#[derive(Clone)]
struct PyStatementCovenantForecast {
    inner: finstack_quant_statements_analytics::analysis::StatementCovenantForecast,
}

#[pymethods]
impl PyStatementCovenantForecast {
    #[getter]
    #[doc = pydoc!("finstack_quant.statements_analytics._covenants", "StatementCovenantForecast.assessment")]
    fn assessment(&self) -> PyStatementCovenantAssessment {
        PyStatementCovenantAssessment {
            inner: self.inner.assessment.clone(),
        }
    }

    #[getter]
    #[doc = pydoc!("finstack_quant.statements_analytics._covenants", "StatementCovenantForecast.breaches")]
    fn breaches(&self) -> Vec<PyFutureBreach> {
        self.inner
            .breaches
            .iter()
            .cloned()
            .map(PyFutureBreach::from_inner)
            .collect()
    }

    #[getter]
    #[doc = pydoc!("finstack_quant.statements_analytics._covenants", "StatementCovenantForecast.reference_date")]
    fn reference_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.reference_date)
    }
}

#[doc = pydoc!("finstack_quant.statements_analytics._covenants", "assess_statement_covenants")]
#[pyfunction]
fn assess_statement_covenants(
    engine: &PyCovenantEngine,
    model: PyRef<'_, PyFinancialModelSpec>,
    results: PyRef<'_, PyStatementResult>,
    spec: &PyStatementCovenantSpec,
) -> PyResult<PyStatementCovenantAssessment> {
    let model = &model.inner;
    let results = &results.inner;
    let inner = finstack_quant_statements_analytics::analysis::assess_statement_covenants(
        &engine.inner,
        model,
        results,
        &spec.inner,
    )
    .map_err(core_to_py)?;
    Ok(PyStatementCovenantAssessment { inner })
}

#[doc = pydoc!("finstack_quant.statements_analytics._covenants", "forecast_statement_covenants")]
#[pyfunction]
fn forecast_statement_covenants(
    engine: &PyCovenantEngine,
    model: PyRef<'_, PyFinancialModelSpec>,
    results: PyRef<'_, PyStatementResult>,
    spec: &PyStatementCovenantSpec,
    config: &PyCovenantForecastConfig,
) -> PyResult<PyStatementCovenantForecast> {
    let model = &model.inner;
    let results = &results.inner;
    let inner = finstack_quant_statements_analytics::analysis::forecast_statement_covenants(
        &engine.inner,
        model,
        results,
        &spec.inner,
        config.inner.clone(),
    )
    .map_err(core_to_py)?;
    Ok(PyStatementCovenantForecast { inner })
}

/// Register the statement covenant functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStatementCovenantSpec>()?;
    m.add_class::<PyStatementCovenantAssessment>()?;
    m.add_class::<PyStatementCovenantForecast>()?;
    m.add_function(pyo3::wrap_pyfunction!(assess_statement_covenants, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(forecast_statement_covenants, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(evaluate_covenants, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(forecast_breaches, m)?)?;
    Ok(())
}
