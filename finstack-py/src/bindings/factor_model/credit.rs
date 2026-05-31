//! Python bindings for the credit factor hierarchy.
//!
//! Exposes [`PyCreditFactorModel`], [`PyCreditCalibrator`], the free functions
//! [`decompose_levels`] and [`decompose_period`], and
//! [`PyFactorCovarianceForecast`] which wraps the vol-forecast engine from
//! `finstack-portfolio`.

use crate::bindings::date_utils::parse_iso_date_py as parse_date;
use crate::errors::display_to_py;
use pyo3::prelude::*;
use pyo3::types::PyDict;

// ---------------------------------------------------------------------------
// PyCreditFactorModel
// ---------------------------------------------------------------------------

/// Calibrated credit factor hierarchy artifact.
///
/// Produced by :class:`CreditCalibrator` or loaded from JSON via
/// :meth:`from_json`.  All fields are read-only; mutations require
/// re-calibrating.
///
/// The canonical round-trip is::
///
///     model = CreditFactorModel.from_json(json_str)
///     json_out = model.to_json()
///
/// Example:
///     >>> from finstack.factor_model.credit import CreditFactorModel
///     >>> model = CreditFactorModel.from_json(json_str)  # doctest: +SKIP
///     >>> model.schema_version  # doctest: +SKIP
///     'finstack.credit_factor_model/1'
#[pyclass(
    name = "CreditFactorModel",
    module = "finstack.factor_model.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyCreditFactorModel {
    pub(crate) inner: finstack_factor_model::credit::hierarchy::CreditFactorModel,
}

impl PyCreditFactorModel {
    pub(crate) fn from_inner(
        inner: finstack_factor_model::credit::hierarchy::CreditFactorModel,
    ) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCreditFactorModel {
    /// Deserialize a :class:`CreditFactorModel` from JSON.
    ///
    /// Validates the ``schema_version`` field and all structural constraints.
    ///
    /// Args:
    ///     json: JSON string produced by :meth:`to_json` or the offline calibrator.
    ///
    /// Returns:
    ///     Parsed :class:`CreditFactorModel` instance.
    ///
    /// Raises:
    ///     ValueError: If the JSON is malformed or fails validation.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_factor_model::credit::hierarchy::CreditFactorModel =
            serde_json::from_str(json).map_err(display_to_py)?;
        inner.validate().map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize this model to pretty-printed JSON.
    ///
    /// Returns:
    ///     JSON string suitable for storage or transmission.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    /// Schema version string (``"finstack.credit_factor_model/1"``).
    #[getter]
    fn schema_version(&self) -> &str {
        &self.inner.schema_version
    }

    /// Calibration anchor date (ISO 8601 string).
    #[getter]
    fn as_of(&self) -> String {
        self.inner.as_of.to_string()
    }

    /// Number of hierarchy levels (broadest → narrowest).
    #[getter]
    fn n_levels(&self) -> usize {
        self.inner.hierarchy.levels.len()
    }

    /// Number of issuer beta rows in the artifact.
    #[getter]
    fn n_issuers(&self) -> usize {
        self.inner.issuer_betas.len()
    }

    /// Number of factors in the model configuration.
    #[getter]
    fn n_factors(&self) -> usize {
        self.inner.config.factors.len()
    }

    /// Hierarchy level names as a list of strings.
    ///
    /// Returns:
    ///     List of dimension names (e.g. ``["Rating", "Region", "Sector"]``).
    fn level_names(&self) -> Vec<String> {
        use finstack_factor_model::credit::hierarchy::HierarchyDimension;
        self.inner
            .hierarchy
            .levels
            .iter()
            .map(|d| match d {
                HierarchyDimension::Rating => "Rating".to_owned(),
                HierarchyDimension::Region => "Region".to_owned(),
                HierarchyDimension::Sector => "Sector".to_owned(),
                HierarchyDimension::Custom(name) => name.clone(),
            })
            .collect()
    }

    /// Issuer IDs present in the artifact.
    ///
    /// Returns:
    ///     List of issuer ID strings.
    fn issuer_ids(&self) -> Vec<String> {
        self.inner
            .issuer_betas
            .iter()
            .map(|row| row.issuer_id.as_str().to_owned())
            .collect()
    }

    /// Factor IDs in the model configuration.
    ///
    /// Returns:
    ///     List of factor ID strings.
    fn factor_ids(&self) -> Vec<String> {
        self.inner
            .config
            .factors
            .iter()
            .map(|f| f.id.to_string())
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "CreditFactorModel(as_of={:?}, n_levels={}, n_issuers={}, n_factors={})",
            self.inner.as_of,
            self.inner.hierarchy.levels.len(),
            self.inner.issuer_betas.len(),
            self.inner.config.factors.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// PyCreditCalibrator
// ---------------------------------------------------------------------------

/// Deterministic calibrator that produces a :class:`CreditFactorModel`.
///
/// Configuration and inputs are passed as JSON strings so that Python callers
/// can work with plain dicts (serialized via ``json.dumps``) without needing
/// typed wrappers for every sub-field.
///
/// Example:
///     >>> import json
///     >>> from finstack.factor_model.credit import CreditCalibrator
///     >>> cal = CreditCalibrator(json.dumps(config))
///     >>> model = cal.calibrate(json.dumps(inputs))  # doctest: +SKIP
#[pyclass(
    name = "CreditCalibrator",
    module = "finstack.factor_model.credit",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyCreditCalibrator {
    inner: finstack_factor_model::CreditCalibrator,
}

#[pymethods]
impl PyCreditCalibrator {
    /// Construct a calibrator from a JSON-serialized ``CreditCalibrationConfig``.
    ///
    /// Args:
    ///     config_json: JSON string of a ``CreditCalibrationConfig``.
    ///
    /// Raises:
    ///     ValueError: If ``config_json`` is not a valid ``CreditCalibrationConfig``.
    #[new]
    fn new(config_json: &str) -> PyResult<Self> {
        let config: finstack_factor_model::CreditCalibrationConfig =
            serde_json::from_str(config_json).map_err(display_to_py)?;
        Ok(Self {
            inner: finstack_factor_model::CreditCalibrator::new(config),
        })
    }

    /// Run the full calibration pipeline and return a :class:`CreditFactorModel`.
    ///
    /// Args:
    ///     inputs_json: JSON string of a ``CreditCalibrationInputs`` object
    ///         containing the history panel, issuer tags, generic factor
    ///         series, and anchor date.
    ///
    /// Returns:
    ///     Calibrated :class:`CreditFactorModel` artifact.
    ///
    /// Raises:
    ///     ValueError: If inputs are structurally invalid or calibration fails.
    fn calibrate(&self, inputs_json: &str) -> PyResult<PyCreditFactorModel> {
        let inputs: finstack_factor_model::CreditCalibrationInputs =
            serde_json::from_str(inputs_json).map_err(display_to_py)?;
        let model = self.inner.calibrate(inputs).map_err(display_to_py)?;
        Ok(PyCreditFactorModel::from_inner(model))
    }

    fn __repr__(&self) -> String {
        "CreditCalibrator(...)".to_owned()
    }
}

// ---------------------------------------------------------------------------
// PyLevelsAtDate
// ---------------------------------------------------------------------------

/// Snapshot of all hierarchy-level factor values at a single date.
///
/// Produced by :func:`decompose_levels`.  Carry this into
/// :func:`decompose_period` to compute period-over-period changes.
///
/// Example:
///     >>> from finstack.factor_model.credit import decompose_levels
///     >>> snap = decompose_levels(model, spreads, generic, "2024-03-29")  # doctest: +SKIP
///     >>> snap.generic  # doctest: +SKIP
///     100.5
#[pyclass(
    name = "LevelsAtDate",
    module = "finstack.factor_model.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyLevelsAtDate {
    inner: finstack_factor_model::LevelsAtDate,
}

impl PyLevelsAtDate {
    fn from_inner(inner: finstack_factor_model::LevelsAtDate) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyLevelsAtDate {
    /// Observation date (ISO 8601 string).
    #[getter]
    fn date(&self) -> String {
        self.inner.date.to_string()
    }

    /// Generic (PC) factor value at this date.
    #[getter]
    fn generic(&self) -> f64 {
        self.inner.generic
    }

    /// Number of hierarchy levels.
    #[getter]
    fn n_levels(&self) -> usize {
        self.inner.by_level.len()
    }

    /// Bucket values for a given level index as a dict ``{bucket_path: value}``.
    ///
    /// Args:
    ///     level_index: Zero-based hierarchy level index.
    ///
    /// Returns:
    ///     Dict mapping bucket path string to factor value.
    ///
    /// Raises:
    ///     ValueError: If ``level_index`` is out of range.
    fn level_values<'py>(
        &self,
        py: Python<'py>,
        level_index: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let lev = self.inner.by_level.get(level_index).ok_or_else(|| {
            crate::errors::value_error(format!(
                "level_index {} out of range (n_levels={})",
                level_index,
                self.inner.by_level.len()
            ))
        })?;
        let d = PyDict::new(py);
        for (k, v) in &lev.values {
            d.set_item(k, v)?;
        }
        Ok(d)
    }

    /// Per-issuer residual adder after peeling all levels, as a dict.
    ///
    /// Returns:
    ///     Dict mapping issuer ID to adder value.
    fn adder<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        for (issuer, val) in &self.inner.adder {
            d.set_item(issuer.as_str(), val)?;
        }
        Ok(d)
    }

    fn __repr__(&self) -> String {
        format!(
            "LevelsAtDate(date={:?}, generic={:.4}, n_levels={}, n_issuers={})",
            self.inner.date,
            self.inner.generic,
            self.inner.by_level.len(),
            self.inner.adder.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// PyPeriodDecomposition
// ---------------------------------------------------------------------------

/// Component-wise difference between two :class:`LevelsAtDate` snapshots.
///
/// Produced by :func:`decompose_period`.  Satisfies the linear
/// reconciliation invariant
///
/// .. code-block:: text
///
///     ΔS_i ≡ β_i^PC · Δgeneric
///            + Σ_k β_i^level_k · ΔL_level_k(g_i^k)
///            + Δadder_i
///
/// for every issuer present in both snapshots.
///
/// Example:
///     >>> from finstack.factor_model.credit import decompose_period
///     >>> period = decompose_period(snap_t0, snap_t1)  # doctest: +SKIP
///     >>> period.d_generic  # doctest: +SKIP
///     0.3
#[pyclass(
    name = "PeriodDecomposition",
    module = "finstack.factor_model.credit",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPeriodDecomposition {
    inner: finstack_factor_model::PeriodDecomposition,
}

impl PyPeriodDecomposition {
    fn from_inner(inner: finstack_factor_model::PeriodDecomposition) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPeriodDecomposition {
    /// Earlier snapshot date (ISO 8601).
    #[getter(from_date)]
    fn get_from_date(&self) -> String {
        self.inner.from.to_string()
    }

    /// Later snapshot date (ISO 8601).
    #[getter]
    fn to_date(&self) -> String {
        self.inner.to.to_string()
    }

    /// Change in the generic (PC) factor value.
    #[getter]
    fn d_generic(&self) -> f64 {
        self.inner.d_generic
    }

    /// Number of hierarchy levels.
    #[getter]
    fn n_levels(&self) -> usize {
        self.inner.by_level.len()
    }

    /// Bucket value deltas for a given level index as a dict.
    ///
    /// Args:
    ///     level_index: Zero-based hierarchy level index.
    ///
    /// Returns:
    ///     Dict mapping bucket path string to delta value.
    ///
    /// Raises:
    ///     ValueError: If ``level_index`` is out of range.
    fn level_deltas<'py>(
        &self,
        py: Python<'py>,
        level_index: usize,
    ) -> PyResult<Bound<'py, PyDict>> {
        let lev = self.inner.by_level.get(level_index).ok_or_else(|| {
            crate::errors::value_error(format!(
                "level_index {} out of range (n_levels={})",
                level_index,
                self.inner.by_level.len()
            ))
        })?;
        let d = PyDict::new(py);
        for (k, v) in &lev.deltas {
            d.set_item(k, v)?;
        }
        Ok(d)
    }

    /// Per-issuer adder deltas as a dict.
    ///
    /// Returns:
    ///     Dict mapping issuer ID to adder change.
    fn d_adder<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let d = PyDict::new(py);
        for (issuer, val) in &self.inner.d_adder {
            d.set_item(issuer.as_str(), val)?;
        }
        Ok(d)
    }

    fn __repr__(&self) -> String {
        format!(
            "PeriodDecomposition(from={:?}, to={:?}, d_generic={:.4}, n_levels={})",
            self.inner.from,
            self.inner.to,
            self.inner.d_generic,
            self.inner.by_level.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// decompose_levels  (free function)
// ---------------------------------------------------------------------------

/// Decompose observed issuer spreads at a point in time into per-level factor
/// values and per-issuer residual adders.
///
/// Args:
///     model: Calibrated :class:`CreditFactorModel` artifact.
///     observed_spreads: Dict mapping issuer ID string to observed spread (float).
///     observed_generic: Generic (PC) factor value at ``as_of``.
///     as_of: Valuation date in ISO 8601 format.
///     runtime_tags: Optional dict of ``{issuer_id: {dim_key: tag_value}}`` for
///         issuers not present in the model.
///
/// Returns:
///     :class:`LevelsAtDate` snapshot with generic value, per-level bucket values,
///     and per-issuer residual adders.
///
/// Raises:
///     ValueError: If an issuer has no model row and no ``runtime_tags`` entry,
///         or if an issuer is missing a required hierarchy tag.
///
/// Example:
///     >>> from finstack.factor_model.credit import decompose_levels
///     >>> snap = decompose_levels(model, {"ISSUER-A": 120.5}, 100.0, "2024-03-29")  # doctest: +SKIP
#[pyfunction]
#[pyo3(signature = (model, observed_spreads_json, observed_generic, as_of, runtime_tags_json=None))]
fn decompose_levels(
    model: &PyCreditFactorModel,
    observed_spreads_json: &str,
    observed_generic: f64,
    as_of: &str,
    runtime_tags_json: Option<&str>,
) -> PyResult<PyLevelsAtDate> {
    let observed_spreads: std::collections::BTreeMap<finstack_core::types::IssuerId, f64> =
        serde_json::from_str(observed_spreads_json).map_err(display_to_py)?;

    let date = parse_date(as_of)?;

    let runtime_tags: Option<
        std::collections::BTreeMap<
            finstack_core::types::IssuerId,
            finstack_factor_model::credit::hierarchy::IssuerTags,
        >,
    > = match runtime_tags_json {
        Some(json) => Some(serde_json::from_str(json).map_err(display_to_py)?),
        None => None,
    };

    let result = finstack_factor_model::decompose_levels(
        &model.inner,
        &observed_spreads,
        observed_generic,
        date,
        runtime_tags.as_ref(),
    )
    .map_err(display_to_py)?;

    Ok(PyLevelsAtDate::from_inner(result))
}

// ---------------------------------------------------------------------------
// decompose_period  (free function)
// ---------------------------------------------------------------------------

/// Difference two :class:`LevelsAtDate` snapshots component-wise.
///
/// Output buckets and issuers are restricted to those present in **both**
/// snapshots so the linear reconciliation invariant on ``ΔS_i`` holds for
/// every entry.
///
/// Args:
///     from_levels: Earlier :class:`LevelsAtDate` snapshot.
///     to_levels: Later :class:`LevelsAtDate` snapshot.
///
/// Returns:
///     :class:`PeriodDecomposition` with ``d_generic``, per-level bucket deltas,
///     and per-issuer adder deltas.
///
/// Raises:
///     ValueError: If ``from_levels.date > to_levels.date`` or the two
///         snapshots disagree on hierarchy depth.
///
/// Example:
///     >>> from finstack.factor_model.credit import decompose_period
///     >>> period = decompose_period(snap_t0, snap_t1)  # doctest: +SKIP
///     >>> period.d_generic  # doctest: +SKIP
///     0.3
#[pyfunction]
fn decompose_period(
    from_levels: &PyLevelsAtDate,
    to_levels: &PyLevelsAtDate,
) -> PyResult<PyPeriodDecomposition> {
    let result = finstack_factor_model::decompose_period(&from_levels.inner, &to_levels.inner)
        .map_err(display_to_py)?;
    Ok(PyPeriodDecomposition::from_inner(result))
}

// ---------------------------------------------------------------------------
// PyFactorCovarianceForecast
// ---------------------------------------------------------------------------

/// Vol-forecast view over a calibrated :class:`CreditFactorModel`.
///
/// The forecaster is a thin wrapper; all business logic stays in Rust.
/// ``VolHorizon::Custom`` is intentionally **not** exposed (closures don't
/// cross the FFI boundary cleanly).
///
/// Horizon strings accepted by :meth:`covariance_at` and
/// :meth:`idiosyncratic_vol`:
///
/// - ``"one_step"`` — calibrated annualized variance unchanged.
/// - ``"unconditional"`` — long-run (identical to ``"one_step"`` for
///   ``Sample`` vol model).
/// - ``{"n_steps": N}`` (JSON string) — variance scaled by ``N``.
///
/// Example:
///     >>> from finstack.factor_model.credit import FactorCovarianceForecast
///     >>> fcf = FactorCovarianceForecast(model)
///     >>> cov_json = fcf.covariance_at("one_step")  # doctest: +SKIP
#[pyclass(
    name = "FactorCovarianceForecast",
    module = "finstack.factor_model.credit",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyFactorCovarianceForecast {
    /// We store the model by value (cloned from the Python wrapper) so that
    /// `FactorCovarianceForecast<'a>` lifetime requirements don't escape.
    model: finstack_factor_model::credit::hierarchy::CreditFactorModel,
}

/// Parse a horizon descriptor from a Python string.
///
/// Accepted forms:
/// - `"one_step"` → `VolHorizon::OneStep`
/// - `"unconditional"` → `VolHorizon::Unconditional`
/// - JSON string `'{"n_steps": 5}'` → `VolHorizon::NSteps(5)`
///
/// Delegates to the canonical [`VolHorizon::parse`] implementation in
/// `finstack-portfolio`; this wrapper only maps the error to `PyValueError`.
fn parse_vol_horizon(s: &str) -> PyResult<finstack_portfolio::factor_model::VolHorizon> {
    finstack_portfolio::factor_model::VolHorizon::parse(s).map_err(crate::errors::value_error)
}

#[pymethods]
impl PyFactorCovarianceForecast {
    /// Wrap a :class:`CreditFactorModel` for vol forecasting.
    ///
    /// Args:
    ///     model: Calibrated :class:`CreditFactorModel` artifact.
    #[new]
    fn new(model: &PyCreditFactorModel) -> Self {
        Self {
            model: model.inner.clone(),
        }
    }

    /// Build the factor covariance matrix ``Σ(t, h) = D · ρ_static · D``.
    ///
    /// Args:
    ///     horizon: Horizon descriptor string — ``"one_step"``,
    ///         ``"unconditional"``, or a JSON string ``'{"n_steps": N}'``.
    ///
    /// Returns:
    ///     Pretty-printed JSON of a ``FactorCovarianceMatrix``.
    ///
    /// Raises:
    ///     ValueError: If the horizon string is invalid or the model data is
    ///         inconsistent (mismatched axes, negative variance).
    fn covariance_at(&self, horizon: &str) -> PyResult<String> {
        let h = parse_vol_horizon(horizon)?;
        let forecast = finstack_portfolio::factor_model::FactorCovarianceForecast::new(&self.model);
        let cov = forecast.covariance_at(h).map_err(display_to_py)?;
        serde_json::to_string_pretty(&cov).map_err(display_to_py)
    }

    /// Idiosyncratic vol (std dev) for a specific issuer at the requested horizon.
    ///
    /// Args:
    ///     issuer_id: Issuer identifier string.
    ///     horizon: Horizon descriptor (same vocabulary as :meth:`covariance_at`).
    ///
    /// Returns:
    ///     Idiosyncratic standard deviation (annualized).
    ///
    /// Raises:
    ///     ValueError: If the issuer is not present in the model's vol state or
    ///         the calibrated variance is negative.
    fn idiosyncratic_vol(&self, issuer_id: &str, horizon: &str) -> PyResult<f64> {
        let h = parse_vol_horizon(horizon)?;
        let id = finstack_core::types::IssuerId::new(issuer_id);
        let forecast = finstack_portfolio::factor_model::FactorCovarianceForecast::new(&self.model);
        forecast.idiosyncratic_vol(&id, h).map_err(display_to_py)
    }

    /// Build a portfolio-level ``FactorModel`` JSON using ``Σ(t, h)`` at the
    /// given horizon and risk measure.
    ///
    /// The returned JSON can be passed to the portfolio risk decomposition
    /// pipeline.
    ///
    /// Args:
    ///     horizon: Horizon descriptor (same vocabulary as :meth:`covariance_at`).
    ///     risk_measure_json: Risk measure — ``"variance"``, ``"volatility"``,
    ///         or a JSON string (e.g. ``'{"var": {"confidence": 0.99}}'``).
    ///
    /// Returns:
    ///     Pretty-printed JSON of the assembled :class:`FactorModel` configuration.
    ///
    /// Raises:
    ///     ValueError: If the horizon or risk measure is invalid, or the model
    ///         builder rejects the assembled configuration.
    fn factor_model_at(&self, horizon: &str, risk_measure_json: &str) -> PyResult<String> {
        let h = parse_vol_horizon(horizon)?;
        let measure: finstack_factor_model::RiskMeasure =
            serde_json::from_str(risk_measure_json).map_err(display_to_py)?;
        let forecast = finstack_portfolio::factor_model::FactorCovarianceForecast::new(&self.model);
        let config = forecast
            .factor_model_config_at(h, measure)
            .map_err(display_to_py)?;
        serde_json::to_string_pretty(&config).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorCovarianceForecast(as_of={:?}, n_factors={})",
            self.model.as_of,
            self.model.config.factors.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCreditFactorModel>()?;
    m.add_class::<PyCreditCalibrator>()?;
    m.add_class::<PyLevelsAtDate>()?;
    m.add_class::<PyPeriodDecomposition>()?;
    m.add_class::<PyFactorCovarianceForecast>()?;
    m.add_function(pyo3::wrap_pyfunction!(decompose_levels, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(decompose_period, m)?)?;
    Ok(())
}
