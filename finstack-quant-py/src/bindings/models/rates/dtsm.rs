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

use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::wire_methods;
use crate::bindings::pandas_utils::{dates_to_datetime_index, dict_to_dataframe};
use crate::bindings::pydoc::pydoc;
use crate::errors::{core_to_py, value_error};
use finstack_quant_core::math::linalg::rows_of;
use finstack_quant_models::rates::dtsm::{
    self, DieboldLi, FactorTimeSeries, YieldForecast, YieldPanel, YieldPca, YieldPcaView,
};
use nalgebra::DVector;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule, PyType};

fn vector_values(vector: &DVector<f64>) -> Vec<f64> {
    vector.iter().copied().collect()
}

fn extract_dates(
    dates: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<Vec<finstack_quant_core::dates::Date>>> {
    match dates {
        None => Ok(None),
        Some(obj) if obj.is_none() => Ok(None),
        Some(obj) => obj
            .try_iter()?
            .map(|item| py_to_date(&item?))
            .collect::<PyResult<Vec<_>>>()
            .map(Some),
    }
}

fn dates_to_py<'py>(
    py: Python<'py>,
    dates: Option<&[finstack_quant_core::dates::Date]>,
) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
    dates
        .map(|values| values.iter().map(|&d| date_to_py(py, d)).collect())
        .transpose()
}

fn component_labels(count: usize) -> Vec<String> {
    (1..=count).map(|k| format!("PC{k}")).collect()
}

#[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPanel")]
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

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPanel.from_dataframe")]
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

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPanel.tenors")]
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPanel.dates")]
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        dates_to_py(py, self.inner.dates.as_deref())
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPanel.yields")]
    #[getter]
    fn yields(&self) -> Vec<Vec<f64>> {
        rows_of(&self.inner.yields)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPanel.num_dates")]
    #[getter]
    fn num_dates(&self) -> usize {
        self.inner.num_dates()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPanel.num_tenors")]
    #[getter]
    fn num_tenors(&self) -> usize {
        self.inner.num_tenors()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPanel.yield_changes")]
    fn yield_changes(&self) -> Vec<Vec<f64>> {
        rows_of(&self.inner.yield_changes())
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPanel.to_dataframe")]
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

wire_methods!(
    PyYieldPanel,
    YieldPanel,
    "YieldPanel",
    module = "finstack_quant.models.rates.dtsm"
);

#[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries")]
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
    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries.dates")]
    #[getter]
    fn dates<'py>(&self, py: Python<'py>) -> PyResult<Option<Vec<Bound<'py, PyAny>>>> {
        dates_to_py(py, self.inner.dates.as_deref())
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries.level")]
    #[getter]
    fn level(&self) -> Vec<f64> {
        self.column(0)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries.slope")]
    #[getter]
    fn slope(&self) -> Vec<f64> {
        self.column(1)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries.curvature")]
    #[getter]
    fn curvature(&self) -> Vec<f64> {
        self.column(2)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries.factors")]
    #[getter]
    fn factors(&self) -> Vec<Vec<f64>> {
        rows_of(&self.inner.factors)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries.residuals")]
    #[getter]
    fn residuals(&self) -> Vec<Vec<f64>> {
        rows_of(&self.inner.residuals)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries.r_squared")]
    #[getter]
    fn r_squared(&self) -> Vec<f64> {
        self.inner.r_squared.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries.r_squared_avg")]
    #[getter]
    fn r_squared_avg(&self) -> f64 {
        self.inner.r_squared_avg
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries.num_dates")]
    #[getter]
    fn num_dates(&self) -> usize {
        self.inner.num_dates()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "FactorTimeSeries.to_dataframe")]
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

    fn __repr__(&self) -> String {
        format!(
            "FactorTimeSeries(num_dates={}, num_tenors={}, r_squared_avg={:?})",
            self.inner.num_dates(),
            self.inner.r_squared.len(),
            self.inner.r_squared_avg
        )
    }
}

wire_methods!(
    PyFactorTimeSeries,
    FactorTimeSeries,
    "FactorTimeSeries",
    module = "finstack_quant.models.rates.dtsm"
);

#[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi")]
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
        DieboldLi::new(lambda_)
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.lambda_")]
    #[getter]
    fn lambda_(&self) -> f64 {
        self.inner.lambda()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.tenors")]
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.factors")]
    #[getter]
    fn factors(&self) -> Option<PyFactorTimeSeries> {
        self.inner
            .factors()
            .cloned()
            .map(PyFactorTimeSeries::from_inner)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.phi")]
    #[getter]
    fn phi(&self) -> Option<Vec<Vec<f64>>> {
        self.inner.phi().map(rows_of)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.mu")]
    #[getter]
    fn mu(&self) -> Option<Vec<f64>> {
        self.inner.mu().map(vector_values)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.q_cov")]
    #[getter]
    fn q_cov(&self) -> Option<Vec<Vec<f64>>> {
        self.inner.q_cov().map(rows_of)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.loading_matrix")]
    fn loading_matrix(&self) -> Vec<Vec<f64>> {
        rows_of(&self.inner.loading_matrix())
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.extract_factors")]
    #[pyo3(text_signature = "(self, panel)")]
    fn extract_factors(&self, py: Python<'_>, panel: PyYieldPanel) -> PyResult<Self> {
        let model = self.inner.clone();
        py.detach(|| model.extract_factors(&panel.inner))
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.fit_var")]
    fn fit_var(&self, py: Python<'_>) -> PyResult<Self> {
        let model = self.inner.clone();
        py.detach(|| model.fit_var())
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.fit")]
    #[pyo3(text_signature = "(self, panel)")]
    fn fit(&self, py: Python<'_>, panel: PyYieldPanel) -> PyResult<Self> {
        let model = self.inner.clone();
        py.detach(|| model.fit(&panel.inner))
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "DieboldLi.forecast")]
    #[pyo3(text_signature = "(self, horizon)")]
    fn forecast(&self, py: Python<'_>, horizon: usize) -> PyResult<PyYieldForecast> {
        py.detach(|| self.inner.forecast(horizon))
            .map(PyYieldForecast::from_inner)
            .map_err(core_to_py)
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

wire_methods!(
    PyDieboldLi,
    DieboldLi,
    "DieboldLi",
    module = "finstack_quant.models.rates.dtsm"
);

#[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldForecast")]
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
    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldForecast.horizon")]
    #[getter]
    fn horizon(&self) -> usize {
        self.inner.horizon
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldForecast.yields")]
    #[getter]
    fn yields(&self) -> Vec<f64> {
        self.inner.yields.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldForecast.tenors")]
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldForecast.factors")]
    #[getter]
    fn factors(&self) -> (f64, f64, f64) {
        (
            self.inner.factors[0],
            self.inner.factors[1],
            self.inner.factors[2],
        )
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldForecast.lower_95")]
    #[getter]
    fn lower_95(&self) -> Vec<f64> {
        self.inner.lower_95.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldForecast.upper_95")]
    #[getter]
    fn upper_95(&self) -> Vec<f64> {
        self.inner.upper_95.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldForecast.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("tenor", self.inner.tenors.clone())?;
        data.set_item("yield", self.inner.yields.clone())?;
        data.set_item("lower_95", self.inner.lower_95.clone())?;
        data.set_item("upper_95", self.inner.upper_95.clone())?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("YieldForecast", &self.inner)
    }
}

wire_methods!(
    PyYieldForecast,
    YieldForecast,
    "YieldForecast",
    module = "finstack_quant.models.rates.dtsm"
);

#[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca")]
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
    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.fit")]
    #[classmethod]
    #[pyo3(text_signature = "(cls, panel)")]
    fn fit(_cls: &Bound<'_, PyType>, py: Python<'_>, panel: PyYieldPanel) -> PyResult<Self> {
        py.detach(|| YieldPca::fit(&panel.inner))
            .map(Self::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.fit_yield_changes")]
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

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.num_components")]
    #[getter]
    fn num_components(&self) -> usize {
        self.inner.num_components()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.eigenvalues")]
    #[getter]
    fn eigenvalues(&self) -> Vec<f64> {
        self.inner.eigenvalues().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.loadings")]
    #[getter]
    fn loadings(&self) -> Vec<Vec<f64>> {
        rows_of(self.inner.loadings())
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.scores")]
    #[getter]
    fn scores(&self) -> Vec<Vec<f64>> {
        rows_of(self.inner.scores())
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.tenors")]
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.variance_explained")]
    #[getter]
    fn variance_explained(&self) -> Vec<f64> {
        self.inner.variance_explained().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.cumulative_variance")]
    #[getter]
    fn cumulative_variance(&self) -> Vec<f64> {
        self.inner.cumulative_variance().to_vec()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.mean_change")]
    #[getter]
    fn mean_change(&self) -> Vec<f64> {
        vector_values(self.inner.mean_change())
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.loading")]
    #[pyo3(text_signature = "(self, k)")]
    fn loading(&self, k: usize) -> PyResult<Vec<f64>> {
        self.inner
            .loading(k)
            .map(|v| vector_values(&v))
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.components_for_threshold")]
    #[pyo3(text_signature = "(self, threshold)")]
    fn components_for_threshold(&self, threshold: f64) -> usize {
        self.inner.components_for_threshold(threshold)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.scenario")]
    #[pyo3(text_signature = "(self, shocks)")]
    fn scenario(&self, shocks: Vec<f64>) -> PyResult<Vec<f64>> {
        self.inner.scenario(&shocks).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.apply_scenario")]
    #[pyo3(text_signature = "(self, base_yields, shocks)")]
    fn apply_scenario(&self, base_yields: Vec<f64>, shocks: Vec<f64>) -> PyResult<Vec<f64>> {
        self.inner
            .apply_scenario(&base_yields, &shocks)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.reconstruct")]
    #[pyo3(text_signature = "(self, num_components)")]
    fn reconstruct(&self, num_components: usize) -> PyResult<Vec<Vec<f64>>> {
        self.inner
            .reconstruct(num_components)
            .map(|m| rows_of(&m))
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.truncated")]
    #[pyo3(text_signature = "(self, n_components)")]
    fn truncated(&self, n_components: usize) -> PyResult<PyYieldPcaView> {
        self.inner
            .truncated(n_components)
            .map(PyYieldPcaView::from_inner)
            .map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPca.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        loadings_dataframe(py, &rows_of(self.inner.loadings()), self.inner.tenors())
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

wire_methods!(
    PyYieldPca,
    YieldPca,
    "YieldPca",
    module = "finstack_quant.models.rates.dtsm"
);

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

#[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView")]
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
    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView.loadings")]
    #[getter]
    fn loadings(&self) -> Vec<Vec<f64>> {
        self.inner.loadings.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView.scores")]
    #[getter]
    fn scores(&self) -> Vec<Vec<f64>> {
        self.inner.scores.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView.eigenvalues")]
    #[getter]
    fn eigenvalues(&self) -> Vec<f64> {
        self.inner.eigenvalues.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView.explained_variance_ratio")]
    #[getter]
    fn explained_variance_ratio(&self) -> Vec<f64> {
        self.inner.explained_variance_ratio.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView.cumulative_variance")]
    #[getter]
    fn cumulative_variance(&self) -> Vec<f64> {
        self.inner.cumulative_variance.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView.mean_change")]
    #[getter]
    fn mean_change(&self) -> Vec<f64> {
        self.inner.mean_change.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView.tenors")]
    #[getter]
    fn tenors(&self) -> Vec<f64> {
        self.inner.tenors.clone()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView.num_components")]
    #[getter]
    fn num_components(&self) -> usize {
        self.inner.eigenvalues.len()
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView.to_dataframe")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        loadings_dataframe(py, &self.inner.loadings, &self.inner.tenors)
    }

    #[doc = pydoc!("finstack_quant.models.rates.dtsm", "YieldPcaView.to_scores_dataframe")]
    fn to_scores_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let n_components = self.inner.eigenvalues.len();
        let data = PyDict::new(py);
        for (k, label) in component_labels(n_components).iter().enumerate() {
            let column: Vec<f64> = self.inner.scores.iter().map(|row| row[k]).collect();
            data.set_item(label, column)?;
        }
        dict_to_dataframe(py, &data, None)
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

wire_methods!(
    PyYieldPcaView,
    YieldPcaView,
    "YieldPcaView",
    module = "finstack_quant.models.rates.dtsm"
);

#[doc = pydoc!("finstack_quant.models.rates.dtsm", "nelson_siegel_yields")]
#[pyfunction]
#[pyo3(signature = (lambda_, factors, tenors))]
#[pyo3(text_signature = "(lambda_, factors, tenors)")]
fn nelson_siegel_yields(lambda_: f64, factors: [f64; 3], tenors: Vec<f64>) -> PyResult<Vec<f64>> {
    dtsm::nelson_siegel_yields(lambda_, factors, &tenors).map_err(core_to_py)
}

/// Build the `finstack_quant.models.rates.dtsm` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "dtsm")?;
    m.setattr(
        "__doc__",
        pydoc!("finstack_quant.models.rates.dtsm", "__module__"),
    )?;

    m.add_class::<PyDieboldLi>()?;
    m.add_class::<PyFactorTimeSeries>()?;
    m.add_class::<PyYieldForecast>()?;
    m.add_class::<PyYieldPanel>()?;
    m.add_class::<PyYieldPca>()?;
    m.add_class::<PyYieldPcaView>()?;
    m.add_function(wrap_pyfunction!(nelson_siegel_yields, &m)?)?;

    let all = PyList::new(
        py,
        [
            "DieboldLi",
            "FactorTimeSeries",
            "YieldForecast",
            "YieldPanel",
            "YieldPca",
            "YieldPcaView",
            "nelson_siegel_yields",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
