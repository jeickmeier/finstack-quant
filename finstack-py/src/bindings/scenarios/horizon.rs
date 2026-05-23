// finstack-py/src/bindings/scenarios/horizon.rs

//! Python bindings for horizon total return analysis.

use crate::bindings::attribution::PyPnlAttribution;
use crate::bindings::extract::extract_market_ref;
use crate::errors::display_to_py;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Compute horizon total return under a scenario.
///
/// Applies a scenario specification (which may include time-roll and market
/// shocks) to project an instrument forward, then decomposes the resulting
/// P&L using factor-based attribution.
///
/// Parameters
/// ----------
/// instrument_json : str
///     JSON-serialized instrument (tagged: ``{"type": "bond", "spec": {...}}``).
/// market : MarketContext | str
///     A ``MarketContext`` object or JSON string.
/// as_of : str
///     Valuation date in ISO 8601 format (e.g. ``"2025-01-15"``).
/// scenario_json : str
///     JSON-serialized ``ScenarioSpec``.
/// method : str, optional
///     Attribution method: ``"parallel"`` (default), ``"waterfall"``,
///     ``"metrics_based"``, or ``"taylor"``.
///
/// Returns
/// -------
/// HorizonResult
///     Decomposed total return with factor attribution.
#[pyfunction]
#[pyo3(signature = (instrument_json, market, as_of, scenario_json, method = "parallel", config = None))]
pub(crate) fn compute_horizon_return<'py>(
    _py: Python<'py>,
    instrument_json: &str,
    market: &Bound<'py, PyAny>,
    as_of: &str,
    scenario_json: &str,
    method: &str,
    config: Option<&str>,
) -> PyResult<PyHorizonResult> {
    use finstack_attribution::AttributionMethod;
    use finstack_valuations::instruments::InstrumentJson;
    use std::sync::Arc;

    // Parse instrument
    let inst: InstrumentJson = serde_json::from_str(instrument_json).map_err(display_to_py)?;
    let boxed = inst.into_boxed().map_err(display_to_py)?;
    let instrument: Arc<dyn finstack_valuations::instruments::Instrument> = Arc::from(boxed);

    // Parse market
    let market_ctx = extract_market_ref(market)?;

    // Parse date
    let date = super::parse_date(as_of)?;

    // Parse scenario
    let scenario: finstack_scenarios::ScenarioSpec =
        serde_json::from_str(scenario_json).map_err(display_to_py)?;

    // Parse method
    let attribution_method = match method {
        "parallel" => AttributionMethod::Parallel,
        "waterfall" => {
            AttributionMethod::Waterfall(finstack_attribution::default_waterfall_order())
        }
        "metrics_based" => AttributionMethod::MetricsBased,
        "taylor" => {
            AttributionMethod::Taylor(finstack_attribution::TaylorAttributionConfig::default())
        }
        other => {
            return Err(PyValueError::new_err(format!(
                "Unknown attribution method '{other}'. Expected: parallel, waterfall, metrics_based, taylor"
            )));
        }
    };

    // Parse config
    let finstack_config = match config {
        Some(json) => serde_json::from_str(json).map_err(display_to_py)?,
        None => finstack_core::config::FinstackConfig::default(),
    };

    // Run analysis
    let analyzer =
        finstack_scenarios::horizon::HorizonAnalysis::new(attribution_method, finstack_config);
    let result = analyzer
        .compute(&instrument, &market_ctx, date, &scenario)
        .map_err(display_to_py)?;

    Ok(PyHorizonResult { inner: result })
}

/// Horizon total return result.
///
/// Wraps a full P&L attribution with scenario context and convenience
/// accessors for total return percentage, annualized return, and
/// per-factor contributions.
#[pyclass(
    name = "HorizonResult",
    module = "finstack.scenarios",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyHorizonResult {
    inner: finstack_scenarios::horizon::HorizonResult,
}

#[pymethods]
impl PyHorizonResult {
    /// Full P&L attribution breakdown.
    #[getter]
    fn attribution(&self) -> PyPnlAttribution {
        PyPnlAttribution {
            inner: self.inner.attribution.clone(),
        }
    }

    /// Initial instrument value.
    #[getter]
    fn initial_value(&self) -> f64 {
        self.inner.initial_value.amount()
    }

    /// Final instrument value after scenario.
    #[getter]
    fn terminal_value(&self) -> f64 {
        self.inner.terminal_value.amount()
    }

    /// Horizon in calendar days (``None`` if no time-roll).
    #[getter]
    fn horizon_days(&self) -> Option<i64> {
        self.inner.horizon_days
    }

    /// Total return as decimal fraction (0.05 = 5%).
    #[getter]
    fn total_return_pct(&self) -> f64 {
        self.inner.total_return_pct()
    }

    /// Annualized return (``None`` if no time-roll).
    #[getter]
    fn annualized_return(&self) -> Option<f64> {
        self.inner.annualized_return()
    }

    /// Number of scenario operations applied.
    #[getter]
    fn operations_applied(&self) -> usize {
        self.inner.scenario_report.operations_applied
    }

    /// Number of user-provided scenario operations before hierarchy expansion.
    #[getter]
    fn user_operations(&self) -> usize {
        self.inner.scenario_report.user_operations
    }

    /// Number of direct operations after hierarchy expansion and deduplication.
    #[getter]
    fn expanded_operations(&self) -> usize {
        self.inner.scenario_report.expanded_operations
    }

    /// Warnings from scenario application, rendered in human-readable form.
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.inner
            .scenario_report
            .warnings
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Warnings from scenario application as a JSON-encoded array, mirroring
    /// the structured `Warning` enum. Parse with `json.loads(...)` to obtain
    /// `list[dict]` where each entry has a `kind` discriminator plus
    /// variant-specific fields.
    #[getter]
    fn warnings_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.scenario_report.warnings).map_err(display_to_py)
    }

    /// Factor contribution as decimal fraction of initial value.
    fn factor_contribution(&self, factor: &str) -> PyResult<f64> {
        use finstack_attribution::AttributionFactor;
        let f = match factor {
            "carry" => AttributionFactor::Carry,
            "rates" | "rates_curves" => AttributionFactor::RatesCurves,
            "credit" | "credit_curves" => AttributionFactor::CreditCurves,
            "inflation" | "inflation_curves" => AttributionFactor::InflationCurves,
            "correlations" => AttributionFactor::Correlations,
            "fx" => AttributionFactor::Fx,
            "volatility" | "vol" => AttributionFactor::Volatility,
            "model_parameters" | "model_params" => AttributionFactor::ModelParameters,
            "market_scalars" | "scalars" => AttributionFactor::MarketScalars,
            other => {
                return Err(PyValueError::new_err(format!("Unknown factor '{other}'")));
            }
        };
        Ok(self.inner.factor_contribution(&f))
    }

    /// Serialize to JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(display_to_py)
    }

    /// Human-readable summary.
    fn explain(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "Horizon Total Return: {:.4}%\n",
            self.inner.total_return_pct() * 100.0
        ));
        if let Some(ann) = self.inner.annualized_return() {
            s.push_str(&format!("Annualized: {:.4}%\n", ann * 100.0));
        }
        if let Some(days) = self.inner.horizon_days {
            s.push_str(&format!("Horizon: {} days\n", days));
        }
        s.push_str(&format!("Initial Value: {}\n", self.inner.initial_value));
        s.push_str(&format!("Terminal Value: {}\n", self.inner.terminal_value));
        s.push_str(&format!(
            "Total P&L: {}\n",
            self.inner.attribution.total_pnl
        ));
        s.push_str(&format!("  Carry: {}\n", self.inner.attribution.carry));
        s.push_str(&format!(
            "  Rates: {}\n",
            self.inner.attribution.rates_curves_pnl
        ));
        s.push_str(&format!(
            "  Credit: {}\n",
            self.inner.attribution.credit_curves_pnl
        ));
        s.push_str(&format!(
            "  Residual: {}\n",
            self.inner.attribution.residual
        ));
        s
    }

    fn __repr__(&self) -> String {
        format!(
            "HorizonResult(total_return={:.4}%, horizon_days={:?})",
            self.inner.total_return_pct() * 100.0,
            self.inner.horizon_days,
        )
    }
}

/// Register horizon functions on the scenarios submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyHorizonResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(compute_horizon_return, m)?)?;
    Ok(())
}
