//! Python bindings for `finstack_quant_models::rates::dtsm`.
//!
//! Typed wrappers over the Rust dynamic term-structure engines:
//!
//! - `YieldPanel`: the canonical dated yield matrix input.
//! - `DieboldLi` / `FactorTimeSeries` / `YieldForecast`: dynamic Nelson-Siegel
//!   factor extraction, VAR(1) dynamics and forecasting.
//! - `YieldPca` / `YieldPcaView`: PCA of yield changes and scenario shocks.
//!
//! The free functions are thin twins over the same Rust entry points for
//! callers holding plain nested lists.

use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::pandas_utils::{dates_to_datetime_index, dict_to_dataframe};
use crate::errors::{core_to_py, serde_json_to_py, value_error};
use finstack_quant_models::rates::dtsm::{
    self, DieboldLi, FactorTimeSeries, YieldForecast, YieldPanel, YieldPca, YieldPcaView,
};
use nalgebra::{DMatrix, DVector};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule, PyType};

fn matrix_rows(matrix: &DMatrix<f64>) -> Vec<Vec<f64>> {
    (0..matrix.nrows())
        .map(|i| (0..matrix.ncols()).map(|j| matrix[(i, j)]).collect())
        .collect()
}

fn vector_values(vector: &DVector<f64>) -> Vec<f64> {
    vector.iter().copied().collect()
}

fn extract_dates(dates: Option<&Bound<'_, PyAny>>) -> PyResult<Option<Vec<time::Date>>> {
    match dates {
        None => Ok(None),
        Some(obj) if obj.is_none() => Ok(None),
        Some(obj) => obj
            .try_iter()?
            .map(|item| extract_date(&item?))
            .collect::<PyResult<Vec<_>>>()
            .map(Some),
    }
}

fn dates_to_py<'py>(
    py: Python<'py>,
    dates: Option<&[time::Date]>,
) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
    dates
        .map(|values| values.iter().map(|&d| date_to_py(py, d)).collect())
        .transpose()
}

fn component_labels(count: usize) -> Vec<String> {
    (1..=count).map(|k| format!("PC{k}")).collect()
}

/// Panel of continuously compounded zero yields: rows are observation dates,
/// columns are tenors in years.
///
/// Parameters
/// ----------
/// tenors : list[float]
///     Tenor grid in years, strictly ascending and positive (length ``N``).
/// yields : list[list[float]]
///     ``yields[date_idx][tenor_idx]`` decimal zero rates (``T`` rows of
///     ``N`` values, ``T >= 2``, all finite).
/// dates : Sequence[date | datetime | str] | None, default ``None``
///     Optional observation labels (length ``T``); any date-like value is
///     accepted, including ISO strings and ``pandas.Timestamp``.
///
/// Raises
/// ------
/// ValueError
///     If the tenor grid is not strictly ascending and positive, the yield
///     rows are empty/ragged/non-finite or do not match the tenor count,
///     fewer than two observations are supplied, or ``dates`` has the wrong
///     length.
#[pyclass(
    name = "YieldPanel",
    module = "finstack_quant.models.rates.dtsm",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyYieldPanel {
    pub(crate) inner: YieldPanel,
}

impl PyYieldPanel {
    pub(crate) fn from_inner(inner: YieldPanel) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyYieldPanel {
    #[new]
    #[pyo3(signature = (tenors, yields, dates = None))]
    #[pyo3(text_signature = "(tenors, yields, dates=None)")]
    fn new(
        tenors: Vec<f64>,
        yields: Vec<Vec<f64>>,
        dates: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let dates = extract_dates(dates)?;
        YieldPanel::from_rows(tenors, yields, dates)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Build a panel from a ``pandas.DataFrame``.
    ///
    /// Column labels are parsed as tenors in years (``float(label)``), the
    /// index supplies the observation dates when it is date-like (otherwise
    /// the panel is unlabeled), and the values are decimal zero rates.
    ///
    /// Raises ``TypeError`` if ``df`` is not a DataFrame and ``ValueError``
    /// if a column label is not numeric or the panel fails validation.
    #[classmethod]
    #[pyo3(text_signature = "(cls, df)")]
    fn from_dataframe(_cls: &Bound<'_, PyType>, df: &Bound<'_, PyAny>) -> PyResult<Self> {
        let py = df.py();
        let df_type = py.import("pandas")?.getattr("DataFrame")?;
        if !df.is_instance(&df_type)? {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "YieldPanel.from_dataframe expects a pandas DataFrame with tenor columns",
            ));
        }
        let columns: Vec<Bound<'_, PyAny>> =
            df.getattr("columns")?.call_method0("tolist")?.extract()?;
        let tenors = columns
            .iter()
            .map(|label| {
                py.import("builtins")?
                    .getattr("float")?
                    .call1((label,))?
                    .extract::<f64>()
                    .map_err(|_| {
                        value_error(format!(
                            "YieldPanel.from_dataframe: column label {label} is not a numeric tenor in years"
                        ))
                    })
            })
            .collect::<PyResult<Vec<f64>>>()?;
        let index_values = df.getattr("index")?.call_method0("tolist")?;
        let dates = extract_dates(Some(&index_values)).unwrap_or_default();
        let kwargs = PyDict::new(py);
        kwargs.set_item("dtype", "float64")?;
        let yields: Vec<Vec<f64>> = df
            .call_method("to_numpy", (), Some(&kwargs))?
            .call_method0("tolist")?
            .extract()?;
        YieldPanel::from_rows(tenors, yields, dates)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Tenor grid in years.
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors.clone()
    }

    /// Observation dates as ``datetime.date`` values, or ``None`` when unlabeled.
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        dates_to_py(py, self.inner.dates.as_deref())
    }

    /// Yield matrix as row-major nested lists (``yields[date_idx][tenor_idx]``).
    #[getter]
    fn yields(&self) -> Vec<Vec<f64>> {
        matrix_rows(&self.inner.yields)
    }

    /// Number of observation dates.
    #[getter]
    fn num_dates(&self) -> usize {
        self.inner.num_dates()
    }

    /// Number of tenors.
    #[getter]
    fn num_tenors(&self) -> usize {
        self.inner.num_tenors()
    }

    /// First differences of the yields as ``T-1`` row-major nested lists.
    fn yield_changes(&self) -> Vec<Vec<f64>> {
        matrix_rows(&self.inner.yield_changes())
    }

    /// The panel as a ``pandas.DataFrame`` (one column per tenor, in years).
    ///
    /// The index is a ``DatetimeIndex`` when the panel carries dates and a
    /// ``RangeIndex`` otherwise.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        for (j, tenor) in self.inner.tenors.iter().enumerate() {
            let column: Vec<f64> = (0..self.inner.num_dates())
                .map(|i| self.inner.yields[(i, j)])
                .collect();
            data.set_item(*tenor, column)?;
        }
        let index = match &self.inner.dates {
            Some(dates) => Some(dates_to_datetime_index(py, dates)?),
            None => None,
        };
        dict_to_dataframe(py, &data, index)
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "YieldPanel serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid YieldPanel JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "YieldPanel(num_dates={}, num_tenors={}, tenors={:?}, dated={})",
            self.inner.num_dates(),
            self.inner.num_tenors(),
            self.inner.tenors,
            if self.inner.dates.is_some() {
                "True"
            } else {
                "False"
            }
        )
    }
}

/// Time series of Nelson-Siegel factors extracted by ``DieboldLi``.
///
/// ``level`` (beta1), ``slope`` (beta2) and ``curvature`` (beta3) are in
/// decimal yield units, one value per observation date; ``residuals`` is the
/// ``T x N`` OLS residual matrix and ``r_squared`` the per-tenor fit quality.
#[pyclass(
    name = "FactorTimeSeries",
    module = "finstack_quant.models.rates.dtsm",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyFactorTimeSeries {
    pub(crate) inner: FactorTimeSeries,
}

impl PyFactorTimeSeries {
    pub(crate) fn from_inner(inner: FactorTimeSeries) -> Self {
        Self { inner }
    }

    fn column(&self, k: usize) -> Vec<f64> {
        (0..self.inner.factors.nrows())
            .map(|i| self.inner.factors[(i, k)])
            .collect()
    }
}

#[pymethods]
impl PyFactorTimeSeries {
    /// Observation dates as ``datetime.date`` values, or ``None`` when the
    /// source panel was unlabeled.
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        dates_to_py(py, self.inner.dates.as_deref())
    }

    /// Level factor (beta1) per date.
    #[getter]
    fn level(&self) -> Vec<f64> {
        self.column(0)
    }

    /// Slope factor (beta2) per date.
    #[getter]
    fn slope(&self) -> Vec<f64> {
        self.column(1)
    }

    /// Curvature factor (beta3) per date.
    #[getter]
    fn curvature(&self) -> Vec<f64> {
        self.column(2)
    }

    /// Factor matrix as row-major nested lists ``factors[date_idx] = [level, slope, curvature]``.
    #[getter]
    fn factors(&self) -> Vec<Vec<f64>> {
        matrix_rows(&self.inner.factors)
    }

    /// OLS residual matrix as row-major nested lists (``residuals[date_idx][tenor_idx]``).
    #[getter]
    fn residuals(&self) -> Vec<Vec<f64>> {
        self.inner.residual_rows()
    }

    /// Cross-sectional R-squared per tenor.
    #[getter]
    fn r_squared(&self) -> Vec<f64> {
        self.inner.r_squared.clone()
    }

    /// Average R-squared across tenors.
    #[getter]
    fn r_squared_avg(&self) -> f64 {
        self.inner.r_squared_avg
    }

    /// Number of observation dates.
    #[getter]
    fn num_dates(&self) -> usize {
        self.inner.num_dates()
    }

    /// Factors as a ``pandas.DataFrame`` with columns ``level``, ``slope``,
    /// ``curvature``.
    ///
    /// The index is a ``DatetimeIndex`` when dates are available and a
    /// ``RangeIndex`` otherwise.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("level", self.column(0))?;
        data.set_item("slope", self.column(1))?;
        data.set_item("curvature", self.column(2))?;
        let index = match &self.inner.dates {
            Some(dates) => Some(dates_to_datetime_index(py, dates)?),
            None => None,
        };
        dict_to_dataframe(py, &data, index)
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "FactorTimeSeries serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid FactorTimeSeries JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorTimeSeries(num_dates={}, num_tenors={}, r_squared_avg={:?})",
            self.inner.num_dates(),
            self.inner.r_squared.len(),
            self.inner.r_squared_avg
        )
    }
}

/// Diebold-Li (2006) dynamic Nelson-Siegel model.
///
/// Parameters
/// ----------
/// lambda_ : float | None, default ``None``
///     Decay parameter for tenors **in years**; must be finite and positive.
///     ``None`` uses the Rust default ``0.7308`` (years-equivalent of
///     Diebold-Li's canonical ``0.0609`` months value, curvature peak near
///     2.45 years). Named ``lambda_`` because ``lambda`` is a Python keyword.
///
/// Raises
/// ------
/// ValueError
///     If ``lambda_`` is supplied but non-finite or not strictly positive.
#[pyclass(
    name = "DieboldLi",
    module = "finstack_quant.models.rates.dtsm",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyDieboldLi {
    pub(crate) inner: DieboldLi,
}

impl PyDieboldLi {
    pub(crate) fn from_inner(inner: DieboldLi) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyDieboldLi {
    #[new]
    #[pyo3(signature = (lambda_ = None))]
    #[pyo3(text_signature = "(lambda_=None)")]
    fn new(lambda_: Option<f64>) -> PyResult<Self> {
        dtsm::diebold_li_model(lambda_)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Decay parameter in the years convention.
    #[getter]
    fn lambda_(&self) -> f64 {
        self.inner.lambda()
    }

    /// Tenor grid recorded from the last extraction (empty before ``extract_factors``).
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors().to_vec()
    }

    /// Extracted factor time series, or ``None`` before ``extract_factors``.
    #[getter]
    fn factors(&self) -> Option<PyFactorTimeSeries> {
        self.inner
            .factors()
            .cloned()
            .map(PyFactorTimeSeries::from_inner)
    }

    /// VAR(1) coefficient matrix ``Phi`` (3x3, row-major), or ``None`` before ``fit_var``.
    #[getter]
    fn phi(&self) -> Option<Vec<Vec<f64>>> {
        self.inner.phi().map(matrix_rows)
    }

    /// VAR(1) unconditional mean ``mu`` (length 3), or ``None`` before ``fit_var``.
    #[getter]
    fn mu(&self) -> Option<Vec<f64>> {
        self.inner.mu().map(vector_values)
    }

    /// VAR(1) residual covariance ``Q`` (3x3, row-major), or ``None`` before ``fit_var``.
    #[getter]
    fn q_cov(&self) -> Option<Vec<Vec<f64>>> {
        self.inner.q_cov().map(matrix_rows)
    }

    /// Nelson-Siegel loading matrix (``N x 3``, row-major) for the recorded tenors.
    fn loading_matrix(&self) -> Vec<Vec<f64>> {
        matrix_rows(&self.inner.loading_matrix())
    }

    /// Extract level/slope/curvature factors from ``panel`` via OLS.
    ///
    /// Returns a new model; the receiver is unchanged.
    ///
    /// Raises ``ValueError`` if the panel has fewer than three tenors or the
    /// loading matrix is singular.
    #[pyo3(text_signature = "(self, panel)")]
    fn extract_factors(&self, py: Python<'_>, panel: PyYieldPanel) -> PyResult<Self> {
        let model = self.inner.clone();
        py.detach(|| model.extract_factors(&panel.inner))
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Fit VAR(1) dynamics to the extracted factors.
    ///
    /// Returns a new model; the receiver is unchanged.
    ///
    /// Raises ``ValueError`` if factors have not been extracted or fewer than
    /// five observations are available.
    fn fit_var(&self, py: Python<'_>) -> PyResult<Self> {
        let model = self.inner.clone();
        py.detach(|| model.fit_var())
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// ``extract_factors(panel)`` followed by ``fit_var()``.
    ///
    /// Raises ``ValueError`` on any validation failure of either step.
    #[pyo3(text_signature = "(self, panel)")]
    fn fit(&self, py: Python<'_>, panel: PyYieldPanel) -> PyResult<Self> {
        let model = self.inner.clone();
        py.detach(|| model.fit(&panel.inner))
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Forecast the curve ``horizon`` observation periods ahead.
    ///
    /// Raises ``ValueError`` if the VAR has not been fitted or ``horizon`` is zero.
    #[pyo3(text_signature = "(self, horizon)")]
    fn forecast(&self, py: Python<'_>, horizon: usize) -> PyResult<PyYieldForecast> {
        py.detach(|| self.inner.forecast(horizon))
            .map(PyYieldForecast::from_inner)
            .map_err(core_to_py)
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "DieboldLi serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed or ``lambda`` is invalid.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid DieboldLi JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "DieboldLi(lambda_={:?}, factors_extracted={}, var_fitted={})",
            self.inner.lambda(),
            if self.inner.factors().is_some() {
                "True"
            } else {
                "False"
            },
            if self.inner.phi().is_some() {
                "True"
            } else {
                "False"
            }
        )
    }
}

/// h-step-ahead Diebold-Li yield-curve forecast with 95% Gaussian bands.
#[pyclass(
    name = "YieldForecast",
    module = "finstack_quant.models.rates.dtsm",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyYieldForecast {
    pub(crate) inner: YieldForecast,
}

impl PyYieldForecast {
    pub(crate) fn from_inner(inner: YieldForecast) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyYieldForecast {
    /// Forecast horizon in observation periods.
    #[getter]
    fn horizon(&self) -> usize {
        self.inner.horizon
    }

    /// Point-forecast decimal zero rates, one per tenor.
    #[getter]
    fn yields(&self) -> Vec<f64> {
        self.inner.yields.clone()
    }

    /// Tenor grid in years.
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors.clone()
    }

    /// Forecast factor triple ``(level, slope, curvature)``.
    #[getter]
    fn factors(&self) -> (f64, f64, f64) {
        (
            self.inner.factors[0],
            self.inner.factors[1],
            self.inner.factors[2],
        )
    }

    /// Lower 95% band per tenor.
    #[getter]
    fn lower_95(&self) -> Vec<f64> {
        self.inner.lower_95.clone()
    }

    /// Upper 95% band per tenor.
    #[getter]
    fn upper_95(&self) -> Vec<f64> {
        self.inner.upper_95.clone()
    }

    /// Forecast as a ``pandas.DataFrame`` with columns ``tenor``, ``yield``,
    /// ``lower_95``, ``upper_95`` (one row per tenor).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("tenor", self.inner.tenors.clone())?;
        data.set_item("yield", self.inner.yields.clone())?;
        data.set_item("lower_95", self.inner.lower_95.clone())?;
        data.set_item("upper_95", self.inner.upper_95.clone())?;
        dict_to_dataframe(py, &data, None)
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "YieldForecast serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid YieldForecast JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("YieldForecast", &self.inner)
    }
}

/// PCA decomposition of yield-curve changes (Litterman-Scheinkman).
///
/// Construct with ``YieldPca.fit(panel)`` or ``YieldPca.fit_yield_changes``.
#[pyclass(
    name = "YieldPca",
    module = "finstack_quant.models.rates.dtsm",
    frozen,
    from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyYieldPca {
    pub(crate) inner: YieldPca,
}

impl PyYieldPca {
    pub(crate) fn from_inner(inner: YieldPca) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyYieldPca {
    /// Fit PCA to the first differences of ``panel``.
    ///
    /// Raises ``ValueError`` for fewer than two tenors, fewer than three
    /// observations, or a degenerate covariance matrix.
    #[classmethod]
    #[pyo3(text_signature = "(cls, panel)")]
    fn fit(_cls: &Bound<'_, PyType>, py: Python<'_>, panel: PyYieldPanel) -> PyResult<Self> {
        py.detach(|| YieldPca::fit(&panel.inner))
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Fit PCA to already-differenced yields (``yield_changes[t][tenor]``).
    ///
    /// The fitted object carries a synthetic tenor grid ``1.0, 2.0, ..., N``
    /// because the changes do not identify the maturities; ``tenors`` on the
    /// result are placeholders in loading-row order.
    ///
    /// Raises ``ValueError`` for empty/ragged rows, fewer than two rows or
    /// tenors, or a degenerate covariance matrix.
    #[classmethod]
    #[pyo3(text_signature = "(cls, yield_changes)")]
    fn fit_yield_changes(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        yield_changes: Vec<Vec<f64>>,
    ) -> PyResult<Self> {
        py.detach(|| YieldPca::fit_yield_changes(yield_changes))
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    /// Number of extracted components (``min(T-1, N)``).
    #[getter]
    fn num_components(&self) -> usize {
        self.inner.num_components()
    }

    /// Eigenvalues in descending order.
    #[getter]
    fn eigenvalues(&self) -> Vec<f64> {
        self.inner.eigenvalues().to_vec()
    }

    /// Loadings as row-major nested lists ``loadings[tenor][k]``.
    #[getter]
    fn loadings(&self) -> Vec<Vec<f64>> {
        matrix_rows(self.inner.loadings())
    }

    /// Scores as row-major nested lists ``scores[t][k]``.
    #[getter]
    fn scores(&self) -> Vec<Vec<f64>> {
        matrix_rows(self.inner.scores())
    }

    /// Tenor grid in years (synthetic ``1..N`` after ``fit_yield_changes``).
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors().to_vec()
    }

    /// Fraction of variance explained by each component.
    #[getter]
    fn variance_explained(&self) -> Vec<f64> {
        self.inner.variance_explained().to_vec()
    }

    /// Cumulative fraction of variance explained.
    #[getter]
    fn cumulative_variance(&self) -> Vec<f64> {
        self.inner.cumulative_variance().to_vec()
    }

    /// Mean yield change subtracted before PCA (one per tenor).
    #[getter]
    fn mean_change(&self) -> Vec<f64> {
        vector_values(self.inner.mean_change())
    }

    /// Loading vector of component ``k`` (0-based, length ``N``).
    ///
    /// Raises ``ValueError`` if ``k`` is out of range.
    #[pyo3(text_signature = "(self, k)")]
    fn loading(&self, k: usize) -> PyResult<Vec<f64>> {
        self.inner
            .loading(k)
            .map(|v| vector_values(&v))
            .map_err(core_to_py)
    }

    /// Number of leading components explaining at least ``threshold`` of variance.
    #[pyo3(text_signature = "(self, threshold)")]
    fn components_for_threshold(&self, threshold: f64) -> usize {
        self.inner.components_for_threshold(threshold)
    }

    /// Yield-change vector for standard-deviation ``shocks`` along the leading components.
    ///
    /// Raises ``ValueError`` if more shocks than components are given.
    #[pyo3(text_signature = "(self, shocks)")]
    fn scenario(&self, shocks: Vec<f64>) -> PyResult<Vec<f64>> {
        self.inner.scenario(&shocks).map_err(core_to_py)
    }

    /// ``base_yields`` shifted by ``scenario(shocks)``.
    ///
    /// Raises ``ValueError`` if ``base_yields`` does not match the tenor count
    /// or more shocks than components are given.
    #[pyo3(text_signature = "(self, base_yields, shocks)")]
    fn apply_scenario(&self, base_yields: Vec<f64>, shocks: Vec<f64>) -> PyResult<Vec<f64>> {
        self.inner
            .apply_scenario(&base_yields, &shocks)
            .map_err(core_to_py)
    }

    /// Reconstruct the yield changes from the leading ``num_components``
    /// (row-major, mean added back).
    ///
    /// Raises ``ValueError`` if ``num_components`` is zero or too large.
    #[pyo3(text_signature = "(self, num_components)")]
    fn reconstruct(&self, num_components: usize) -> PyResult<Vec<Vec<f64>>> {
        self.inner
            .reconstruct(num_components)
            .map(|m| matrix_rows(&m))
            .map_err(core_to_py)
    }

    /// Serializable view of the leading ``n_components`` components.
    ///
    /// Raises ``ValueError`` if ``n_components`` is zero or exceeds ``num_components``.
    #[pyo3(text_signature = "(self, n_components)")]
    fn truncated(&self, n_components: usize) -> PyResult<PyYieldPcaView> {
        self.inner
            .truncated(n_components)
            .map(PyYieldPcaView::from_inner)
            .map_err(core_to_py)
    }

    /// Loadings as a ``pandas.DataFrame`` indexed by tenor with columns
    /// ``PC1, PC2, ...``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        loadings_dataframe(py, &matrix_rows(self.inner.loadings()), self.inner.tenors())
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "YieldPca serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid YieldPca JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "YieldPca(num_components={}, num_tenors={}, cumulative_variance={:?})",
            self.inner.num_components(),
            self.inner.tenors().len(),
            self.inner.cumulative_variance()
        )
    }
}

fn loadings_dataframe<'py>(
    py: Python<'py>,
    loadings: &[Vec<f64>],
    tenors: &[f64],
) -> PyResult<Bound<'py, PyAny>> {
    let n_components = loadings.first().map_or(0, Vec::len);
    let data = PyDict::new(py);
    for (k, label) in component_labels(n_components).iter().enumerate() {
        let column: Vec<f64> = loadings.iter().map(|row| row[k]).collect();
        data.set_item(label, column)?;
    }
    let index = PyList::new(py, tenors)?;
    dict_to_dataframe(py, &data, Some(index.into_any()))
}

/// Leading components of a ``YieldPca`` fit in plain nested-list form.
///
/// ``explained_variance_ratio`` is the per-component variance share (the
/// ``variance_explained`` accessor on ``YieldPca``); ``cumulative_variance``
/// accumulates it. ``tenors`` are placeholders ``1..N`` when the fit came from
/// ``fit_yield_changes``.
#[pyclass(
    name = "YieldPcaView",
    module = "finstack_quant.models.rates.dtsm",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyYieldPcaView {
    pub(crate) inner: YieldPcaView,
}

impl PyYieldPcaView {
    pub(crate) fn from_inner(inner: YieldPcaView) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyYieldPcaView {
    /// Row-major loadings ``loadings[tenor][k]``.
    #[getter]
    fn loadings(&self) -> Vec<Vec<f64>> {
        self.inner.loadings.clone()
    }

    /// Row-major scores ``scores[t][k]``.
    #[getter]
    fn scores(&self) -> Vec<Vec<f64>> {
        self.inner.scores.clone()
    }

    /// Leading eigenvalues, descending.
    #[getter]
    fn eigenvalues(&self) -> Vec<f64> {
        self.inner.eigenvalues.clone()
    }

    /// Fraction of total variance explained by each leading component.
    #[getter]
    fn explained_variance_ratio(&self) -> Vec<f64> {
        self.inner.explained_variance_ratio.clone()
    }

    /// Cumulative explained-variance fraction.
    #[getter]
    fn cumulative_variance(&self) -> Vec<f64> {
        self.inner.cumulative_variance.clone()
    }

    /// Column means subtracted before PCA (one per tenor).
    #[getter]
    fn mean_change(&self) -> Vec<f64> {
        self.inner.mean_change.clone()
    }

    /// Tenor grid in loading-row order.
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors.clone()
    }

    /// Number of leading components in the view.
    #[getter]
    fn num_components(&self) -> usize {
        self.inner.eigenvalues.len()
    }

    /// Loadings as a ``pandas.DataFrame`` indexed by tenor with columns ``PC1, PC2, ...``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        loadings_dataframe(py, &self.inner.loadings, &self.inner.tenors)
    }

    /// Scores as a ``pandas.DataFrame`` (one row per yield-change observation,
    /// columns ``PC1, PC2, ...``).
    fn to_scores_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let n_components = self.inner.eigenvalues.len();
        let data = PyDict::new(py);
        for (k, label) in component_labels(n_components).iter().enumerate() {
            let column: Vec<f64> = self.inner.scores.iter().map(|row| row[k]).collect();
            data.set_item(label, column)?;
        }
        dict_to_dataframe(py, &data, None)
    }

    /// Serialize to the canonical JSON wire format.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|err| serde_json_to_py(err, "YieldPcaView serialization failed"))
    }

    /// Deserialize from JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` when the payload is malformed.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(Self::from_inner)
            .map_err(|err| serde_json_to_py(err, "invalid YieldPcaView JSON"))
    }

    /// Support ``pickle``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "YieldPcaView(num_components={}, num_tenors={}, explained_variance_ratio={:?})",
            self.inner.eigenvalues.len(),
            self.inner.tenors.len(),
            self.inner.explained_variance_ratio
        )
    }
}

/// Extract time-varying Nelson-Siegel factors (level, slope, curvature) from a
/// yield panel using the Diebold-Li (2006) parameterization.
///
/// Thin twin of ``DieboldLi(lambda_).extract_factors(YieldPanel(tenors, yields_matrix)).factors``.
///
/// Parameters
/// ----------
/// tenors : list[float]
///     Tenor grid in years, strictly ascending and positive (length ``N``).
/// yields_matrix : list[list[float]]
///     ``yields_matrix[date_idx][tenor_idx]`` decimal zero rates.
/// lambda_ : float | None, default ``None``
///     Decay parameter for tenors in years; ``None`` uses the Rust default
///     ``0.7308``.
///
/// Returns
/// -------
/// FactorTimeSeries
///     Extracted factors with ``to_dataframe()``.
///
/// Raises
/// ------
/// ValueError
///     If the panel is malformed, has fewer than three tenors, or
///     ``lambda_`` is invalid.
#[pyfunction]
#[pyo3(signature = (tenors, yields_matrix, lambda_ = None))]
#[pyo3(text_signature = "(tenors, yields_matrix, lambda_=None)")]
fn diebold_li_fit_factors(
    py: Python<'_>,
    tenors: Vec<f64>,
    yields_matrix: Vec<Vec<f64>>,
    lambda_: Option<f64>,
) -> PyResult<PyFactorTimeSeries> {
    py.detach(|| dtsm::diebold_li_fit_factors(tenors, yields_matrix, lambda_))
        .map(PyFactorTimeSeries::from_inner)
        .map_err(core_to_py)
}

/// Extract Diebold-Li factors, fit VAR(1) dynamics, and forecast the yield
/// curve ``horizon`` steps ahead.
///
/// Thin twin of ``DieboldLi(lambda_).fit(panel).forecast(horizon)``.
///
/// Parameters
/// ----------
/// tenors : list[float]
///     Tenor grid in years, length ``N``.
/// yields_matrix : list[list[float]]
///     ``yields_matrix[date_idx][tenor_idx]`` decimal zero rates (at least
///     five rows for the VAR fit).
/// horizon : int
///     Forecast horizon in observation periods (``>= 1``).
/// lambda_ : float | None, default ``None``
///     Decay parameter for tenors in years; ``None`` uses the Rust default.
///
/// Returns
/// -------
/// YieldForecast
///     Point forecast, factor triple and 95% bands with ``to_dataframe()``.
///
/// Raises
/// ------
/// ValueError
///     If the panel is malformed, too short for the VAR fit, ``horizon`` is
///     zero or ``lambda_`` is invalid.
#[pyfunction]
#[pyo3(signature = (tenors, yields_matrix, horizon, lambda_ = None))]
#[pyo3(text_signature = "(tenors, yields_matrix, horizon, lambda_=None)")]
fn diebold_li_forecast(
    py: Python<'_>,
    tenors: Vec<f64>,
    yields_matrix: Vec<Vec<f64>>,
    horizon: usize,
    lambda_: Option<f64>,
) -> PyResult<PyYieldForecast> {
    py.detach(|| dtsm::diebold_li_forecast(tenors, yields_matrix, horizon, lambda_))
        .map(PyYieldForecast::from_inner)
        .map_err(core_to_py)
}

/// Fit PCA to a matrix of yield changes and return the leading components.
///
/// Thin twin of ``YieldPca.fit_yield_changes(yield_changes).truncated(n_components)``.
///
/// Parameters
/// ----------
/// yield_changes : list[list[float]]
///     ``yield_changes[t][tenor]`` in decimal units (e.g. ``numpy.diff(yields, axis=0)``).
/// n_components : int, default ``3``
///     Number of leading components to keep (``1..=min(T-1, N)``).
///
/// Returns
/// -------
/// YieldPcaView
///     Loadings, scores, eigenvalues and variance shares with ``to_dataframe()``.
///     ``tenors`` are placeholders ``1..N`` because yield changes do not
///     identify the maturities.
///
/// Raises
/// ------
/// ValueError
///     If the panel is empty/ragged/non-finite, has fewer than two rows or
///     tenors, or ``n_components`` is out of range.
#[pyfunction]
#[pyo3(signature = (yield_changes, n_components = dtsm::DEFAULT_PCA_COMPONENTS))]
#[pyo3(text_signature = "(yield_changes, n_components=3)")]
fn yield_pca_fit(
    py: Python<'_>,
    yield_changes: Vec<Vec<f64>>,
    n_components: usize,
) -> PyResult<PyYieldPcaView> {
    py.detach(|| YieldPca::fit_yield_changes(yield_changes)?.truncated(n_components))
        .map(PyYieldPcaView::from_inner)
        .map_err(core_to_py)
}

/// Generate a single-component N-sigma PCA scenario shift to the yield curve.
///
/// Returns ``delta_yield = sigma_shock * sqrt(eigenvalue_k) * loading_k``,
/// i.e. the yield-change vector that corresponds to a ``sigma_shock``-sigma
/// move along principal component ``component_index``.
///
/// Parameters
/// ----------
/// yield_changes : list[list[float]]
///     ``yield_changes[t][tenor]`` matrix of decimal yield changes.
/// component_index : int
///     0-based principal component to shock (``< n_components``).
/// sigma_shock : float
///     Shock size in standard deviations (``2.0`` for +2 sigma).
/// n_components : int, default ``3``
///     Number of PCs to fit; used for bounds checking on ``component_index``.
///
/// Returns
/// -------
/// list[float]
///     Yield-change vector of length ``N`` in decimal units.
///
/// Raises
/// ------
/// ValueError
///     If the panel is malformed or ``component_index`` / ``n_components``
///     are out of range.
#[pyfunction]
#[pyo3(signature = (yield_changes, component_index, sigma_shock, n_components = dtsm::DEFAULT_PCA_COMPONENTS))]
#[pyo3(text_signature = "(yield_changes, component_index, sigma_shock, n_components=3)")]
fn yield_pca_scenario(
    yield_changes: Vec<Vec<f64>>,
    component_index: usize,
    sigma_shock: f64,
    n_components: usize,
) -> PyResult<Vec<f64>> {
    YieldPca::scenario_from_yield_changes(yield_changes, component_index, sigma_shock, n_components)
        .map_err(core_to_py)
}

/// Evaluate the static Nelson-Siegel (1987) yield curve for a given decay
/// parameter, factor triple, and tenor grid.
///
/// This is the Diebold-Li cross-sectional equation evaluated for a single date:
/// ``y(tau) = beta1 + beta2 * slope(tau) + beta3 * (slope(tau) - exp(-lambda*tau))``
/// where ``slope(tau) = (1 - exp(-lambda*tau)) / (lambda*tau)``. Use it to
/// reconstruct a fitted or forecast curve from factors returned by
/// ``DieboldLi`` / ``diebold_li_forecast``.
///
/// Parameters
/// ----------
/// lambda_ : float
///     Decay parameter for tenors **in years**; must be finite and > 0.
///     ``0.7308`` is the years-equivalent of Diebold-Li's canonical
///     ``0.0609`` months value.
/// factors : tuple[float, float, float]
///     ``(beta1, beta2, beta3)`` = ``(level, slope, curvature)`` in decimal
///     yield units (``0.045`` = 4.5%). Exactly three finite values.
/// tenors : list[float]
///     Maturities in years, each finite and >= 0. Order is preserved in the
///     output; no sorting or de-duplication is applied.
///
/// Returns
/// -------
/// list[float]
///     Fitted yields in decimal units, one per input tenor.
///
/// Raises
/// ------
/// ValueError
///     If ``lambda_`` is non-positive/non-finite, a factor is non-finite, or
///     a tenor is negative/non-finite.
#[pyfunction]
#[pyo3(signature = (lambda_, factors, tenors))]
#[pyo3(text_signature = "(lambda_, factors, tenors)")]
fn nelson_siegel_yields(lambda_: f64, factors: [f64; 3], tenors: Vec<f64>) -> PyResult<Vec<f64>> {
    dtsm::nelson_siegel_yields(lambda_, factors, &tenors).map_err(core_to_py)
}

/// Build the `finstack_quant.models.rates.dtsm` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "dtsm")?;
    m.setattr(
        "__doc__",
        "Dynamic term structure models: Diebold-Li dynamic Nelson-Siegel and yield-curve PCA.",
    )?;

    m.add_class::<PyDieboldLi>()?;
    m.add_class::<PyFactorTimeSeries>()?;
    m.add_class::<PyYieldForecast>()?;
    m.add_class::<PyYieldPanel>()?;
    m.add_class::<PyYieldPca>()?;
    m.add_class::<PyYieldPcaView>()?;
    m.add_function(wrap_pyfunction!(diebold_li_fit_factors, &m)?)?;
    m.add_function(wrap_pyfunction!(diebold_li_forecast, &m)?)?;
    m.add_function(wrap_pyfunction!(nelson_siegel_yields, &m)?)?;
    m.add_function(wrap_pyfunction!(yield_pca_fit, &m)?)?;
    m.add_function(wrap_pyfunction!(yield_pca_scenario, &m)?)?;

    let all = PyList::new(
        py,
        [
            "DieboldLi",
            "FactorTimeSeries",
            "YieldForecast",
            "YieldPanel",
            "YieldPca",
            "YieldPcaView",
            "diebold_li_fit_factors",
            "diebold_li_forecast",
            "nelson_siegel_yields",
            "yield_pca_fit",
            "yield_pca_scenario",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "dtsm",
        "finstack_quant.models.rates",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
