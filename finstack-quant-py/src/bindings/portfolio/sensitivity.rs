//! Python bindings for the factor-model sensitivity engines.
//!
//! Wraps `finstack_quant_portfolio::sensitivity` to expose delta-based and
//! full-repricing factor sensitivities from Python, with DataFrame export.

use crate::bindings::extract::{extract_market_ref, extract_portfolio_ref, extract_records_json};
use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::{core_to_py, display_to_py, serde_json_to_py};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

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
#[derive(Clone)]
pub(crate) struct PySensitivityMatrix {
    base_currency: finstack_quant_core::currency::Currency,
    pub(crate) inner: finstack_quant_factor::SensitivityMatrix,
}

impl PySensitivityMatrix {
    pub(crate) fn from_inner(
        inner: finstack_quant_factor::SensitivityMatrix,
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

    /// Parse from the canonical wire JSON
    /// (``{base_currency, position_ids, factor_ids, data}``, ``data`` as one
    /// row per position), the shape WASM ``computeFactorSensitivities``
    /// returns and ``models.factor.risk.decomposeFactors`` accepts.
    ///
    /// ``data[position][factor]`` must contain exactly one finite entry per
    /// declared position/factor pair. Malformed shapes and unknown fields raise
    /// ``ValueError``.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let wire: finstack_quant_portfolio::sensitivity::SensitivityMatrixJson =
            serde_json::from_str(json).map_err(display_to_py)?;
        let base_currency = wire.base_currency;
        let inner = finstack_quant_factor::SensitivityMatrix::try_from(wire).map_err(core_to_py)?;
        Ok(Self::from_inner(inner, base_currency))
    }

    /// Serialize to the canonical wire JSON (see :meth:`from_json`).
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(
            &finstack_quant_portfolio::sensitivity::SensitivityMatrixJson::from_matrix(
                &self.inner,
                self.base_currency,
            ),
        )
        .map_err(display_to_py)
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
        self.inner
            .delta(position_idx, factor_idx)
            .map_err(core_to_py)
    }

    /// Sensitivity row for a single position across all factors.
    #[pyo3(text_signature = "(self, position_idx)")]
    fn position_deltas(&self, position_idx: usize) -> PyResult<Vec<f64>> {
        self.inner
            .position_deltas(position_idx)
            .map(<[f64]>::to_vec)
            .map_err(core_to_py)
    }

    /// Sensitivity column for a single factor across all positions.
    #[pyo3(text_signature = "(self, factor_idx)")]
    fn factor_deltas(&self, factor_idx: usize) -> PyResult<Vec<f64>> {
        self.inner.factor_deltas(factor_idx).map_err(core_to_py)
    }

    /// Export as a pandas ``DataFrame`` with positions as rows and factors as columns.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        for (fi, factor_id) in self.inner.factor_ids().iter().enumerate() {
            data.set_item(
                factor_id.to_string(),
                self.inner.factor_deltas(fi).map_err(core_to_py)?,
            )?;
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

crate::bindings::macros::wire_methods!(
    PyFactorPnlProfile,
    finstack_quant_portfolio::sensitivity::FactorPnlProfile,
    "FactorPnlProfile"
);

impl PyFactorPnlProfile {
    fn from_inner(inner: finstack_quant_portfolio::sensitivity::FactorPnlProfile) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorPnlProfile {
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
        self.inner.validate().map_err(crate::errors::core_to_py)?;
        let data = PyDict::new(py);
        for (pi, pid) in self.inner.position_ids.iter().enumerate() {
            let column: Vec<f64> = self.inner.position_pnls.iter().map(|row| row[pi]).collect();
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

/// Deserialize factor definitions from a JSON string, list of dicts or
/// ``pandas.DataFrame``.
fn extract_factors(
    py: Python<'_>,
    factors: &Bound<'_, PyAny>,
) -> PyResult<Vec<finstack_quant_factor::FactorDefinition>> {
    let json = extract_records_json(py, factors, "factors")?;
    serde_json::from_str(&json).map_err(|e| serde_json_to_py(e, "invalid factors JSON"))
}

/// Deserialize an optional ``BumpSizeConfig`` from a JSON string or dict.
fn extract_bump_config(
    py: Python<'_>,
    bump_config: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<finstack_quant_factor::BumpSizeConfig>> {
    bump_config
        .map(|obj| {
            let json = extract_records_json(py, obj, "bump_config")?;
            serde_json::from_str(&json).map_err(|e| serde_json_to_py(e, "invalid bump_config JSON"))
        })
        .transpose()
}

/// Compute the position-by-factor sensitivity matrix of a portfolio.
///
/// Each cell is the central difference of the position's base-currency PV
/// under an up and a down bump of the factor, weighted by the position's
/// scale factor. Rows follow portfolio position order, columns follow
/// ``factors``, and every value is in ``portfolio.base_currency``.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     Built ``Portfolio`` or ``PortfolioSpec`` JSON.
/// factors : str | list | pandas.DataFrame
///     ``FactorDefinition`` array (JSON or records). Every factor must map to
///     market data; credit hierarchy factors need ``FactorModel``.
/// market : MarketContext | str
///     Market to bump, including FX for cross-currency positions.
/// as_of : datetime.date | str
///     Valuation date for pricing and spot FX.
/// bump_config : str | dict, optional
///     ``BumpSizeConfig``; defaults to 1 bp / 1 % per factor type.
///
/// Returns
/// -------
/// SensitivityMatrix
///     Positions × factors delta matrix.
#[pyfunction]
#[pyo3(signature = (portfolio, factors, market, as_of, bump_config=None))]
fn compute_factor_sensitivities(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    factors: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    bump_config: Option<&Bound<'_, PyAny>>,
) -> PyResult<PySensitivityMatrix> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let factors = extract_factors(py, factors)?;
    let market = extract_market_ref(py, market)?;
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    let bump_config = extract_bump_config(py, bump_config)?;
    let (portfolio, market) = (&*portfolio, &*market);
    let base_currency = portfolio.base_currency;
    py.detach(move || {
        finstack_quant_portfolio::sensitivity::compute_factor_sensitivities(
            portfolio,
            &factors,
            market,
            as_of,
            bump_config,
        )
    })
    .map(|matrix| PySensitivityMatrix::from_inner(matrix, base_currency))
    .map_err(core_to_py)
}

/// Compute full-repricing scenario P&L profiles of a portfolio, one per factor.
///
/// Parameters
/// ----------
/// portfolio : Portfolio | str
///     Built ``Portfolio`` or ``PortfolioSpec`` JSON; its positions are the
///     profile columns and its ``base_currency`` the reporting currency.
/// factors : str | list | pandas.DataFrame
///     ``FactorDefinition`` array (JSON or records).
/// market : MarketContext | str
///     Market reused by every scenario.
/// as_of : datetime.date | str
///     Valuation date for pricing and spot FX.
/// bump_config : str | dict, optional
///     ``BumpSizeConfig``; defaults to 1 bp / 1 % per factor type.
/// n_points : int, optional
///     Odd number of grid points in ``3..=1001``; defaults to 5
///     (shifts ``[-2, -1, 0, 1, 2]``).
///
/// Returns
/// -------
/// list[FactorPnlProfile]
///     One profile per factor, each containing scenario P&L for every position.
#[pyfunction]
#[pyo3(signature = (portfolio, factors, market, as_of, bump_config=None, n_points=None))]
fn compute_pnl_profiles(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    factors: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    bump_config: Option<&Bound<'_, PyAny>>,
    n_points: Option<usize>,
) -> PyResult<Vec<PyFactorPnlProfile>> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let factors = extract_factors(py, factors)?;
    let market = extract_market_ref(py, market)?;
    let as_of = crate::bindings::date_utils::py_to_date(as_of)?;
    let bump_config = extract_bump_config(py, bump_config)?;
    let (portfolio, market) = (&*portfolio, &*market);
    py.detach(move || {
        finstack_quant_portfolio::sensitivity::compute_pnl_profiles(
            portfolio,
            &factors,
            market,
            as_of,
            bump_config,
            n_points,
        )
    })
    .map(|profiles| {
        profiles
            .into_iter()
            .map(PyFactorPnlProfile::from_inner)
            .collect()
    })
    .map_err(core_to_py)
}

/// Register factor-model functions on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySensitivityMatrix>()?;
    m.add_class::<PyFactorPnlProfile>()?;
    m.add_function(pyo3::wrap_pyfunction!(compute_factor_sensitivities, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(compute_pnl_profiles, m)?)?;
    Ok(())
}
