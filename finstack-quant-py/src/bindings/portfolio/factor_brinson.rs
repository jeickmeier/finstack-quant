//! Python bindings for Factor-Brinson unified attribution (Jeet & Partani 2023).
//!
//! Binds `finstack_quant_portfolio::factor_brinson_attribution`. The typed
//! entry point returns a `Py*` wrapper; the paired `*_json` function keeps
//! the exact JSON wire string. `factor_returns` is a plain numeric list
//! rather than embedded in the JSON payload because it is typically produced
//! by `finstack_quant.analytics.constrained_least_squares`.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::pandas_utils::{
    serde_rows_to_dataframe_with_schema, serde_to_py, ColumnSchema,
};
use crate::errors::{portfolio_to_py, serde_json_to_py};

/// Column schema for [`PyFactorBrinsonResult::to_dataframe`] (per-factor
/// contributions).
const FACTOR_CONTRIBUTION_COLUMNS: &[ColumnSchema<'static>] = &[
    ("factor", "str"),
    ("active_loading", "float64"),
    ("factor_return", "float64"),
    ("contribution", "float64"),
];

/// Column schema for
/// [`PyFactorBrinsonResult::to_asset_contributions_dataframe`].
const ASSET_CONTRIBUTION_COLUMNS: &[ColumnSchema<'static>] = &[
    ("asset", "str"),
    ("specific_return", "float64"),
    ("active_weight", "float64"),
    ("contribution", "float64"),
];

/// Factor-Brinson unified attribution result.
///
/// Returned by :func:`factor_brinson_attribution`.
#[pyclass(
    name = "FactorBrinsonResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFactorBrinsonResult {
    pub(crate) inner: finstack_quant_portfolio::FactorBrinsonResult,
}

crate::bindings::macros::wire_methods!(
    PyFactorBrinsonResult,
    finstack_quant_portfolio::FactorBrinsonResult,
    "FactorBrinsonResult"
);

#[pymethods]
impl PyFactorBrinsonResult {
    /// Portfolio total return.
    #[getter]
    fn portfolio_return(&self) -> f64 {
        self.inner.portfolio_return
    }

    /// Benchmark total return.
    #[getter]
    fn benchmark_return(&self) -> f64 {
        self.inner.benchmark_return
    }

    /// Active return, ``portfolio_return - benchmark_return``.
    #[getter]
    fn active_return(&self) -> f64 {
        self.inner.active_return
    }

    /// Factor (allocation) contribution.
    #[getter]
    fn allocation(&self) -> f64 {
        self.inner.allocation
    }

    /// Specific (selection) contribution.
    #[getter]
    fn selection(&self) -> f64 {
        self.inner.selection
    }

    /// Per-factor breakdown of ``allocation`` as a list of dicts, in
    /// ``factor_names`` order.
    #[getter]
    fn factor_contributions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.factor_contributions)
    }

    /// Per-asset breakdown of ``selection`` as a list of dicts, in
    /// ``asset_ids`` order.
    #[getter]
    fn asset_contributions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.asset_contributions)
    }

    /// Per-factor contributions as a :class:`pandas.DataFrame`.
    ///
    /// The primary frame is the factor axis; the per-asset selection
    /// breakdown is available from :meth:`to_asset_contributions_dataframe`.
    ///
    /// Columns: ``factor``, ``active_loading``, ``factor_return``,
    /// ``contribution``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(
            py,
            &self.inner.factor_contributions,
            FACTOR_CONTRIBUTION_COLUMNS,
        )
    }

    /// Per-asset specific contributions as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``asset``, ``specific_return``, ``active_weight``,
    /// ``contribution``.
    fn to_asset_contributions_dataframe<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(
            py,
            &self.inner.asset_contributions,
            ASSET_CONTRIBUTION_COLUMNS,
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorBrinsonResult(factors={}, assets={}, active_return={})",
            self.inner.factor_contributions.len(),
            self.inner.asset_contributions.len(),
            self.inner.active_return,
        )
    }
}

/// Parse the payload and run the canonical factor-Brinson attribution.
fn run_factor_brinson_attribution(
    py: Python<'_>,
    input_json: &str,
    factor_returns: Vec<f64>,
) -> PyResult<finstack_quant_portfolio::FactorBrinsonResult> {
    let input_json = input_json.to_owned();
    py.detach(move || {
        let input: finstack_quant_portfolio::FactorBrinsonInput = serde_json::from_str(&input_json)
            .map_err(|err| serde_json_to_py(err, "invalid factor-Brinson input JSON"))?;
        finstack_quant_portfolio::factor_brinson_attribution(&input, &factor_returns)
            .map_err(portfolio_to_py)
    })
}

/// Compute Jeet-Partani (2023) factor-Brinson unified attribution.
///
/// Binds Rust `finstack_quant_portfolio::factor_brinson_attribution`:
/// generalizes classical Brinson-Fachler allocation/selection to continuous
/// factor exposures by replacing the sector partition with a factor-exposure
/// matrix and a caller-supplied benchmark factor-return vector.
///
/// Parameters
/// ----------
/// input_json : str | dict | list | pandas.DataFrame
///     JSON ``FactorBrinsonInput`` with ``asset_ids``, ``asset_returns``,
///     ``exposures`` (row-major ``n_assets x n_factors``), ``factor_names``,
///     ``portfolio_weights`` and ``benchmark_weights``. Each weight vector
///     must sum to ``1.0`` within ``1e-6``; weights may be negative (short
///     positions).
/// factor_returns : list[float]
///     Caller-supplied benchmark factor returns ``f_b``, length
///     ``input.factor_names``. Typically fit with
///     :func:`finstack_quant.analytics.constrained_least_squares` (using
///     benchmark weights) so the completeness condition below holds.
///
/// Returns
/// -------
/// FactorBrinsonResult
///     Typed result with ``allocation``, ``selection``, and their
///     per-factor / per-asset breakdowns. Use
///     :meth:`to_json` for the raw wire string.
///
/// Raises
/// ------
/// PortfolioError
///     If any array has the wrong length relative to ``n_assets`` /
///     ``n_factors``, either ``n_assets`` or ``n_factors`` is zero, any
///     input value is non-finite, portfolio or benchmark weights don't sum
///     to ``1.0`` within tolerance, or the Jeet-Partani completeness
///     residual ``|h_b'eps_b|`` exceeds tolerance — meaning the supplied
///     ``factor_returns`` do not fully explain the benchmark return, so
///     ``allocation``/``selection`` would not be valid Brinson effects. The
///     error directs callers to
///     :func:`finstack_quant.analytics.constrained_least_squares`.
/// ValueError
///     If ``input_json`` is malformed or carries unknown fields.
///
/// Sources
/// -------
/// See ``docs/REFERENCES.md#jeet-partani-2023``.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.portfolio import factor_brinson_attribution
/// >>> inputs = {
/// ...     "asset_ids": ["A"],
/// ...     "asset_returns": [0.02],
/// ...     "exposures": [1.0],
/// ...     "factor_names": ["Market"],
/// ...     "portfolio_weights": [1.0],
/// ...     "benchmark_weights": [1.0],
/// ... }
/// >>> result = factor_brinson_attribution(json.dumps(inputs), [0.02])
/// >>> result.active_return
/// 0.0
#[pyfunction]
#[pyo3(text_signature = "(input_json, factor_returns)")]
fn factor_brinson_attribution(
    py: Python<'_>,
    input_json: &Bound<'_, PyAny>,
    factor_returns: Vec<f64>,
) -> PyResult<PyFactorBrinsonResult> {
    let input_json = crate::bindings::extract::extract_records_json(py, input_json, "input")?;
    let input_json: &str = &input_json;
    Ok(PyFactorBrinsonResult {
        inner: run_factor_brinson_attribution(py, input_json, factor_returns)?,
    })
}
/// Register factor-Brinson attribution functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFactorBrinsonResult>()?;
    m.add_function(wrap_pyfunction!(factor_brinson_attribution, m)?)?;
    Ok(())
}
