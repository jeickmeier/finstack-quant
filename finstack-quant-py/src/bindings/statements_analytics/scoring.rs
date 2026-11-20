//! Typed admission of statement observations to native credit scoring models.

use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::extract::extract_results_ref;
use crate::bindings::models::credit::PyScoringResult;
use crate::bindings::module_utils::fields_from_py;
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, statements_to_py};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements_analytics::analysis::{
    self, CreditScoringModel, StatementScoringMapping,
};
use pyo3::prelude::*;
use pyo3::types::PyDict;

#[pyclass(
    name = "StatementScoringMapping",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = pydoc!("finstack_quant.statements_analytics._scoring", "StatementScoringMapping")]
#[derive(Clone)]
pub(super) struct PyStatementScoringMapping {
    inner: StatementScoringMapping,
}

#[pymethods]
impl PyStatementScoringMapping {
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, total_assets, total_liabilities, working_capital=None, retained_earnings=None, ebit=None, sales=None, book_equity=None, current_assets=None, current_liabilities=None, net_income=None)"
    )]
    #[doc = pydoc!("finstack_quant.statements_analytics._scoring", "StatementScoringMapping.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        Ok(Self {
            inner: fields_from_py(py, fields, "StatementScoringMapping")?,
        })
    }
}

#[pyclass(
    name = "CreditScoringModel",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = pydoc!("finstack_quant.statements_analytics._scoring", "CreditScoringModel")]
#[derive(Clone)]
pub(super) struct PyCreditScoringModel {
    inner: CreditScoringModel,
}

#[pymethods]
impl PyCreditScoringModel {
    #[staticmethod]
    #[doc = pydoc!("finstack_quant.statements_analytics._scoring", "CreditScoringModel.altman_z")]
    fn altman_z(market_equity: &PyMoney, as_of: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: CreditScoringModel::AltmanZ {
                market_equity: market_equity.inner,
                as_of: py_to_date(as_of)?,
            },
        })
    }

    #[staticmethod]
    #[doc = pydoc!("finstack_quant.statements_analytics._scoring", "CreditScoringModel.altman_z_prime")]
    fn altman_z_prime() -> Self {
        Self {
            inner: CreditScoringModel::AltmanZPrime,
        }
    }

    #[staticmethod]
    #[doc = pydoc!("finstack_quant.statements_analytics._scoring", "CreditScoringModel.altman_z_double_prime")]
    fn altman_z_double_prime() -> Self {
        Self {
            inner: CreditScoringModel::AltmanZDoublePrime,
        }
    }

    #[staticmethod]
    #[doc = pydoc!("finstack_quant.statements_analytics._scoring", "CreditScoringModel.altman_em")]
    fn altman_em() -> Self {
        Self {
            inner: CreditScoringModel::AltmanEm,
        }
    }

    #[staticmethod]
    #[doc = pydoc!("finstack_quant.statements_analytics._scoring", "CreditScoringModel.zmijewski")]
    fn zmijewski() -> Self {
        Self {
            inner: CreditScoringModel::Zmijewski,
        }
    }
}

#[pyfunction]
#[doc = pydoc!("finstack_quant.statements_analytics._scoring", "score_statement")]
fn score_statement(
    results: &Bound<'_, PyAny>,
    period: &str,
    mapping: &PyStatementScoringMapping,
    model: &PyCreditScoringModel,
) -> PyResult<PyScoringResult> {
    let period: PeriodId = period.parse().map_err(core_to_py)?;
    let results = extract_results_ref(results)?;
    let score = analysis::score_statement(&results, &period, &mapping.inner, &model.inner)
        .map_err(statements_to_py)?;
    Ok(PyScoringResult::from_inner(score))
}

#[pyfunction]
#[doc = pydoc!("finstack_quant.statements_analytics._scoring", "score_statement_series")]
fn score_statement_series(
    results: &Bound<'_, PyAny>,
    rows: Vec<(String, PyCreditScoringModel)>,
    mapping: &PyStatementScoringMapping,
) -> PyResult<Vec<PyScoringResult>> {
    let results = extract_results_ref(results)?;
    let requested = rows
        .into_iter()
        .map(|(period, model)| Ok((period.parse().map_err(core_to_py)?, model.inner)))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(
        analysis::score_statement_series(&results, &requested, &mapping.inner)
            .map_err(statements_to_py)?
            .into_iter()
            .map(PyScoringResult::from_inner)
            .collect(),
    )
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyStatementScoringMapping>()?;
    module.add_class::<PyCreditScoringModel>()?;
    module.add_function(wrap_pyfunction!(score_statement, module)?)?;
    module.add_function(wrap_pyfunction!(score_statement_series, module)?)?;
    Ok(())
}
