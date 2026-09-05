//! Python bindings for the factor-model sensitivity engines.
//!
//! Wraps `finstack_quant_portfolio::sensitivity` to expose delta-based and
//! full-repricing factor sensitivities from Python, with DataFrame export.

use crate::bindings::extract::extract_market;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::{core_to_py, display_to_py};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

const DEFAULT_PNL_SCENARIO_POINTS: usize =
    finstack_quant_portfolio::sensitivity::DEFAULT_PNL_SCENARIO_POINTS;

/// Positions-by-factors sensitivity matrix.
///
/// Each element ``(i, j)`` is the first-order sensitivity of position *i* to
/// factor *j*, denominated in the factor's bump units (e.g. PV change per 1 bp
/// for a rates factor).
///
/// Construct via :func:`compute_factor_sensitivities`.
#[pyclass(
    name = "SensitivityMatrix",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct PySensitivityMatrix {
    base_currency: finstack_quant_core::currency::Currency,
    #[serde(flatten)]
    pub(crate) inner: finstack_quant_portfolio::sensitivity::SensitivityMatrix,
}

impl PySensitivityMatrix {
    fn from_inner(
        inner: finstack_quant_portfolio::sensitivity::SensitivityMatrix,
        base_currency: finstack_quant_core::currency::Currency,
    ) -> Self {
        Self {
            inner,
            base_currency,
        }
    }
}

#[pymethods]
impl PySensitivityMatrix {
    /// Support `pickle` via the same serde round-trip as ``to_json``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from JSON (``{base_currency, position_ids, factor_ids, data}``).
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json).map_err(display_to_py)
    }

    /// Serialize to compact JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(self).map_err(display_to_py)
    }

    /// ISO reporting currency for every sensitivity entry.
    #[getter]
    fn base_currency(&self) -> String {
        self.base_currency.to_string()
    }

    /// Ordered position identifiers (row axis).
    #[getter]
    fn position_ids(&self) -> Vec<String> {
        self.inner.position_ids().to_vec()
    }

    /// Ordered factor identifiers (column axis).
    #[getter]
    fn factor_ids(&self) -> Vec<String> {
        self.inner
            .factor_ids()
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Number of positions (rows).
    #[getter]
    fn n_positions(&self) -> usize {
        self.inner.n_positions()
    }

    /// Number of factors (columns).
    #[getter]
    fn n_factors(&self) -> usize {
        self.inner.n_factors()
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
    #[pyo3(text_signature = "(self, position_idx, factor_idx)")]
    fn delta(&self, position_idx: usize, factor_idx: usize) -> PyResult<f64> {
        if position_idx >= self.inner.n_positions() || factor_idx >= self.inner.n_factors() {
            return Err(crate::errors::value_error("index out of bounds"));
        }
        Ok(self.inner.delta(position_idx, factor_idx))
    }

    /// Sensitivity row for a single position across all factors.
    #[pyo3(text_signature = "(self, position_idx)")]
    fn position_deltas(&self, position_idx: usize) -> PyResult<Vec<f64>> {
        if position_idx >= self.inner.n_positions() {
            return Err(crate::errors::value_error("position index out of bounds"));
        }
        Ok(self.inner.position_deltas(position_idx).to_vec())
    }

    /// Sensitivity column for a single factor across all positions.
    #[pyo3(text_signature = "(self, factor_idx)")]
    fn factor_deltas(&self, factor_idx: usize) -> PyResult<Vec<f64>> {
        if factor_idx >= self.inner.n_factors() {
            return Err(crate::errors::value_error("factor index out of bounds"));
        }
        Ok(self.inner.factor_deltas(factor_idx))
    }

    /// Export as a pandas ``DataFrame`` with positions as rows and factors as columns.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        for (fi, factor_id) in self.inner.factor_ids().iter().enumerate() {
            data.set_item(factor_id.to_string(), self.inner.factor_deltas(fi))?;
        }
        let index = PyList::new(py, self.inner.position_ids())?;
        dict_to_dataframe(py, &data, Some(index.into_any()))
    }

    fn __repr__(&self) -> String {
        format!(
            "SensitivityMatrix(positions={}, factors={})",
            self.inner.n_positions(),
            self.inner.n_factors()
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// P&L profile for one factor across a scenario grid.
///
/// Each profile captures the hypothetical P&L for every position at each
/// scenario shift, enabling non-linear (gamma, convexity) analysis.
///
/// Construct via :func:`compute_pnl_profiles`.
#[pyclass(
    name = "FactorPnlProfile",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyFactorPnlProfile {
    inner: finstack_quant_portfolio::sensitivity::FactorPnlProfile,
}

impl PyFactorPnlProfile {
    fn from_inner(inner: finstack_quant_portfolio::sensitivity::FactorPnlProfile) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorPnlProfile {
    /// Support `pickle` via the same serde round-trip as ``to_json``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from JSON (``{base_currency, factor_id, position_ids, shifts, position_pnls}``).
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to compact JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// ISO reporting currency for every P&L amount.
    #[getter]
    fn base_currency(&self) -> String {
        self.inner.base_currency.to_string()
    }

    /// Factor identifier.
    #[getter]
    fn factor_id(&self) -> String {
        self.inner.factor_id.to_string()
    }

    /// Ordered position identifiers indexing the inner ``position_pnls`` axis.
    #[getter]
    fn position_ids(&self) -> Vec<String> {
        self.inner.position_ids.clone()
    }

    /// Scenario shift coordinates (bump-size multiples).
    #[getter]
    fn shifts(&self) -> Vec<f64> {
        self.inner.shifts.clone()
    }

    /// Per-shift P&L vectors indexed as ``[shift_idx][position_idx]``.
    #[getter]
    fn position_pnls(&self) -> Vec<Vec<f64>> {
        self.inner.position_pnls.clone()
    }

    /// Export as a pandas ``DataFrame`` with shifts as rows and positions as
    /// columns (column labels are the profile's own ``position_ids``).
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        for (pi, pid) in self.inner.position_ids.iter().enumerate() {
            let column: Vec<f64> = self
                .inner
                .position_pnls
                .iter()
                .map(|row| row.get(pi).copied().unwrap_or(f64::NAN))
                .collect();
            data.set_item(pid, column)?;
        }
        let index = PyList::new(py, &self.inner.shifts)?;
        dict_to_dataframe(py, &data, Some(index.into_any()))
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorPnlProfile(factor={:?}, shifts={}, positions={})",
            self.inner.factor_id.as_str(),
            self.inner.shifts.len(),
            self.inner.position_ids.len(),
        )
    }
}

// compute_factor_sensitivities

/// Compute first-order factor sensitivities using central finite differences.
///
/// Parameters
/// ----------
/// positions_json : str | dict | list | pandas.DataFrame
///     JSON array of position objects, each with ``id`` (str),
///     ``instrument`` (canonical v1 instrument envelope), and ``weight`` (float).
/// factors_json : str | dict | list | pandas.DataFrame
///     JSON array of ``FactorDefinition`` objects.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON-serialized ``MarketContext``
///     string.
/// as_of : datetime.date | str
///     Valuation date, either a date-like object (``datetime.date``,
///     ``pandas.Timestamp``) or an ISO 8601 string.
/// base_currency : str
///     ISO reporting currency for every returned sensitivity or P&L amount; missing FX raises KeyError.
/// bump_config_json : str, optional
///     JSON-serialized ``BumpSizeConfig``.  Defaults to 1 bp / 1 % per
///     factor type.
///
/// Returns
/// -------
/// SensitivityMatrix
///     Positions × factors delta matrix.
#[pyfunction]
#[pyo3(signature = (positions_json, factors_json, market, as_of, base_currency, bump_config_json=None))]
fn compute_factor_sensitivities(
    py: Python<'_>,
    positions_json: &Bound<'_, PyAny>,
    factors_json: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    base_currency: &str,
    bump_config_json: Option<&str>,
) -> PyResult<PySensitivityMatrix> {
    let positions_json =
        crate::bindings::extract::extract_records_json(py, positions_json, "positions")?;
    let positions_json: &str = &positions_json;
    let factors_json = crate::bindings::extract::extract_records_json(py, factors_json, "factors")?;
    let factors_json: &str = &factors_json;
    let market = extract_market(py, market)?;
    let date = crate::bindings::date_utils::extract_date(as_of)?;
    let base_currency = base_currency
        .parse::<finstack_quant_core::currency::Currency>()
        .map_err(display_to_py)?;
    let positions_json = positions_json.to_owned();
    let factors_json = factors_json.to_owned();
    let bump_config_json = bump_config_json.map(str::to_owned);

    py.detach(move || {
        finstack_quant_portfolio::sensitivity::compute_factor_sensitivities_from_json(
            &positions_json,
            &factors_json,
            &market,
            date,
            base_currency,
            bump_config_json.as_deref(),
        )
        .map(|matrix| PySensitivityMatrix::from_inner(matrix, base_currency))
    })
    .map_err(core_to_py)
}

// compute_pnl_profiles

/// Compute scenario P&L profiles via full repricing across a factor grid.
///
/// Parameters
/// ----------
/// positions_json : str | dict | list | pandas.DataFrame
///     JSON array of position objects (same schema as
///     :func:`compute_factor_sensitivities`).
/// factors_json : str | dict | list | pandas.DataFrame
///     JSON array of ``FactorDefinition`` objects.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON-serialized ``MarketContext``
///     string.
/// as_of : datetime.date | str
///     Valuation date, either a date-like object (``datetime.date``,
///     ``pandas.Timestamp``) or an ISO 8601 string.
/// base_currency : str
///     ISO reporting currency for every returned sensitivity or P&L amount; missing FX raises KeyError.
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
#[pyo3(
    signature = (positions_json, factors_json, market, as_of, base_currency, bump_config_json=None, n_scenario_points=DEFAULT_PNL_SCENARIO_POINTS),
    text_signature = "(positions_json, factors_json, market, as_of, base_currency, bump_config_json=None, n_scenario_points=5)"
)]
fn compute_pnl_profiles(
    positions_json: &Bound<'_, PyAny>,
    factors_json: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    base_currency: &str,
    bump_config_json: Option<&str>,
    n_scenario_points: usize,
) -> PyResult<Vec<PyFactorPnlProfile>> {
    let py = market.py();
    let positions_json =
        crate::bindings::extract::extract_records_json(py, positions_json, "positions")?;
    let positions_json: &str = &positions_json;
    let factors_json = crate::bindings::extract::extract_records_json(py, factors_json, "factors")?;
    let factors_json: &str = &factors_json;
    let market = extract_market(py, market)?;
    let date = crate::bindings::date_utils::extract_date(as_of)?;
    let base_currency = base_currency
        .parse::<finstack_quant_core::currency::Currency>()
        .map_err(display_to_py)?;
    let positions_json = positions_json.to_owned();
    let factors_json = factors_json.to_owned();
    let bump_config_json = bump_config_json.map(str::to_owned);

    py.detach(move || {
        finstack_quant_portfolio::sensitivity::compute_pnl_profiles_from_json(
            &positions_json,
            &factors_json,
            &market,
            date,
            base_currency,
            bump_config_json.as_deref(),
            n_scenario_points,
        )
        .map(|profiles| {
            profiles
                .into_iter()
                .map(PyFactorPnlProfile::from_inner)
                .collect()
        })
    })
    .map_err(core_to_py)
}

/// Portfolio-level decomposition of total risk across factors and positions.
///
/// Obtain via :func:`decompose_factor_risk`.  The decomposition expresses
/// forecasted portfolio risk (variance, volatility, VaR, or ES) as a sum of
/// factor-level contributions, each of which can be further drilled into
/// per-position contributions.
#[pyclass(
    name = "FactorRiskDecomposition",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
struct PyFactorRiskDecomposition {
    inner: finstack_quant_models::factor::risk::RiskDecomposition,
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
    residual_contributions: Vec<finstack_quant_models::factor::risk::PositionResidualContribution>,
}

/// Bare snake_case serde tag of a [`RiskMeasure`], without JSON quoting or
/// variant payload (`"variance"`, `"volatility"`, `"var"`,
/// `"expected_shortfall"`). Matches the tag the WASM binding reports.
fn risk_measure_tag(measure: &finstack_quant_models::factor::RiskMeasure) -> String {
    use finstack_quant_models::factor::RiskMeasure as M;
    match measure {
        M::Variance => "variance".to_owned(),
        M::Volatility => "volatility".to_owned(),
        M::VaR { .. } => "var".to_owned(),
        M::ExpectedShortfall { .. } => "expected_shortfall".to_owned(),
        // `RiskMeasure` is `#[non_exhaustive]`; derive the tag of a future
        // variant from its serde form so the binding stays forward-compatible.
        other => match serde_json::to_value(other) {
            Ok(serde_json::Value::String(tag)) => tag,
            Ok(serde_json::Value::Object(map)) => map
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| format!("{other:?}")),
            _ => format!("{other:?}"),
        },
    }
}

impl PyFactorRiskDecomposition {
    fn from_inner(decomp: finstack_quant_models::factor::risk::RiskDecomposition) -> Self {
        let measure = risk_measure_tag(&decomp.measure);
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
            residual_contributions: decomp.position_residual_contributions.clone(),
            inner: decomp,
        }
    }
}

#[pymethods]
impl PyFactorRiskDecomposition {
    /// Support `pickle` via the same serde round-trip as ``to_json``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from canonical ``RiskDecomposition`` JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_models::factor::risk::RiskDecomposition =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to canonical ``RiskDecomposition`` JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Total portfolio risk under the selected measure.
    #[getter]
    fn total_risk(&self) -> f64 {
        self.total_risk
    }

    /// Risk-measure tag in canonical snake_case serde form: ``"variance"``,
    /// ``"volatility"``, ``"var"``, or ``"expected_shortfall"``. Matches the
    /// tag reported by the WASM ``decomposeFactorRisk`` output.
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

    /// Per-position residual (idiosyncratic) variance contributions as a
    /// list of dicts.
    ///
    /// Each dict contains ``position_id``, ``residual_variance`` (annualized
    /// variance units), and a ``source`` object tagged by ``kind``. Empty for
    /// the parametric decomposer used by :func:`decompose_factor_risk` —
    /// populated only by credit-aware position decomposers.
    fn position_residual_contributions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.residual_contributions)
    }

    /// Primary table: the factor-level risk decomposition.
    ///
    /// Alias of :meth:`to_factor_dataframe`. Every tabular result type in the
    /// library answers ``to_dataframe()``; the position × factor view stays on
    /// :meth:`to_position_factor_dataframe`.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_factor_dataframe(py)
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

// decompose_factor_risk

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
/// covariance_json : str | dict | list | pandas.DataFrame
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
/// FactorRiskDecomposition
///     Portfolio-level risk decomposition with factor and position detail.
#[pyfunction]
#[pyo3(signature = (sensitivities, covariance_json, risk_measure=None))]
fn decompose_factor_risk(
    py: Python<'_>,
    sensitivities: &PySensitivityMatrix,
    covariance_json: &Bound<'_, PyAny>,
    risk_measure: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyFactorRiskDecomposition> {
    let covariance_json =
        crate::bindings::extract::extract_records_json(py, covariance_json, "covariance")?;
    let covariance_json: &str = &covariance_json;
    let measure: finstack_quant_models::factor::RiskMeasure = match risk_measure {
        Some(obj) => py_to_serde(py, obj, "risk_measure")?,
        None => finstack_quant_models::factor::RiskMeasure::Variance,
    };

    let matrix = sensitivities.inner.clone();
    let covariance_json = covariance_json.to_owned();
    py.detach(move || {
        let covariance: finstack_quant_models::factor::FactorCovarianceMatrix =
            serde_json::from_str(&covariance_json).map_err(display_to_py)?;
        let decomposer = finstack_quant_models::factor::risk::ParametricDecomposer;
        let result = decomposer
            .decompose(&matrix, &covariance, &measure)
            .map_err(core_to_py)?;
        Ok(PyFactorRiskDecomposition::from_inner(result))
    })
}

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
