use pyo3::prelude::*;

use finstack_quant_factor::{RiskMeasure, risk as model_risk};

use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, value_error};

use super::super::config::PyFactorCovarianceMatrix;
use super::contributions::{PyPositionRiskDecomposition, PyRiskDecomposition};
use crate::bindings::matrix_input::{PositionPnlMatrix, extract_position_pnls};
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::portfolio::sensitivity::PySensitivityMatrix;

#[doc = pydoc!("finstack_quant.models.factor.risk._functions", "RiskMeasure")]
#[pyclass(
    name = "RiskMeasure",
    module = "finstack_quant.models.factor.risk",
    frozen,
    skip_from_py_object
)]
pub(in crate::bindings::models::factor) struct PyRiskMeasure {
    inner: RiskMeasure,
}

#[pymethods]
impl PyRiskMeasure {
    #[doc = pydoc!("finstack_quant.models.factor.risk._functions", "RiskMeasure.variance")]
    #[staticmethod]
    fn variance() -> Self {
        Self {
            inner: RiskMeasure::Variance,
        }
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._functions", "RiskMeasure.volatility")]
    #[staticmethod]
    fn volatility() -> Self {
        Self {
            inner: RiskMeasure::Volatility,
        }
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._functions", "RiskMeasure.var")]
    #[staticmethod]
    fn var(confidence: f64) -> PyResult<Self> {
        RiskMeasure::var(confidence)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._functions", "RiskMeasure.expected_shortfall")]
    #[staticmethod]
    fn expected_shortfall(confidence: f64) -> PyResult<Self> {
        RiskMeasure::expected_shortfall(confidence)
            .map(|inner| Self { inner })
            .map_err(core_to_py)
    }
}

pub(in crate::bindings::models::factor) fn extract_risk_measure(
    py: Python<'_>,
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<RiskMeasure> {
    let Some(value) = value else {
        return Ok(RiskMeasure::default());
    };
    match value.cast::<PyRiskMeasure>() {
        Ok(typed) => Ok(typed.borrow().inner),
        Err(_)
            if value
                .extract::<&str>()
                .is_ok_and(|text| !matches!(text, "variance" | "volatility")) =>
        {
            crate::bindings::module_utils::py_to_serde(py, value, "risk_measure")
        }
        Err(_) => py_to_serde(py, value, "risk_measure"),
    }
}

/// Resolve `position_ids` and the position × scenario P&L matrix.
///
/// Accepted shapes for `position_pnls`:
///
/// - `pandas.DataFrame`, `polars.DataFrame`, or Arrow table — rows are scenarios,
///   columns are positions (`position_ids` defaults to the column labels);
/// - nested list or 2-D NumPy array shaped `n_positions × n_scenarios`
///   (position-major), the Rust `flatten_position_pnls` layout WASM shares.
pub(super) fn extract_pnl_input(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    position_pnls: &Bound<'_, PyAny>,
) -> PyResult<(Vec<String>, PositionPnlMatrix)> {
    use crate::bindings::dataframe_utils::{extract_2d_f64, is_dataframe_like, table_column_names};
    if is_dataframe_like(position_pnls)? {
        let columns = table_column_names(position_pnls)?;
        let ids = position_ids.unwrap_or(columns);
        let (data, n_scenarios, ncols) = extract_2d_f64(position_pnls, "position_pnls")?
            .ok_or_else(|| value_error("position_pnls DataFrame must be convertible to float64"))?;
        if ncols != ids.len() {
            return Err(value_error(format!(
                "position_pnls DataFrame has {} columns but {} position ids; \
                 rows are scenarios and columns are positions",
                ncols,
                ids.len()
            )));
        }
        let matrix = PositionPnlMatrix::from_scenario_major(data, n_scenarios);
        return Ok((ids, matrix));
    }
    let ids = position_ids.ok_or_else(|| {
        value_error("position_ids is required unless position_pnls is a DataFrame")
    })?;
    let matrix = extract_position_pnls(py, position_pnls, ids.len())?;
    Ok((ids, matrix))
}

#[doc = pydoc!("finstack_quant.models.factor.risk._functions", "position_component_var")]
#[pyfunction]
#[pyo3(signature = (decomp, position_id))]
pub(super) fn position_component_var(
    decomp: &PyPositionRiskDecomposition,
    position_id: &str,
) -> PyResult<f64> {
    decomp.inner.component_var(position_id).map_err(core_to_py)
}

#[doc = pydoc!("finstack_quant.models.factor.risk._functions", "decompose_factors")]
#[pyfunction]
#[pyo3(signature = (sensitivities, covariance, risk_measure=None))]
pub(super) fn decompose_factors(
    py: Python<'_>,
    sensitivities: &PySensitivityMatrix,
    covariance: &Bound<'_, PyAny>,
    risk_measure: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyRiskDecomposition> {
    let covariance = match covariance.cast::<PyFactorCovarianceMatrix>() {
        Ok(matrix) => matrix.borrow().inner.clone(),
        Err(_) => crate::bindings::module_utils::py_to_serde(py, covariance, "covariance")?,
    };
    let measure = extract_risk_measure(py, risk_measure)?;

    let matrix = sensitivities.inner.clone();
    py.detach(move || {
        let result =
            model_risk::decompose_factors(&matrix, &covariance, &measure).map_err(core_to_py)?;
        Ok(PyRiskDecomposition::from_inner(result))
    })
}
