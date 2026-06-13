//! Python bindings for the factor-model sensitivity engines.
//!
//! Wraps `finstack_portfolio::sensitivity` to expose delta-based and
//! full-repricing factor sensitivities from Python, with DataFrame export.

use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

const DEFAULT_PNL_SCENARIO_POINTS: usize =
    finstack_portfolio::sensitivity::DEFAULT_PNL_SCENARIO_POINTS;

/// Serialize a Python object to JSON via `json.dumps`, then deserialize into `T`.
fn py_to_serde<'py, T: serde::de::DeserializeOwned>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    label: &str,
) -> PyResult<T> {
    let json_mod = py.import("json")?;
    let json_str: String = json_mod.call_method1("dumps", (obj,))?.extract()?;
    serde_json::from_str(&json_str).map_err(|e| serde_json_to_py(e, &format!("invalid {label}")))
}

// ---------------------------------------------------------------------------
// SensitivityMatrix
// ---------------------------------------------------------------------------

/// Positions-by-factors sensitivity matrix.
///
/// Each element ``(i, j)`` is the first-order sensitivity of position *i* to
/// factor *j*, denominated in the factor's bump units (e.g. PV change per 1 bp
/// for a rates factor).
///
/// Construct via :func:`compute_factor_sensitivities`.
#[pyclass(
    name = "SensitivityMatrix",
    module = "finstack.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PySensitivityMatrix {
    position_ids: Vec<String>,
    factor_ids: Vec<String>,
    data: Vec<f64>,
    n_factors: usize,
}

impl PySensitivityMatrix {
    fn from_inner(matrix: finstack_portfolio::sensitivity::SensitivityMatrix) -> Self {
        let position_ids = matrix.position_ids().to_vec();
        let factor_ids = matrix
            .factor_ids()
            .iter()
            .map(|id| id.to_string())
            .collect();
        let n_factors = matrix.n_factors();
        let data = matrix.as_slice().to_vec();
        Self {
            position_ids,
            factor_ids,
            data,
            n_factors,
        }
    }
}

#[pymethods]
impl PySensitivityMatrix {
    /// Ordered position identifiers (row axis).
    #[getter]
    fn position_ids(&self) -> Vec<String> {
        self.position_ids.clone()
    }

    /// Ordered factor identifiers (column axis).
    #[getter]
    fn factor_ids(&self) -> Vec<String> {
        self.factor_ids.clone()
    }

    /// Number of positions (rows).
    #[getter]
    fn n_positions(&self) -> usize {
        self.position_ids.len()
    }

    /// Number of factors (columns).
    #[getter]
    fn n_factors(&self) -> usize {
        self.n_factors
    }

    /// Read a single sensitivity element.
    ///
    /// Parameters
    /// ----------
    /// position_idx : int
    ///     Row index.
    /// factor_idx : int
    ///     Column index.
    ///
    /// Returns
    /// -------
    /// float
    fn delta(&self, position_idx: usize, factor_idx: usize) -> PyResult<f64> {
        if position_idx >= self.position_ids.len() || factor_idx >= self.n_factors {
            return Err(crate::errors::value_error("index out of bounds"));
        }
        Ok(self.data[position_idx * self.n_factors + factor_idx])
    }

    /// Sensitivity row for a single position across all factors.
    fn position_deltas(&self, position_idx: usize) -> PyResult<Vec<f64>> {
        if position_idx >= self.position_ids.len() {
            return Err(crate::errors::value_error("position index out of bounds"));
        }
        let start = position_idx * self.n_factors;
        Ok(self.data[start..start + self.n_factors].to_vec())
    }

    /// Sensitivity column for a single factor across all positions.
    fn factor_deltas(&self, factor_idx: usize) -> PyResult<Vec<f64>> {
        if factor_idx >= self.n_factors {
            return Err(crate::errors::value_error("factor index out of bounds"));
        }
        Ok((0..self.position_ids.len())
            .map(|pos| self.data[pos * self.n_factors + factor_idx])
            .collect())
    }

    /// Export as a pandas ``DataFrame`` with positions as rows and factors as columns.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        for (fi, factor_id) in self.factor_ids.iter().enumerate() {
            let column: Vec<f64> = (0..self.position_ids.len())
                .map(|pi| self.data[pi * self.n_factors + fi])
                .collect();
            data.set_item(factor_id, column)?;
        }
        let index = PyList::new(py, &self.position_ids)?;
        dict_to_dataframe(py, &data, Some(index.into_any()))
    }

    fn __repr__(&self) -> String {
        format!(
            "SensitivityMatrix(positions={}, factors={})",
            self.position_ids.len(),
            self.n_factors
        )
    }
}

// ---------------------------------------------------------------------------
// FactorPnlProfile
// ---------------------------------------------------------------------------

/// P&L profile for one factor across a scenario grid.
///
/// Each profile captures the hypothetical P&L for every position at each
/// scenario shift, enabling non-linear (gamma, convexity) analysis.
///
/// Construct via :func:`compute_pnl_profiles`.
#[pyclass(
    name = "FactorPnlProfile",
    module = "finstack.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyFactorPnlProfile {
    factor_id: String,
    shifts: Vec<f64>,
    position_pnls: Vec<Vec<f64>>,
}

impl PyFactorPnlProfile {
    fn from_inner(profile: &finstack_portfolio::sensitivity::FactorPnlProfile) -> Self {
        Self {
            factor_id: profile.factor_id.to_string(),
            shifts: profile.shifts.clone(),
            position_pnls: profile.position_pnls.clone(),
        }
    }
}

#[pymethods]
impl PyFactorPnlProfile {
    /// Factor identifier.
    #[getter]
    fn factor_id(&self) -> &str {
        &self.factor_id
    }

    /// Scenario shift coordinates (bump-size multiples).
    #[getter]
    fn shifts(&self) -> Vec<f64> {
        self.shifts.clone()
    }

    /// Per-shift P&L vectors indexed as ``[shift_idx][position_idx]``.
    #[getter]
    fn position_pnls(&self) -> Vec<Vec<f64>> {
        self.position_pnls.clone()
    }

    /// Export as a pandas ``DataFrame`` with shifts as rows and positions as columns.
    ///
    /// Parameters
    /// ----------
    /// position_ids : list[str]
    ///     Position identifiers to use as column names.  Must match the
    ///     number of positions in the profile.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``len(position_ids)`` does not match the profile width.
    fn to_dataframe<'py>(
        &self,
        py: Python<'py>,
        position_ids: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let expected_width = self.position_pnls.first().map_or(0, Vec::len);
        if position_ids.len() != expected_width {
            return Err(crate::errors::value_error(format!(
                "position_ids length ({}) does not match profile width ({})",
                position_ids.len(),
                expected_width,
            )));
        }
        let data = PyDict::new(py);
        for (pi, pid) in position_ids.iter().enumerate() {
            let column: Vec<f64> = self.position_pnls.iter().map(|row| row[pi]).collect();
            data.set_item(pid, column)?;
        }
        let index = PyList::new(py, &self.shifts)?;
        dict_to_dataframe(py, &data, Some(index.into_any()))
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorPnlProfile(factor={:?}, shifts={}, positions={})",
            self.factor_id,
            self.shifts.len(),
            self.position_pnls.first().map_or(0, std::vec::Vec::len),
        )
    }
}

// ---------------------------------------------------------------------------
// compute_factor_sensitivities
// ---------------------------------------------------------------------------

/// Compute first-order factor sensitivities using central finite differences.
///
/// Parameters
/// ----------
/// positions_json : str
///     JSON array of position objects, each with ``id`` (str),
///     ``instrument`` (tagged instrument JSON), and ``weight`` (float).
/// factors_json : str
///     JSON array of ``FactorDefinition`` objects.
/// market_json : str
///     JSON-serialized ``MarketContext``.
/// as_of : str
///     Valuation date in ISO 8601 format.
/// bump_config_json : str, optional
///     JSON-serialized ``BumpSizeConfig``.  Defaults to 1 bp / 1 % per
///     factor type.
///
/// Returns
/// -------
/// SensitivityMatrix
///     Positions × factors delta matrix.
#[pyfunction]
#[pyo3(signature = (positions_json, factors_json, market, as_of, bump_config_json=None))]
fn compute_factor_sensitivities(
    py: Python<'_>,
    positions_json: &str,
    factors_json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    bump_config_json: Option<&str>,
) -> PyResult<PySensitivityMatrix> {
    let market = extract_market(market)?;
    let date = super::parse_date(as_of)?;
    let positions_json = positions_json.to_owned();
    let factors_json = factors_json.to_owned();
    let bump_config_json = bump_config_json.map(str::to_owned);

    let matrix = py
        .detach(move || {
            finstack_portfolio::sensitivity::compute_factor_sensitivities_from_json(
                &positions_json,
                &factors_json,
                &market,
                date,
                bump_config_json.as_deref(),
            )
        })
        .map_err(core_to_py)?;

    Ok(PySensitivityMatrix::from_inner(matrix))
}

// ---------------------------------------------------------------------------
// compute_pnl_profiles
// ---------------------------------------------------------------------------

/// Compute scenario P&L profiles via full repricing across a factor grid.
///
/// Parameters
/// ----------
/// positions_json : str
///     JSON array of position objects (same schema as
///     :func:`compute_factor_sensitivities`).
/// factors_json : str
///     JSON array of ``FactorDefinition`` objects.
/// market_json : str
///     JSON-serialized ``MarketContext``.
/// as_of : str
///     Valuation date in ISO 8601 format.
/// bump_config_json : str, optional
///     JSON-serialized ``BumpSizeConfig``.
/// n_scenario_points : int, optional
///     Number of scenario grid points (default 5 → shifts ``[-2, -1, 0, 1, 2]``).
///
/// Returns
/// -------
/// list[FactorPnlProfile]
///     One profile per factor, each containing scenario P&L for every position.
#[pyfunction]
#[pyo3(signature = (positions_json, factors_json, market, as_of, bump_config_json=None, n_scenario_points=DEFAULT_PNL_SCENARIO_POINTS))]
fn compute_pnl_profiles(
    py: Python<'_>,
    positions_json: &str,
    factors_json: &str,
    market: &Bound<'_, PyAny>,
    as_of: &str,
    bump_config_json: Option<&str>,
    n_scenario_points: usize,
) -> PyResult<Vec<PyFactorPnlProfile>> {
    let market = extract_market(market)?;
    let date = super::parse_date(as_of)?;
    let positions_json = positions_json.to_owned();
    let factors_json = factors_json.to_owned();
    let bump_config_json = bump_config_json.map(str::to_owned);

    let profiles = py
        .detach(move || {
            finstack_portfolio::sensitivity::compute_pnl_profiles_from_json(
                &positions_json,
                &factors_json,
                &market,
                date,
                bump_config_json.as_deref(),
                n_scenario_points,
            )
        })
        .map_err(core_to_py)?;

    Ok(profiles
        .iter()
        .map(PyFactorPnlProfile::from_inner)
        .collect())
}

// ---------------------------------------------------------------------------
// RiskDecomposition
// ---------------------------------------------------------------------------

/// Portfolio-level decomposition of total risk across factors and positions.
///
/// Obtain via :func:`decompose_factor_risk`.  The decomposition expresses
/// forecasted portfolio risk (variance, volatility, VaR, or ES) as a sum of
/// factor-level contributions, each of which can be further drilled into
/// per-position contributions.
#[pyclass(
    name = "FactorRiskDecomposition",
    module = "finstack.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyFactorRiskDecomposition {
    total_risk: f64,
    measure: String,
    residual_risk: f64,
    factor_ids: Vec<String>,
    absolute_risks: Vec<f64>,
    relative_risks: Vec<f64>,
    marginal_risks: Vec<f64>,
    pfc_position_ids: Vec<String>,
    pfc_factor_ids: Vec<String>,
    pfc_risk_contributions: Vec<f64>,
}

impl PyFactorRiskDecomposition {
    fn from_inner(decomp: finstack_portfolio::factor_model::RiskDecomposition) -> Self {
        let measure = serde_json::to_string(&decomp.measure)
            .unwrap_or_else(|_| format!("{:?}", decomp.measure));
        let factor_ids: Vec<String> = decomp
            .factor_contributions
            .iter()
            .map(|c| c.factor_id.to_string())
            .collect();
        let absolute_risks: Vec<f64> = decomp
            .factor_contributions
            .iter()
            .map(|c| c.absolute_risk)
            .collect();
        let relative_risks: Vec<f64> = decomp
            .factor_contributions
            .iter()
            .map(|c| c.relative_risk)
            .collect();
        let marginal_risks: Vec<f64> = decomp
            .factor_contributions
            .iter()
            .map(|c| c.marginal_risk)
            .collect();
        let pfc_position_ids: Vec<String> = decomp
            .position_factor_contributions
            .iter()
            .map(|c| c.position_id.to_string())
            .collect();
        let pfc_factor_ids: Vec<String> = decomp
            .position_factor_contributions
            .iter()
            .map(|c| c.factor_id.to_string())
            .collect();
        let pfc_risk_contributions: Vec<f64> = decomp
            .position_factor_contributions
            .iter()
            .map(|c| c.risk_contribution)
            .collect();
        Self {
            total_risk: decomp.total_risk,
            measure,
            residual_risk: decomp.residual_risk,
            factor_ids,
            absolute_risks,
            relative_risks,
            marginal_risks,
            pfc_position_ids,
            pfc_factor_ids,
            pfc_risk_contributions,
        }
    }
}

#[pymethods]
impl PyFactorRiskDecomposition {
    /// Total portfolio risk under the selected measure.
    #[getter]
    fn total_risk(&self) -> f64 {
        self.total_risk
    }

    /// Risk measure used (e.g. ``"Variance"``, ``"Volatility"``).
    #[getter]
    fn measure(&self) -> &str {
        &self.measure
    }

    /// Residual (idiosyncratic) risk not attributed to any factor.
    #[getter]
    fn residual_risk(&self) -> f64 {
        self.residual_risk
    }

    /// Factor-level contributions as a list of dicts.
    ///
    /// Each dict contains ``factor_id``, ``absolute_risk``, ``relative_risk``,
    /// and ``marginal_risk``.
    fn factor_contributions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let items: Vec<Bound<'py, PyDict>> = self
            .factor_ids
            .iter()
            .enumerate()
            .map(|(i, fid)| {
                let d = PyDict::new(py);
                d.set_item("factor_id", fid)?;
                d.set_item("absolute_risk", self.absolute_risks[i])?;
                d.set_item("relative_risk", self.relative_risks[i])?;
                d.set_item("marginal_risk", self.marginal_risks[i])?;
                Ok(d)
            })
            .collect::<PyResult<Vec<_>>>()?;
        PyList::new(py, items)
    }

    /// Position × factor contributions as a list of dicts.
    ///
    /// Each dict contains ``position_id``, ``factor_id``, and
    /// ``risk_contribution``.
    fn position_factor_contributions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let items: Vec<Bound<'py, PyDict>> = (0..self.pfc_position_ids.len())
            .map(|i| {
                let d = PyDict::new(py);
                d.set_item("position_id", &self.pfc_position_ids[i])?;
                d.set_item("factor_id", &self.pfc_factor_ids[i])?;
                d.set_item("risk_contribution", self.pfc_risk_contributions[i])?;
                Ok(d)
            })
            .collect::<PyResult<Vec<_>>>()?;
        PyList::new(py, items)
    }

    /// Export factor contributions as a pandas ``DataFrame``.
    ///
    /// Columns: ``factor_id``, ``absolute_risk``, ``relative_risk``,
    /// ``marginal_risk``.
    fn to_factor_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("factor_id", &self.factor_ids)?;
        data.set_item("absolute_risk", &self.absolute_risks)?;
        data.set_item("relative_risk", &self.relative_risks)?;
        data.set_item("marginal_risk", &self.marginal_risks)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Export position × factor contributions as a pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``factor_id``, ``risk_contribution``.
    fn to_position_factor_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("position_id", &self.pfc_position_ids)?;
        data.set_item("factor_id", &self.pfc_factor_ids)?;
        data.set_item("risk_contribution", &self.pfc_risk_contributions)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorRiskDecomposition(measure={:?}, total_risk={:.6}, factors={}, positions={})",
            self.measure,
            self.total_risk,
            self.factor_ids.len(),
            {
                let mut unique = self.pfc_position_ids.clone();
                unique.sort();
                unique.dedup();
                unique.len()
            },
        )
    }
}

// ---------------------------------------------------------------------------
// decompose_factor_risk
// ---------------------------------------------------------------------------

/// Decompose portfolio risk into factor and position contributions.
///
/// Uses the parametric (covariance-based) Euler decomposition to attribute
/// forecasted portfolio risk across factors and individual positions.
///
/// Parameters
/// ----------
/// sensitivities : SensitivityMatrix
///     Weighted position × factor sensitivity matrix, as returned by
///     :func:`compute_factor_sensitivities`.
/// covariance_json : str
///     JSON-serialized ``FactorCovarianceMatrix``.  Must use the same factor
///     IDs and ordering as the sensitivity matrix.
/// risk_measure : str | dict, optional
///     Risk measure.  Defaults to ``"variance"``.
///     Accepts Python strings (``"variance"``, ``"volatility"``) or dicts
///     (``{"var": {"confidence": 0.99}}``,
///     ``{"expected_shortfall": {"confidence": 0.975}}``).
///
/// Returns
/// -------
/// RiskDecomposition
///     Portfolio-level risk decomposition with factor and position detail.
#[pyfunction]
#[pyo3(signature = (sensitivities, covariance_json, risk_measure=None))]
fn decompose_factor_risk(
    py: Python<'_>,
    sensitivities: &PySensitivityMatrix,
    covariance_json: &str,
    risk_measure: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyFactorRiskDecomposition> {
    let factor_ids: Vec<finstack_factor_model::FactorId> = sensitivities
        .factor_ids
        .iter()
        .map(finstack_factor_model::FactorId::new)
        .collect();

    let mut matrix = finstack_portfolio::sensitivity::SensitivityMatrix::zeros(
        sensitivities.position_ids.clone(),
        factor_ids,
    );
    if sensitivities.n_factors > 0 {
        for (i, chunk) in sensitivities
            .data
            .chunks_exact(sensitivities.n_factors)
            .enumerate()
        {
            for (j, &val) in chunk.iter().enumerate() {
                matrix.set_delta(i, j, val);
            }
        }
    }

    let covariance: finstack_factor_model::FactorCovarianceMatrix =
        serde_json::from_str(covariance_json).map_err(display_to_py)?;

    let measure: finstack_factor_model::RiskMeasure = match risk_measure {
        Some(obj) => py_to_serde(py, obj, "risk_measure")?,
        None => finstack_factor_model::RiskMeasure::Variance,
    };

    let decomposer = finstack_portfolio::factor_model::ParametricDecomposer;
    let result = py
        .detach(move || {
            finstack_portfolio::factor_model::RiskDecomposer::decompose(
                &decomposer,
                &matrix,
                &covariance,
                &measure,
            )
        })
        .map_err(core_to_py)?;

    Ok(PyFactorRiskDecomposition::from_inner(result))
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// Register factor-model functions on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySensitivityMatrix>()?;
    m.add_class::<PyFactorPnlProfile>()?;
    m.add_class::<PyFactorRiskDecomposition>()?;
    m.add_function(pyo3::wrap_pyfunction!(compute_factor_sensitivities, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_pnl_profiles, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(decompose_factor_risk, m)?)?;
    Ok(())
}
