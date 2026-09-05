//! Python bindings for horizon total return analysis.

use crate::bindings::attribution::PyPnlAttribution;
use crate::bindings::extract::{extract_instrument_json, extract_market};
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::{core_to_py, display_to_py, scenarios_to_py};
use pyo3::prelude::*;

use super::engine::PyApplicationReport;
use super::extract::{extract_config, extract_scenario_spec, recalibration_provider};

/// Compute horizon total return under a scenario.
///
/// Applies a scenario specification (which may include time-roll and market
/// shocks) to project an instrument forward, then decomposes the resulting
/// P&L using factor-based attribution.
///
/// Parameters
/// ----------
/// instrument : Instrument | str
///     Typed instrument (``Bond``, ``CreditDefaultSwap``, ``InterestRateSwap``,
///     ...) or a canonical v1 instrument envelope JSON string.
/// market : MarketContext | str
///     A ``MarketContext`` object or JSON string. Never mutated; the scenario
///     is applied to an internal copy.
/// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date (ISO 8601 accepted, e.g. ``"2025-01-15"``).
/// scenario : ScenarioSpec | str
///     Typed scenario or JSON-serialized ``ScenarioSpec``.
/// method : str, default "parallel"
///     Attribution method: ``"parallel"``, ``"waterfall"``,
///     ``"metrics_based"``, or ``"taylor"``. ``"metrics_based"`` re-prices
///     the instrument with the default attribution metric set (DV01, CS01,
///     vega, ...) under the same configuration and recalibration provider
///     the scenario engine uses; instruments lacking a metric raise
///     ``RuntimeError`` rather than silently dropping the factor.
/// config : FinstackConfig | str | None, default None
///     Library configuration (rounding, tolerances, bump sizes) threaded
///     into both the scenario engine and the attribution pricing.
/// calendar_id : str | None, default None
///     Holiday calendar used to business-day adjust ``time_roll_forward``
///     targets under ``TimeRollMode.business_days`` (e.g. ``"nyse"``,
///     ``"target"``). ``None`` uses a weekends-only calendar.
///
/// Returns
/// -------
/// HorizonResult
///     Decomposed total return with factor attribution and the scenario
///     ``ApplicationReport``.
///
/// Raises
/// ------
/// ValueError
///     If an input fails to parse or validate, ``method`` is unknown,
///     ``calendar_id`` is not a built-in calendar, or the scenario contains
///     an instrument-scoped operation (horizon analysis prices one instrument
///     instance at both dates).
/// KeyError
///     If the scenario references market data or tenors that do not exist.
/// RuntimeError
///     If pricing or attribution fails.
///
/// Notes
/// -----
/// ``total_return`` is a decimal fraction (``0.05`` = +5%) and is ``nan``
/// when the initial value and total P&L are denominated in different
/// currencies (no implicit FX conversion); ``annualized_return`` is ``None``
/// in that case. The GIL is released while the scenario and attribution
/// computations run.
#[pyfunction]
#[pyo3(signature = (instrument, market, as_of, scenario, method = "parallel", config = None, calendar_id = None))]
// Arity is fixed by the documented Python keyword signature; grouping into a
// params struct would change the public API.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_horizon_return<'py>(
    py: Python<'py>,
    instrument: &Bound<'py, PyAny>,
    market: &Bound<'py, PyAny>,
    as_of: &Bound<'py, PyAny>,
    scenario: &Bound<'py, PyAny>,
    method: &str,
    config: Option<&Bound<'py, PyAny>>,
    calendar_id: Option<&str>,
) -> PyResult<PyHorizonResult> {
    use finstack_quant_valuations::instruments::InstrumentEnvelope;
    use std::sync::Arc;

    let instrument_json = extract_instrument_json(instrument)?;
    let boxed = InstrumentEnvelope::from_str(&instrument_json).map_err(core_to_py)?;
    let instrument: Arc<dyn finstack_quant_valuations::instruments::Instrument> = Arc::from(boxed);

    // Owned copy so the compute can run without the GIL.
    let market_ctx = extract_market(py, market)?;
    let date = crate::bindings::date_utils::extract_date(as_of)?;
    let scenario = extract_scenario_spec(scenario)?;
    let attribution_method = finstack_quant_scenarios::horizon::attribution_method_from_str(method)
        .map_err(scenarios_to_py)?;
    let finstack_config = extract_config(config)?;

    // Run analysis with the GIL released: horizon attribution revalues the
    // instrument multiple times (potentially rayon-parallel) and can run for
    // seconds on large books.
    let mut analyzer = finstack_quant_scenarios::horizon::HorizonAnalysis::new(
        attribution_method,
        finstack_config,
    )
    .with_recalibration_provider(recalibration_provider());
    if let Some(id) = calendar_id {
        analyzer = analyzer.with_calendar_id(id);
    }
    let result = py
        .detach(|| analyzer.compute(&instrument, &market_ctx, date, &scenario))
        .map_err(scenarios_to_py)?;

    Ok(PyHorizonResult { inner: result })
}

/// Horizon total return result.
///
/// Wraps a full P&L attribution with scenario context and convenience
/// accessors for total return (decimal fraction), annualized return, and
/// per-factor contributions. ``scenario_report`` is the ``ApplicationReport``
/// from applying the scenario.
#[pyclass(
    name = "HorizonResult",
    module = "finstack_quant.scenarios",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyHorizonResult {
    inner: finstack_quant_scenarios::horizon::HorizonResult,
}

const HORIZON_COLUMNS: [&str; 11] = [
    "initial_value",
    "terminal_value",
    "currency",
    "total_pnl",
    "total_return",
    "annualized_return",
    "horizon_days",
    "user_operations",
    "expanded_operations",
    "operations_applied",
    "warning_count",
];

#[pymethods]
impl PyHorizonResult {
    /// Full P&L attribution breakdown.
    #[getter]
    fn attribution(&self) -> PyPnlAttribution {
        PyPnlAttribution {
            inner: self.inner.attribution.clone(),
        }
    }

    /// Initial instrument value (bare amount in ``currency``).
    #[getter]
    fn initial_value(&self) -> f64 {
        self.inner.initial_value.amount()
    }

    /// Final instrument value after the scenario (bare amount in ``currency``).
    #[getter]
    fn terminal_value(&self) -> f64 {
        self.inner.terminal_value.amount()
    }

    /// ISO-4217 code of the initial and terminal values.
    #[getter]
    fn currency(&self) -> String {
        self.inner.initial_value.currency().to_string()
    }

    /// Horizon in calendar days (``None`` if no time-roll).
    #[getter]
    fn horizon_days(&self) -> Option<i64> {
        self.inner.horizon_days
    }

    /// Total return as a decimal fraction (``0.05`` = +5%).
    ///
    /// ``nan`` when the initial value and total P&L are in different
    /// currencies or the initial value is zero or negative.
    #[getter]
    fn total_return(&self) -> f64 {
        self.inner.total_return()
    }

    /// Annualized return as a decimal fraction (``None`` if no time-roll or
    /// ``total_return`` is not finite).
    #[getter]
    fn annualized_return(&self) -> Option<f64> {
        self.inner.annualized_return()
    }

    /// Report from applying the scenario (counters, change manifest,
    /// structured warnings, time-roll report).
    #[getter]
    fn scenario_report(&self) -> PyApplicationReport {
        PyApplicationReport::from_inner(self.inner.scenario_report.clone())
    }

    /// Structured warnings from scenario application (``list[dict]`` with a
    /// ``kind`` discriminator); shorthand for ``scenario_report.warnings``.
    #[getter]
    fn warnings<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scenario_report.warnings)
    }

    /// The structured warnings as one JSON-encoded array.
    #[getter]
    fn warnings_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.scenario_report.warnings).map_err(display_to_py)
    }

    /// Factor contribution as decimal fraction of initial value.
    ///
    /// ``factor`` must be one of the canonical serde names from
    /// ``AttributionFactor``: ``"carry"``, ``"rates_curves"``,
    /// ``"credit_curves"``, ``"inflation_curves"``, ``"correlations"``,
    /// ``"fx"``, ``"volatility"``, ``"market_scalars"``, or
    /// ``"model_parameters"``.
    fn factor_contribution(&self, factor: &str) -> PyResult<f64> {
        use finstack_quant_attribution::AttributionFactor;
        let f: AttributionFactor = serde_json::from_value(serde_json::Value::String(
            factor.to_string(),
        ))
        .map_err(|_| {
            crate::errors::value_error(format!(
                "Unknown factor '{factor}'. Expected one of: carry, rates_curves, \
                         credit_curves, inflation_curves, correlations, fx, volatility, \
                         market_scalars, model_parameters"
            ))
        })?;
        Ok(self.inner.factor_contribution(&f))
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON produced by ``to_json``.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_scenarios::horizon::HorizonResult =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Export the horizon summary as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``initial_value``, ``terminal_value``, ``currency``,
    /// ``total_pnl``, ``total_return``, ``annualized_return``,
    /// ``horizon_days``, ``user_operations``, ``expanded_operations``,
    /// ``operations_applied``, ``warning_count``. ``total_return`` and
    /// ``annualized_return`` are decimal fractions.
    ///
    /// For the factor-level breakdown use
    /// ``result.attribution.to_dataframe()``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::json!({
            "initial_value": self.inner.initial_value.amount(),
            "terminal_value": self.inner.terminal_value.amount(),
            "currency": self.inner.initial_value.currency().to_string(),
            "total_pnl": self.inner.attribution.total_pnl.amount(),
            "total_return": self.inner.total_return(),
            "annualized_return": self.inner.annualized_return(),
            "horizon_days": self.inner.horizon_days,
            "user_operations": self.inner.scenario_report.user_operations,
            "expanded_operations": self.inner.scenario_report.expanded_operations,
            "operations_applied": self.inner.scenario_report.operations_applied,
            "warning_count": self.inner.scenario_report.warnings.len(),
        });
        crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &HORIZON_COLUMNS,
        )
    }

    /// Human-readable multi-line summary (total and annualized return,
    /// horizon, values, and the carry / rates / credit / residual legs).
    fn explain(&self) -> String {
        self.inner.to_string()
    }

    fn __repr__(&self) -> String {
        format!(
            "HorizonResult(total_return={:.6}, horizon_days={})",
            self.inner.total_return(),
            self.inner
                .horizon_days
                .map_or_else(|| "None".to_string(), |d| d.to_string()),
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from ``to_dataframe``. Returns ``None`` if the
    /// frame cannot be built, which makes IPython fall back to ``__repr__``.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Register horizon functions on the scenarios submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyHorizonResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(compute_horizon_return, m)?)?;
    Ok(())
}
