//! Python bindings for P&L attribution.
//!
//! Exposes the JSON-spec attribution pipeline and a `PnlAttribution` wrapper
//! for interactive exploration from Python.

use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::{display_to_py, serde_json_to_py};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

// ---------------------------------------------------------------------------
// Ergonomic entry point
// ---------------------------------------------------------------------------

/// Run P&L attribution for a single instrument and return JSON.
///
/// This is the main entry point. It accepts the instrument, two market
/// snapshots, valuation dates, and a method descriptor — all as simple
/// Python objects — and returns the canonical JSON form of the attribution.
/// Use ``PnlAttribution.from_json(...)`` when you want the richer Python wrapper.
///
/// Parameters
/// ----------
/// instrument_json : str
///     Tagged instrument JSON (``{"type": "bond", "spec": {...}}``).
/// market_t0_json : str
///     JSON-serialized ``MarketContext`` at T₀.
/// market_t1_json : str
///     JSON-serialized ``MarketContext`` at T₁.
/// as_of_t0 : str
///     Valuation date T₀ as an ISO 8601 calendar date (``YYYY-MM-DD``).
///     Time-of-day and timezone offsets are not accepted; for time-of-day
///     -sensitive workflows pass the start-of-day calendar date in UTC.
/// as_of_t1 : str
///     Valuation date T₁ as an ISO 8601 calendar date (``YYYY-MM-DD``).
/// method : str | dict
///     Attribution method. One of:
///
///     * ``"Parallel"``
///     * ``{"Waterfall": ["Carry", "RatesCurves", ...]}``
///     * ``"MetricsBased"``
///     * ``{"Taylor": {"include_gamma": true, ...}}``
/// config : dict, optional
///     Optional attribution config overrides (tolerance, metrics, bump sizes).
///
/// Returns
/// -------
/// str
///     Compact JSON ``PnlAttribution`` payload.
///
/// Examples
/// --------
/// >>> attr_json = attribute_pnl(inst, mkt_t0, mkt_t1, "2025-01-15", "2025-01-16", "Parallel")
/// >>> attr = PnlAttribution.from_json(attr_json)
/// >>> print(attr.explain())
/// >>> attr.to_dataframe()
#[pyfunction]
#[pyo3(signature = (instrument_json, market_t0_json, market_t1_json, as_of_t0, as_of_t1, method, config=None, full_cross_attribution=None))]
#[allow(clippy::too_many_arguments)]
fn attribute_pnl(
    py: Python<'_>,
    instrument_json: &str,
    market_t0_json: &str,
    market_t1_json: &str,
    as_of_t0: &str,
    as_of_t1: &str,
    method: &Bound<'_, PyAny>,
    config: Option<&Bound<'_, PyAny>>,
    full_cross_attribution: Option<bool>,
) -> PyResult<String> {
    let method_json = py_to_json_string(py, method, "method")?;
    let config_json = config
        .map(|value| py_to_json_string(py, value, "config"))
        .transpose()?;
    let mut spec = finstack_valuations::attribution::AttributionSpec::from_json_inputs(
        instrument_json,
        market_t0_json,
        market_t1_json,
        as_of_t0,
        as_of_t1,
        &method_json,
        config_json.as_deref(),
    )
    .map_err(display_to_py)?;

    if let Some(val) = full_cross_attribution {
        spec.full_cross_attribution = val;
    }

    // GIL is released for the entire attribution computation. The closure body
    // accesses no Python objects (`spec` is a fully-deserialized Rust value
    // built from `&str` arguments above), so concurrent Python callers can run
    // attributions in parallel without serializing on the GIL. Rayon
    // parallelism inside `spec.execute()` is unaffected.
    let result = py.detach(|| spec.execute()).map_err(display_to_py)?;
    serde_json::to_string(&result.attribution).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Raw JSON envelope entry point (power-user / round-trip)
// ---------------------------------------------------------------------------

/// Run attribution from a full JSON ``AttributionEnvelope`` and return JSON.
///
/// This is the raw JSON round-trip variant. Most users should prefer
/// :func:`attribute_pnl` which accepts separate arguments and returns
/// a ``PnlAttribution`` directly.
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-serialized ``AttributionEnvelope``.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``AttributionResultEnvelope``.
#[pyfunction]
fn attribute_pnl_from_spec(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    use finstack_valuations::attribution::AttributionEnvelope;

    let envelope: AttributionEnvelope = serde_json::from_str(spec_json).map_err(display_to_py)?;
    let result_envelope = py.detach(|| envelope.execute()).map_err(display_to_py)?;
    serde_json::to_string(&result_envelope).map_err(display_to_py)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Validate an attribution specification JSON.
///
/// Deserializes the input against the ``AttributionEnvelope`` schema and
/// returns the canonical (re-serialized) JSON.
///
/// Parameters
/// ----------
/// json : str
///     JSON-serialized ``AttributionEnvelope``.
///
/// Returns
/// -------
/// str
///     Canonical compact JSON.
#[pyfunction]
fn validate_attribution_json(json: &str) -> PyResult<String> {
    let envelope: finstack_valuations::attribution::AttributionEnvelope =
        serde_json::from_str(json).map_err(|e| serde_json_to_py(e, "invalid attribution JSON"))?;
    serde_json::to_string(&envelope).map_err(display_to_py)
}

/// Return the default waterfall factor ordering.
///
/// Returns
/// -------
/// list[str]
///     Factor names in the default waterfall order.
#[pyfunction]
fn default_waterfall_order() -> Vec<String> {
    finstack_valuations::attribution::default_waterfall_order()
        .into_iter()
        .map(|f| f.to_string())
        .collect()
}

/// Return the default metric IDs used by metrics-based attribution.
///
/// Returns
/// -------
/// list[str]
///     Metric identifier strings.
#[pyfunction]
fn default_attribution_metrics() -> Vec<String> {
    finstack_valuations::attribution::default_attribution_metrics()
        .into_iter()
        .map(|m| m.to_string())
        .collect()
}

/// Serialize a Python object to JSON via `json.dumps`.
fn py_to_json_string<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
    label: &str,
) -> PyResult<String> {
    let json_mod = py.import("json")?;
    json_mod
        .call_method1("dumps", (obj,))
        .and_then(|value| value.extract())
        .map_err(|e| PyValueError::new_err(format!("invalid {label}: {e}")))
}

// ---------------------------------------------------------------------------
// PnlAttribution wrapper
// ---------------------------------------------------------------------------

/// P&L attribution result for a single instrument.
///
/// Decomposes total P&L into constituent risk factors: carry, rates curves,
/// credit curves, inflation, correlations, FX, volatility, cross-factor
/// interactions, model parameters, market scalars, and residual.
///
/// Construct via :func:`attribute_pnl` or :meth:`from_json`.
#[pyclass(
    name = "PnlAttribution",
    module = "finstack.valuations",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPnlAttribution {
    pub(crate) inner: finstack_valuations::attribution::PnlAttribution,
}

#[pymethods]
impl PyPnlAttribution {
    /// Deserialize from JSON.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_valuations::attribution::PnlAttribution =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Export the canonical serde-shaped attribution payload as a Python dict.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let json = serde_json::to_string(&self.inner).map_err(display_to_py)?;
        let json_mod = py.import("json")?;
        json_mod.call_method1("loads", (json,))
    }

    // --- Aggregate P&L fields (amount as f64) ---

    /// Total P&L amount.
    #[getter]
    fn total_pnl(&self) -> f64 {
        self.inner.total_pnl.amount()
    }

    /// Carry (theta + accruals) P&L amount.
    #[getter]
    fn carry(&self) -> f64 {
        self.inner.carry.amount()
    }

    /// Interest rate curves P&L amount.
    #[getter]
    fn rates_curves_pnl(&self) -> f64 {
        self.inner.rates_curves_pnl.amount()
    }

    /// Credit hazard curves P&L amount.
    #[getter]
    fn credit_curves_pnl(&self) -> f64 {
        self.inner.credit_curves_pnl.amount()
    }

    /// Inflation curves P&L amount.
    #[getter]
    fn inflation_curves_pnl(&self) -> f64 {
        self.inner.inflation_curves_pnl.amount()
    }

    /// Base correlation curves P&L amount.
    #[getter]
    fn correlations_pnl(&self) -> f64 {
        self.inner.correlations_pnl.amount()
    }

    /// FX rate changes P&L amount.
    #[getter]
    fn fx_pnl(&self) -> f64 {
        self.inner.fx_pnl.amount()
    }

    /// Implied volatility changes P&L amount.
    #[getter]
    fn vol_pnl(&self) -> f64 {
        self.inner.vol_pnl.amount()
    }

    /// Cross-factor interaction P&L amount.
    #[getter]
    fn cross_factor_pnl(&self) -> f64 {
        self.inner.cross_factor_pnl.amount()
    }

    /// Model parameters P&L amount.
    #[getter]
    fn model_params_pnl(&self) -> f64 {
        self.inner.model_params_pnl.amount()
    }

    /// Market scalars P&L amount.
    #[getter]
    fn market_scalars_pnl(&self) -> f64 {
        self.inner.market_scalars_pnl.amount()
    }

    /// Residual (unexplained) P&L amount.
    #[getter]
    fn residual(&self) -> f64 {
        self.inner.residual.amount()
    }

    /// Currency code for all P&L amounts.
    #[getter]
    fn currency(&self) -> String {
        self.inner.total_pnl.currency().to_string()
    }

    // --- Metadata ---

    /// Instrument identifier.
    #[getter]
    fn instrument_id(&self) -> &str {
        &self.inner.meta.instrument_id
    }

    /// Attribution method name.
    #[getter]
    fn method(&self) -> String {
        self.inner.meta.method.to_string()
    }

    /// Start date (T₀) as ISO string.
    #[getter]
    fn t0(&self) -> String {
        self.inner.meta.t0.to_string()
    }

    /// End date (T₁) as ISO string.
    #[getter]
    fn t1(&self) -> String {
        self.inner.meta.t1.to_string()
    }

    /// Number of repricings performed.
    #[getter]
    fn num_repricings(&self) -> usize {
        self.inner.meta.num_repricings
    }

    /// Residual as percentage of total P&L.
    #[getter]
    fn residual_pct(&self) -> f64 {
        self.inner.meta.residual_pct
    }

    /// Diagnostic notes.
    #[getter]
    fn notes(&self) -> Vec<String> {
        self.inner.meta.notes.clone()
    }

    /// True if the attribution was flagged invalid (e.g. a non-finite factor
    /// sensitivity, or a residual that could not be computed). When ``True``,
    /// ``residual`` / ``residual_pct`` are not meaningful and the tolerance
    /// checks return ``False``.
    #[getter]
    fn result_invalid(&self) -> bool {
        self.inner.result_invalid
    }

    /// Check whether the residual is within tolerance.
    ///
    /// With no arguments this uses the attribution's own stored,
    /// method-appropriate tolerances — identical to
    /// :meth:`residual_within_meta_tolerance` and consistent with the native
    /// (Rust) check. Pass explicit values to override either threshold.
    ///
    /// Parameters
    /// ----------
    /// pct_tolerance : float, optional
    ///     Percentage tolerance (e.g. 0.1 for 0.1%). Defaults to the
    ///     attribution's stored ``meta.tolerance_pct``.
    /// abs_tolerance : float, optional
    ///     Absolute tolerance. Defaults to the attribution's stored
    ///     ``meta.tolerance_abs``.
    ///
    /// Returns
    /// -------
    /// bool
    #[pyo3(signature = (pct_tolerance=None, abs_tolerance=None))]
    fn residual_within_tolerance(
        &self,
        pct_tolerance: Option<f64>,
        abs_tolerance: Option<f64>,
    ) -> bool {
        self.inner.residual_within_tolerance(
            pct_tolerance.unwrap_or(self.inner.meta.tolerance_pct),
            abs_tolerance.unwrap_or(self.inner.meta.tolerance_abs),
        )
    }

    /// Check whether the residual is within the attribution's stored,
    /// method-appropriate tolerances (``meta.tolerance_pct`` /
    /// ``meta.tolerance_abs``).
    ///
    /// This matches the native ``residual_within_meta_tolerance`` check and is
    /// the recommended pass/fail gate — the per-method tolerances differ
    /// (waterfall is far tighter than metrics-based or Taylor).
    ///
    /// Returns
    /// -------
    /// bool
    fn residual_within_meta_tolerance(&self) -> bool {
        self.inner.residual_within_meta_tolerance()
    }

    /// Human-readable tree explanation (non-zero factors only).
    fn explain(&self) -> String {
        self.inner.explain()
    }

    /// Verbose tree explanation including zero-valued factors.
    fn explain_verbose(&self) -> String {
        self.inner.explain_verbose()
    }

    /// Export attribution as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``instrument_id``, ``method``, ``t0``, ``t1``, ``currency``,
    /// ``total_pnl``, ``carry``, ``rates_curves_pnl``, ``credit_curves_pnl``,
    /// ``inflation_curves_pnl``, ``correlations_pnl``, ``fx_pnl``, ``vol_pnl``,
    /// ``cross_factor_pnl``, ``model_params_pnl``, ``market_scalars_pnl``,
    /// ``residual``, ``residual_pct``, ``num_repricings``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("instrument_id", vec![&self.inner.meta.instrument_id])?;
        data.set_item("method", vec![self.inner.meta.method.to_string()])?;
        data.set_item("t0", vec![self.inner.meta.t0.to_string()])?;
        data.set_item("t1", vec![self.inner.meta.t1.to_string()])?;
        data.set_item(
            "currency",
            vec![self.inner.total_pnl.currency().to_string()],
        )?;
        data.set_item("total_pnl", vec![self.inner.total_pnl.amount()])?;
        data.set_item("carry", vec![self.inner.carry.amount()])?;
        data.set_item(
            "rates_curves_pnl",
            vec![self.inner.rates_curves_pnl.amount()],
        )?;
        data.set_item(
            "credit_curves_pnl",
            vec![self.inner.credit_curves_pnl.amount()],
        )?;
        data.set_item(
            "inflation_curves_pnl",
            vec![self.inner.inflation_curves_pnl.amount()],
        )?;
        data.set_item(
            "correlations_pnl",
            vec![self.inner.correlations_pnl.amount()],
        )?;
        data.set_item("fx_pnl", vec![self.inner.fx_pnl.amount()])?;
        data.set_item("vol_pnl", vec![self.inner.vol_pnl.amount()])?;
        data.set_item(
            "cross_factor_pnl",
            vec![self.inner.cross_factor_pnl.amount()],
        )?;
        data.set_item(
            "model_params_pnl",
            vec![self.inner.model_params_pnl.amount()],
        )?;
        data.set_item(
            "market_scalars_pnl",
            vec![self.inner.market_scalars_pnl.amount()],
        )?;
        data.set_item("residual", vec![self.inner.residual.amount()])?;
        data.set_item("residual_pct", vec![self.inner.meta.residual_pct])?;
        data.set_item("num_repricings", vec![self.inner.meta.num_repricings])?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "PnlAttribution(id={:?}, method={}, total_pnl={:.2} {}, residual_pct={:.2}%)",
            self.inner.meta.instrument_id,
            self.inner.meta.method,
            self.inner.total_pnl.amount(),
            self.inner.total_pnl.currency(),
            self.inner.meta.residual_pct,
        )
    }
}

/// Register attribution functions on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPnlAttribution>()?;
    m.add_function(pyo3::wrap_pyfunction!(attribute_pnl, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(attribute_pnl_from_spec, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validate_attribution_json, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(default_waterfall_order, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(default_attribution_metrics, m)?)?;
    Ok(())
}
