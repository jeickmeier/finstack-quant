//! P&L attribution entry points and JSON helpers.

use crate::bindings::module_utils::py_to_json_string;
use crate::errors::{display_to_py, serde_json_to_py};
use pyo3::prelude::*;

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
pub(crate) fn attribute_pnl(
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
    let mut spec = finstack_attribution::AttributionSpec::from_json_inputs(
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
pub(crate) fn attribute_pnl_from_spec(py: Python<'_>, spec_json: &str) -> PyResult<String> {
    use finstack_attribution::AttributionEnvelope;

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
pub(crate) fn validate_attribution_json(json: &str) -> PyResult<String> {
    let envelope: finstack_attribution::AttributionEnvelope =
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
pub(crate) fn default_waterfall_order() -> Vec<String> {
    finstack_attribution::default_waterfall_order()
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
pub(crate) fn default_attribution_metrics() -> Vec<String> {
    finstack_attribution::default_attribution_metrics()
        .into_iter()
        .map(|m| m.to_string())
        .collect()
}
