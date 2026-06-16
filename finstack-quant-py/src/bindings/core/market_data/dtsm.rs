//! Python bindings for `finstack_quant_core::market_data::dtsm`.
//!
//! Exposes a function-based API for dynamic term structure models:
//!
//! - Diebold-Li (2006) dynamic Nelson-Siegel factor extraction.
//! - Diebold-Li VAR(1) forecast of the yield curve.
//! - PCA decomposition of yield curve changes.
//! - PCA-based scenario generation (N-sigma shocks along principal components).

use finstack_quant_core::market_data::dtsm::{DieboldLi, YieldPanel, YieldPca};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

use crate::errors::core_to_py;

// ---------------------------------------------------------------------------
// Diebold-Li: factor extraction
// ---------------------------------------------------------------------------

/// Extract time-varying Nelson-Siegel factors (level, slope, curvature) from a
/// yield panel using the Diebold-Li (2006) parameterization.
///
/// Arguments:
///     tenors: Tenor grid in years, length N, strictly ascending and all > 0.
///     yields_matrix: Yield panel ``yields_matrix[date_idx][tenor_idx]`` as a
///         list of lists (T rows of N tenors each). Yields are continuously
///         compounded zero rates.
///     lambda: Diebold-Li decay parameter for tenors **in years** (default
///         0.7308, the years-equivalent of Diebold-Li's canonical 0.0609
///         months value; curvature loading peaks at ~2.45 years ~ 30 months).
///         Pass a months-scaled lambda only if you also supply tenors in
///         months.
///
/// Returns a dict with keys:
///     beta1: list[float] -- level factor per date (length T).
///     beta2: list[float] -- slope factor per date (length T).
///     beta3: list[float] -- curvature factor per date (length T).
///     r_squared: list[float] -- R-squared per tenor (length N).
///     r_squared_avg: float -- cross-sectional average R-squared.
#[pyfunction]
#[pyo3(signature = (tenors, yields_matrix, lambda=0.7308))]
#[pyo3(text_signature = "(tenors, yields_matrix, lambda=0.7308)")]
fn diebold_li_fit_factors<'py>(
    py: Python<'py>,
    tenors: Vec<f64>,
    yields_matrix: Vec<Vec<f64>>,
    lambda: f64,
) -> PyResult<Bound<'py, PyDict>> {
    let model = py
        .detach(|| {
            let panel = YieldPanel::from_rows(tenors, yields_matrix, None)?;
            DieboldLi::builder()
                .lambda(lambda)
                .build()?
                .extract_factors(&panel)
        })
        .map_err(core_to_py)?;

    let fts = model
        .factors()
        .ok_or_else(|| crate::errors::value_error("factor extraction produced no factors"))?;
    let t = fts.factors.nrows();

    let mut beta1 = Vec::with_capacity(t);
    let mut beta2 = Vec::with_capacity(t);
    let mut beta3 = Vec::with_capacity(t);
    for i in 0..t {
        beta1.push(fts.factors[(i, 0)]);
        beta2.push(fts.factors[(i, 1)]);
        beta3.push(fts.factors[(i, 2)]);
    }

    let d = PyDict::new(py);
    d.set_item("beta1", beta1)?;
    d.set_item("beta2", beta2)?;
    d.set_item("beta3", beta3)?;
    d.set_item("r_squared", fts.r_squared.clone())?;
    d.set_item("r_squared_avg", fts.r_squared_avg)?;
    Ok(d)
}

// ---------------------------------------------------------------------------
// Diebold-Li: forecast
// ---------------------------------------------------------------------------

/// Extract Diebold-Li factors, fit VAR(1) dynamics, and forecast the yield
/// curve ``horizon`` steps ahead.
///
/// Arguments:
///     tenors: Tenor grid in years, length N.
///     yields_matrix: Yield panel as ``yields_matrix[date_idx][tenor_idx]``
///         (T rows, N columns).
///     horizon: Forecast horizon (>= 1) in observation periods.
///     lambda: Diebold-Li decay parameter for tenors **in years** (default
///         0.7308; see ``diebold_li_fit_factors`` for the years-vs-months
///         convention).
///
/// Returns a dict with keys:
///     horizon: int -- the forecast horizon.
///     tenors: list[float] -- the tenor grid (length N).
///     forecast_factors: list[float] -- ``[beta1, beta2, beta3]`` forecast.
///     forecast_yields: list[float] -- point forecast yields (length N).
///     confidence_bands: dict with keys ``"lower_95"`` and ``"upper_95"``,
///         each a list[float] of length N (95% Gaussian forecast band from
///         the h-step VAR(1) forecast error covariance).
#[pyfunction]
#[pyo3(signature = (tenors, yields_matrix, horizon, lambda=0.7308))]
#[pyo3(text_signature = "(tenors, yields_matrix, horizon, lambda=0.7308)")]
fn diebold_li_forecast<'py>(
    py: Python<'py>,
    tenors: Vec<f64>,
    yields_matrix: Vec<Vec<f64>>,
    horizon: usize,
    lambda: f64,
) -> PyResult<Bound<'py, PyDict>> {
    let fc = py
        .detach(|| {
            let panel = YieldPanel::from_rows(tenors, yields_matrix, None)?;
            DieboldLi::builder()
                .lambda(lambda)
                .build()?
                .extract_factors(&panel)?
                .fit_var()?
                .forecast(horizon)
        })
        .map_err(core_to_py)?;

    let bands = PyDict::new(py);
    bands.set_item("lower_95", fc.lower_95.clone())?;
    bands.set_item("upper_95", fc.upper_95.clone())?;

    let d = PyDict::new(py);
    d.set_item("horizon", fc.horizon)?;
    d.set_item("tenors", fc.tenors.clone())?;
    d.set_item(
        "forecast_factors",
        vec![fc.factors[0], fc.factors[1], fc.factors[2]],
    )?;
    d.set_item("forecast_yields", fc.yields)?;
    d.set_item("confidence_bands", bands)?;
    Ok(d)
}

// ---------------------------------------------------------------------------
// PCA: fit
// ---------------------------------------------------------------------------

/// Fit PCA to a matrix of yield changes (first differences of a yield panel).
///
/// The input ``yield_changes`` is a (T-1) x N matrix of already-differenced
/// yields, typically obtained via ``numpy.diff(yields, axis=0)``.
///
/// Arguments:
///     yield_changes: T_changes x N matrix as list[list[float]].
///     n_components: Number of leading principal components to return (must be
///         in ``[1, N]``; default 3).
///
/// Returns a dict with keys:
///     loadings: list[list[float]] -- N x n_components loadings (column k is
///         the loading vector of principal component k).
///     scores: list[list[float]] -- T_changes x n_components scores.
///     eigenvalues: list[float] -- leading ``n_components`` eigenvalues.
///     explained_variance_ratio: list[float] -- fraction of total variance
///         explained by each of the leading ``n_components`` (length
///         ``n_components``).
///     cumulative_variance: list[float] -- cumulative variance ratio across
///         the leading ``n_components`` (length ``n_components``).
///     mean_change: list[float] -- column means subtracted before PCA (length N).
///     tenors_index_hint: int -- informational; always equals N.
#[pyfunction]
#[pyo3(signature = (yield_changes, n_components=3))]
#[pyo3(text_signature = "(yield_changes, n_components=3)")]
fn yield_pca_fit<'py>(
    py: Python<'py>,
    yield_changes: Vec<Vec<f64>>,
    n_components: usize,
) -> PyResult<Bound<'py, PyDict>> {
    let pca = py
        .detach(|| YieldPca::fit_yield_changes(yield_changes))
        .map_err(core_to_py)?;
    let n = pca.tenors().len();
    if n_components == 0 || n_components > pca.num_components() {
        return Err(crate::errors::value_error(format!(
            "n_components must be in [1, {}], got {n_components}",
            pca.num_components()
        )));
    }

    // Truncate to n_components.
    let loadings = pca.loadings();
    let scores = pca.scores();

    let mut loadings_out: Vec<Vec<f64>> = Vec::with_capacity(n);
    for row in 0..n {
        let mut r = Vec::with_capacity(n_components);
        for k in 0..n_components {
            r.push(loadings[(row, k)]);
        }
        loadings_out.push(r);
    }

    let t_changes = scores.nrows();
    let mut scores_out: Vec<Vec<f64>> = Vec::with_capacity(t_changes);
    for i in 0..t_changes {
        let mut r = Vec::with_capacity(n_components);
        for k in 0..n_components {
            r.push(scores[(i, k)]);
        }
        scores_out.push(r);
    }

    let eigenvalues: Vec<f64> = pca
        .eigenvalues()
        .iter()
        .take(n_components)
        .copied()
        .collect();
    let evr: Vec<f64> = pca
        .variance_explained()
        .iter()
        .take(n_components)
        .copied()
        .collect();
    let cum: Vec<f64> = pca
        .cumulative_variance()
        .iter()
        .take(n_components)
        .copied()
        .collect();
    let mean_change: Vec<f64> = pca.mean_change().iter().copied().collect();

    let d = PyDict::new(py);
    d.set_item("loadings", loadings_out)?;
    d.set_item("scores", scores_out)?;
    d.set_item("eigenvalues", eigenvalues)?;
    d.set_item("explained_variance_ratio", evr)?;
    d.set_item("cumulative_variance", cum)?;
    d.set_item("mean_change", mean_change)?;
    d.set_item("tenors_index_hint", n)?;
    Ok(d)
}

// ---------------------------------------------------------------------------
// PCA: scenario
// ---------------------------------------------------------------------------

/// Generate a single-component N-sigma PCA scenario shift to the yield curve.
///
/// Returns ``delta_yield = sigma_shock * sqrt(eigenvalue_k) * loading_k``,
/// i.e. the yield-change vector that corresponds to an ``sigma_shock``-sigma
/// move along principal component ``component_index``.
///
/// Arguments:
///     yield_changes: T_changes x N matrix of yield changes.
///     component_index: 0-based index of the principal component to shock
///         (must be in ``[0, n_components)``).
///     sigma_shock: Shock size in standard deviations (e.g. ``2.0`` for +2 sigma).
///     n_components: Number of PCs to fit (default 3); only used for bounds
///         checking on ``component_index``.
///
/// Returns a list[float] of length N (the yield-change vector).
#[pyfunction]
#[pyo3(signature = (yield_changes, component_index, sigma_shock, n_components=3))]
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

// ---------------------------------------------------------------------------
// Register
// ---------------------------------------------------------------------------

/// Build the `finstack_quant.core.market_data.dtsm` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "dtsm")?;
    m.setattr(
        "__doc__",
        "Dynamic term structure models: Diebold-Li dynamic Nelson-Siegel and yield-curve PCA.",
    )?;

    m.add_function(wrap_pyfunction!(diebold_li_fit_factors, &m)?)?;
    m.add_function(wrap_pyfunction!(diebold_li_forecast, &m)?)?;
    m.add_function(wrap_pyfunction!(yield_pca_fit, &m)?)?;
    m.add_function(wrap_pyfunction!(yield_pca_scenario, &m)?)?;

    let all = PyList::new(
        py,
        [
            "diebold_li_fit_factors",
            "diebold_li_forecast",
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
        "finstack_quant.core.market_data",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
