//! Instrument pricing pipeline: canonical instrument envelope + market → ValuationResult.
//!
//! Also binds the two typed option payloads `price_instrument` accepts:
//! `MetricPricingOverrides` (metric-time overrides) and `MarketHistory`
//! (historical scenarios for `hvar` / `expected_shortfall`).

use super::PyValuationResult;
use crate::bindings::extract::{extract_instrument_json, extract_market};
use crate::bindings::module_utils::{py_to_json_string, py_to_serde};
use crate::bindings::pandas_utils::{serde_rows_to_dataframe_with_schema, serde_to_py};
use crate::errors::{core_to_py, display_to_py, value_error};
use finstack_quant_valuations::instruments::cashflow_export::InstrumentCashflowEnvelope;
use finstack_quant_valuations::instruments::MetricPricingOverrides;
use finstack_quant_valuations::metrics::risk::{MarketHistory, MarketScenario};
use pyo3::prelude::*;
use pyo3::types::PyString;

/// Metric-time pricing overrides merged into an instrument before pricing.
///
/// Typed twin of the ``metric_pricing_overrides`` JSON accepted by
/// ``price_instrument``, typed ``.price()`` and ``validate_instrument_json``.
/// Every field mirrors the Rust ``MetricPricingOverrides`` struct; omitted
/// fields keep the instrument's own overrides.
///
/// Parameters
/// ----------
/// bump_config : dict | None
///     Finite-difference bump sizes (``spot_bump_decimal``, ``vol_bump_decimal``,
///     ``rate_bump_bp``, ``credit_spread_bump_bp``, ``ytm_bump_bp``,
///     ``adaptive_bumps``). ``None`` keeps defaults.
/// theta_period : Tenor | str | None
///     Theta / carry horizon such as ``"1D"``, ``"1W"``, ``"1M"``, ``"3M"``
///     (a ``finstack_quant.core.dates.Tenor`` or tenor string). ``None`` uses
///     one day.
/// breakeven_config : dict | None
///     Breakeven solve configuration, e.g.
///     ``{"target": "z_spread", "mode": "linear"}``.
/// bond_risk_basis : str | None
///     ``"bullet_discountable"`` (Bloomberg workout risk, default) or
///     ``"callable_oas"``.
/// theta_day_basis : str | None
///     Day basis for per-day analytic option theta: ``"calendar_365"``
///     (default, annual theta / 365) or ``"trading_252"`` (annual theta / 252).
/// var_config : dict | None
///     Historical VaR / expected-shortfall configuration override.
///
/// Raises
/// ------
/// ValueError
///     If a sub-document is malformed, ``theta_period`` is not a valid
///     positive tenor, or ``theta_day_basis`` is not a recognized basis.
/// TypeError
///     If ``theta_period`` is neither a ``Tenor`` nor a string.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import MetricPricingOverrides
/// >>> opts = MetricPricingOverrides(theta_period="1W")
/// >>> str(opts.theta_period)
/// '1W'
#[pyclass(
    name = "MetricPricingOverrides",
    module = "finstack_quant.valuations.instruments",
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyMetricPricingOverrides {
    pub(crate) inner: MetricPricingOverrides,
}

fn opt_serde_from_py<T: serde::de::DeserializeOwned + Send>(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
    label: &str,
) -> PyResult<Option<T>> {
    match obj {
        None => Ok(None),
        Some(value) if value.is_none() => Ok(None),
        Some(value) => py_to_serde(py, value, label).map(Some),
    }
}

#[pymethods]
impl PyMetricPricingOverrides {
    #[new]
    #[pyo3(signature = (*, bump_config=None, theta_period=None, breakeven_config=None, bond_risk_basis=None, theta_day_basis=None, var_config=None))]
    #[pyo3(
        text_signature = "(*, bump_config=None, theta_period=None, breakeven_config=None, bond_risk_basis=None, theta_day_basis=None, var_config=None)"
    )]
    fn new(
        py: Python<'_>,
        bump_config: Option<&Bound<'_, PyAny>>,
        theta_period: Option<&Bound<'_, PyAny>>,
        breakeven_config: Option<&Bound<'_, PyAny>>,
        bond_risk_basis: Option<&str>,
        theta_day_basis: Option<&str>,
        var_config: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let theta_period = match theta_period {
            Some(value) if !value.is_none() => {
                Some(super::convert::tenor_from_py(value, "theta_period")?)
            }
            _ => None,
        };
        let inner = MetricPricingOverrides {
            bump_config: opt_serde_from_py(py, bump_config, "bump_config")?.unwrap_or_default(),
            theta_period,
            breakeven_config: opt_serde_from_py(py, breakeven_config, "breakeven_config")?,
            bond_risk_basis: bond_risk_basis
                .map(|name| {
                    serde_json::from_value(serde_json::Value::String(name.to_string())).map_err(
                        |_| {
                            value_error(format!(
                                "bond_risk_basis: expected 'bullet_discountable' or 'callable_oas', got '{name}'"
                            ))
                        },
                    )
                })
                .transpose()?,
            theta_day_basis: theta_day_basis
                .map(|name| {
                    serde_json::from_value(serde_json::Value::String(name.to_string())).map_err(
                        |_| {
                            value_error(format!(
                                "theta_day_basis: expected 'calendar_365' or 'trading_252', got '{name}'"
                            ))
                        },
                    )
                })
                .transpose()?,
            var_config: opt_serde_from_py(py, var_config, "var_config")?,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Finite-difference bump configuration as a dict (empty when defaulted).
    #[getter]
    fn bump_config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.bump_config)
    }

    /// Theta / carry horizon as a ``Tenor``, or ``None`` for the one-day default.
    #[getter]
    fn theta_period(&self) -> Option<crate::bindings::core::dates::tenor::PyTenor> {
        self.inner
            .theta_period
            .map(crate::bindings::core::dates::tenor::PyTenor::from_inner)
    }

    /// Breakeven configuration dict, or ``None``.
    #[getter]
    fn breakeven_config<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .breakeven_config
            .as_ref()
            .map(|cfg| serde_to_py(py, cfg))
            .transpose()
    }

    /// Bond risk basis serde name (``"bullet_discountable"`` / ``"callable_oas"``), or ``None``.
    #[getter]
    fn bond_risk_basis(&self) -> PyResult<Option<String>> {
        self.inner
            .bond_risk_basis
            .as_ref()
            .map(super::convert::enum_to_py_string)
            .transpose()
    }

    /// Per-day theta basis serde name (``"calendar_365"`` / ``"trading_252"``), or ``None``.
    #[getter]
    fn theta_day_basis(&self) -> PyResult<Option<String>> {
        self.inner
            .theta_day_basis
            .as_ref()
            .map(super::convert::enum_to_py_string)
            .transpose()
    }

    /// Historical VaR configuration dict, or ``None``.
    #[getter]
    fn var_config<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .var_config
            .as_ref()
            .map(|cfg| serde_to_py(py, cfg))
            .transpose()
    }

    /// Deserialize overrides from canonical JSON.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON document produced by ``to_json`` (unknown fields are rejected).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or fails validation.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: MetricPricingOverrides = serde_json::from_str(json).map_err(|e| {
            crate::errors::serde_json_to_py(e, "invalid MetricPricingOverrides JSON")
        })?;
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize these overrides to compact JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` (and therefore ``multiprocessing``, ``joblib``, ``dask``).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> PyResult<String> {
        let quoted = |value: &Option<String>| match value {
            Some(s) => format!("'{s}'"),
            None => "None".to_string(),
        };
        let json_or_none = |value: Option<serde_json::Value>| match value {
            Some(v) => v.to_string(),
            None => "None".to_string(),
        };
        let bump = if self.inner.bump_config.is_empty() {
            "None".to_string()
        } else {
            serde_json::to_value(&self.inner.bump_config)
                .map_err(display_to_py)?
                .to_string()
        };
        Ok(format!(
            "MetricPricingOverrides(bump_config={}, theta_period={}, breakeven_config={}, bond_risk_basis={}, theta_day_basis={}, var_config={})",
            bump,
            quoted(&self.inner.theta_period.map(|period| period.to_string())),
            json_or_none(
                self.inner
                    .breakeven_config
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(display_to_py)?
            ),
            quoted(&self.bond_risk_basis()?),
            quoted(&self.theta_day_basis()?),
            json_or_none(
                self.inner
                    .var_config
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(display_to_py)?
            ),
        ))
    }
}

/// Historical market shifts for historical VaR / expected shortfall.
///
/// Typed twin of the ``market_history`` JSON accepted by ``price_instrument``.
/// Each scenario is one historical date carrying a list of risk-factor
/// shifts relative to the base market; ``hvar`` and ``expected_shortfall``
/// revalue the instrument under every scenario.
///
/// Parameters
/// ----------
/// base_date : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Reference date of the base market the shifts are relative to.
/// window_days : int
///     Length of the historical lookback window in calendar days.
/// scenarios : list[dict]
///     Chronological scenarios, each ``{"date": "YYYY-MM-DD", "shifts":
///     [{"factor": {...}, "shift": float}, ...]}``. ``factor`` is a tagged
///     risk factor: ``{"type": "discount_rate" | "forward_rate" |
///     "credit_spread", "curve_id": str, "tenor_years": float}``,
///     ``{"type": "equity_spot", "ticker": str}``, ``{"type": "fx_spot",
///     "base": "EUR", "quote": "USD"}`` or ``{"type": "implied_vol",
///     "vol_surface_id": str, "expiry_years": float, "strike": float}``.
///     Rate/spread shifts are decimal (``0.0015`` = 15bp); spot shifts are
///     relative (``-0.025`` = -2.5%); vol shifts are absolute vol points.
///
/// Raises
/// ------
/// ValueError
///     If a scenario document is malformed or ``base_date`` is not a date.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import MarketHistory
/// >>> history = MarketHistory("2024-01-01", 2, [
/// ...     {"date": "2023-12-29", "shifts": [
/// ...         {"factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0}, "shift": 0.0010}]},
/// ...     {"date": "2023-12-28", "shifts": [
/// ...         {"factor": {"type": "discount_rate", "curve_id": "USD-OIS", "tenor_years": 5.0}, "shift": -0.0005}]},
/// ... ])
/// >>> len(history)
/// 2
#[pyclass(
    name = "MarketHistory",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMarketHistory {
    pub(crate) inner: MarketHistory,
}

#[pymethods]
impl PyMarketHistory {
    #[new]
    #[pyo3(text_signature = "(base_date, window_days, scenarios)")]
    fn new(
        py: Python<'_>,
        base_date: &Bound<'_, PyAny>,
        window_days: u32,
        scenarios: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let base_date = crate::bindings::date_utils::extract_date(base_date)?;
        let scenarios: Vec<MarketScenario> = py_to_serde(py, scenarios, "scenarios")?;
        Ok(Self {
            inner: MarketHistory::new(base_date, window_days, scenarios),
        })
    }

    /// Build from a plain ``dict`` with keys ``base_date``, ``window_days``, ``scenarios``.
    ///
    /// Parameters
    /// ----------
    /// data : dict
    ///     Same document shape as ``to_json`` emits, as a Python dict.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the document is malformed or carries unknown fields.
    #[staticmethod]
    #[pyo3(text_signature = "(data)")]
    fn from_dict(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: py_to_serde(py, data, "MarketHistory")?,
        })
    }

    /// Reference date of the base market, as ``datetime.date``.
    #[getter]
    fn base_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.base_date)
    }

    /// Historical window length in calendar days.
    #[getter]
    fn window_days(&self) -> u32 {
        self.inner.window_days
    }

    /// Scenarios as a list of dicts in chronological order.
    #[getter]
    fn scenarios<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scenarios)
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// One row per risk-factor shift as a tidy pandas ``DataFrame``.
    ///
    /// Columns: ``date`` (ISO 8601 string), ``type`` (risk-factor tag),
    /// ``curve_id``, ``tenor_years``, ``ticker``, ``base``, ``quote``,
    /// ``vol_surface_id``, ``expiry_years``, ``strike`` (``NaN``/``None``
    /// where the factor type has no such coordinate) and ``shift``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for scenario in &self.inner.scenarios {
            for shift in &scenario.shifts {
                let mut row = serde_json::to_value(&shift.factor).map_err(display_to_py)?;
                if let serde_json::Value::Object(map) = &mut row {
                    map.insert(
                        "date".to_string(),
                        serde_json::Value::String(scenario.date.to_string()),
                    );
                    map.insert("shift".to_string(), serde_json::json!(shift.shift));
                }
                rows.push(row);
            }
        }
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[
                ("date", "str"),
                ("type", "str"),
                ("curve_id", "str"),
                ("tenor_years", "float64"),
                ("ticker", "str"),
                ("base", "str"),
                ("quote", "str"),
                ("vol_surface_id", "str"),
                ("expiry_years", "float64"),
                ("strike", "float64"),
                ("shift", "float64"),
            ],
        )
    }

    /// Deserialize a market history from canonical JSON.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON document produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or carries unknown fields.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid MarketHistory JSON"))?,
        })
    }

    /// Serialize this history to compact JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` (and therefore ``multiprocessing``, ``joblib``, ``dask``).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "MarketHistory(base_date={}, window_days={}, scenarios=<{} items>)",
            self.inner.base_date,
            self.inner.window_days,
            self.inner.scenarios.len()
        )
    }
}

/// Coerce ``MetricPricingOverrides | dict | str | None`` into the JSON the
/// Rust pricing entry points accept.
///
/// The single conversion path for the ``metric_pricing_overrides`` keyword of
/// ``price_instrument``, every typed ``.price()`` and
/// ``validate_instrument_json``.
///
/// # Arguments
///
/// * `py` - GIL token used for ``json.dumps`` on dict inputs.
/// * `obj` - ``None``, a typed ``MetricPricingOverrides``, a JSON string, or a
///   dict of override fields.
pub(crate) fn metric_pricing_overrides_json(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<String>> {
    let Some(obj) = obj else {
        return Ok(None);
    };
    if obj.is_none() {
        return Ok(None);
    }
    if let Ok(typed) = obj.cast::<PyMetricPricingOverrides>() {
        return typed.borrow().to_json().map(Some);
    }
    if let Ok(text) = obj.cast::<PyString>() {
        return Ok(Some(text.to_str()?.to_owned()));
    }
    py_to_json_string(py, obj, "metric_pricing_overrides").map(Some)
}

/// Coerce ``dict | str | MarketHistory | None`` into the JSON the Rust
/// pricing entry point accepts.
pub(crate) fn market_history_json(
    py: Python<'_>,
    obj: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<String>> {
    let Some(obj) = obj else {
        return Ok(None);
    };
    if obj.is_none() {
        return Ok(None);
    }
    if let Ok(typed) = obj.cast::<PyMarketHistory>() {
        return typed.borrow().to_json().map(Some);
    }
    if let Ok(text) = obj.cast::<PyString>() {
        return Ok(Some(text.to_str()?.to_owned()));
    }
    py_to_json_string(py, obj, "market_history").map(Some)
}

/// Price an instrument and return a ``ValuationResult``.
///
/// Parameters
/// ----------
/// instrument : str | Bond | TermLoan | InterestRateSwap | Swaption |
///     CapFloor | CreditDefaultSwap | CdsIndex | FxForward | FxOption |
///     CdsTranche | ConvertibleBond | EquityOption | StructuredCredit |
///     CompositeInstrument
///     A typed instrument instance or a ``finstack_quant.instrument/1``
///     JSON envelope.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date, either a date-like object or an ISO 8601 string.
/// model : str
///     Model key: ``"default"`` (the instrument's registered default),
///     ``"discounting"``, ``"black76"``, ``"hazard_rate"``, ``"hull_white_1f"``,
///     ``"tree"``, ``"rates_credit"``, ``"normal"``, ``"monte_carlo_gbm"``,
///     ... — see ``list_models_grouped()``. For bonds, ``"discounting"`` is
///     non-callable rates-only PV, ``"hazard_rate"`` is non-callable
///     fractional recovery of par,
///     ``"tree"`` values rates-only exercise rights, and ``"rates_credit"``
///     values joint rates-credit bonds including call, put, and return floors.
/// metrics : list[str] | None
///     Metric identifiers to compute (e.g. ``["ytm", "dv01", "duration_mod"]``;
///     see ``list_standard_metrics()``). ``None`` or ``[]`` means valuation
///     only. Mortgage OAS and CMO Z-spread require clean prices per 100 of
///     current face and add settlement accrued interest. MBS ``dv01`` and
///     ``bucketed_dv01`` include the same rate-dependent prepayments as
///     ``duration_mod``. FI TRS ``duration_dv01`` requires ``duration_id`` and
///     its finite signed duration scalar in years. Roll specialness is in
///     basis points against ``repo_curve_id`` (a discount curve), or the
///     discount curve when absent; implied financing is an ACT/360 decimal.
/// metric_pricing_overrides : MetricPricingOverrides | dict | str | None
///     Metric-time overrides merged into
///     ``instrument.spec.metric_pricing_overrides`` before pricing: ``theta_period`` (a tenor; in dict/JSON form
///     ``{"count": 1, "unit": "weeks"}``, or ``MetricPricingOverrides(theta_period="1W")``),
///     ``breakeven_config`` (``{"target": "z_spread", "mode": "linear"}``),
///     ``bump_config``, ``bond_risk_basis``, ``theta_day_basis``, ``var_config``,
///     ``None`` keeps the instrument's own overrides.
///     Dict and JSON patches retain omitted fields, including individual
///     ``bump_config`` fields; explicit ``None``/``null`` clears optional fields.
/// market_history : MarketHistory | dict | str | None
///     Historical scenarios required by the ``hvar`` and
///     ``expected_shortfall`` metrics.
///
/// Returns
/// -------
/// ValuationResult
///     Typed valuation envelope carrying value, currency, metrics, and
///     covenant flags. A stochastic ``"rates_credit"`` bond result also
///     carries Monte Carlo convergence and reproducibility diagnostics in
///     ``details``.
///
/// Raises
/// ------
/// KeyError
///     If a curve, surface, fixing series or scalar the instrument depends on
///     is missing from ``market``.
/// ValueError
///     If the instrument, market, date or option payloads are malformed, a
///     metric is unknown or not applicable, or the instrument fails
///     validation for the requested model (e.g. a seasoned floating leg
///     without fixings).
/// RuntimeError
///     If the model or a metric solver fails numerically, or instrument pricing
///     wraps a failure with instrument/model context. This includes missing
///     quanto inputs, asset-currency mismatches, and an analytical barrier model
///     requested for discrete monitoring; the message retains the cause.
///
/// Notes
/// -----
/// When stochastic rate or credit factors are configured for a
/// ``"rates_credit"`` bond, ``result.details`` is tagged
/// ``{"type": "monte_carlo", "data": ...}``.
/// Its data contains the sampling-only standard error, configured independent
/// exercise-policy paths (``training_paths``) and their total simulated count
/// including antithetic partners (``training_simulated_paths``), configured
/// independent make-whole-reference paths (``make_whole_training_paths``) and
/// their simulated count (``make_whole_training_simulated_paths``), independent
/// estimator paths (``estimator_paths``) and their simulated count
/// (``simulated_paths``), random seed, simulation time grid, and
/// variance-reduction flags. The standard error measures pricing-path
/// sampling uncertainty under the frozen fitted exercise policy and excludes
/// regression approximation, time-grid discretization, and model error. A
/// training stage that did not run reports zero paths.
///
/// The wire payload is still one call away: ``result.to_json()`` returns the
/// JSON that ``ValuationResult.from_json`` accepts, for pipelines that
/// serialize results.
#[pyfunction]
#[pyo3(signature = (instrument, market, as_of, model="default", metrics=None, metric_pricing_overrides=None, market_history=None))]
#[pyo3(
    text_signature = "(instrument, market, as_of, model='default', metrics=None, metric_pricing_overrides=None, market_history=None)"
)]
// PyO3 binding: the argument list mirrors the Python keyword-argument API, so
// it cannot be collapsed into a parameter struct without changing that API.
#[allow(clippy::too_many_arguments)]
fn price_instrument(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
    metrics: Option<Vec<String>>,
    metric_pricing_overrides: Option<&Bound<'_, PyAny>>,
    market_history: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyValuationResult> {
    super::instruments::price_typed_envelope(
        py,
        extract_instrument_json(instrument)?,
        market,
        as_of,
        model,
        metrics,
        metric_pricing_overrides,
        market_history,
    )
}

/// List all metric IDs in the standard metric registry.
///
/// Returns
/// -------
/// list[str]
///     All registered metric identifiers (sorted alphabetically).
#[pyfunction]
fn list_standard_metrics() -> Vec<String> {
    finstack_quant_valuations::pricer::list_standard_metrics()
}

/// List all standard metrics organized by group.
///
/// Returns a dict `{ group_name: [metric_id, ...], ... }` where each key
/// is a human-readable group name (e.g. "Pricing", "Greeks", "Sensitivity")
/// and the value is a sorted list of metric ID strings.
///
/// Returns
/// -------
/// dict[str, list[str]]
///     Metrics grouped by category.
#[pyfunction]
fn list_standard_metrics_grouped() -> std::collections::BTreeMap<String, Vec<String>> {
    finstack_quant_valuations::pricer::list_standard_metrics_grouped()
}

/// Describe canonical metric keys using Rust-owned units and coordinates.
///
/// Parameters
/// ----------
/// keys : list[str]
///     Canonical scalar or qualified wire keys; input order and duplicates
///     are retained and qualified coordinates require the native composite
///     escaping. Custom names are accepted with unknown units and no group.
///
/// Returns
/// -------
/// list[dict[str, Any]]
///     One record per key with ``key``, ``metric``, ``components`` (decoded
///     coordinate labels in original order), ``unit`` (canonical unit family
///     string), ``group`` (native display group or ``None``) and
///     ``bucketed``. No prices are changed and a unit family does not imply
///     a bump size or FX conversion.
///
/// Raises
/// ------
/// ValueError
///     If a key uses malformed or obsolete composite encoding.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import metric_metadata
/// >>> metric_metadata(["ytm"])[0]["unit"]
/// 'decimal'
#[pyfunction]
#[pyo3(text_signature = "(keys)")]
fn metric_metadata<'py>(py: Python<'py>, keys: Vec<String>) -> PyResult<Bound<'py, PyAny>> {
    let metadata = finstack_quant_valuations::pricer::metric_metadata(&keys).map_err(core_to_py)?;
    serde_to_py(py, &metadata)
}

/// List every pricing model key registered in the standard pricer registry.
///
/// The list is registry-derived rather than enum-derived: it reflects real
/// dispatch coverage, so a model with no registered pricer is omitted. The
/// returned names are the canonical keys accepted by the ``model`` argument of
/// :func:`price_instrument`.
///
/// Returns
/// -------
/// list[str]
///     Canonical model keys (e.g. ``"discounting"``, ``"rates_credit"``),
///     sorted.
#[pyfunction]
fn list_models() -> Vec<String> {
    finstack_quant_valuations::pricer::list_models()
}

/// List the standard registry's pricing models grouped by instrument type.
///
/// Returns a dict ``{ instrument_type: [model_key, ...], ... }``. Only
/// instrument types with at least one registered pricer appear, and each entry
/// lists only the models that can actually price that instrument.
///
/// Returns
/// -------
/// dict[str, list[str]]
///     Model keys grouped by canonical instrument-type name. The ``"bond"``
///     entry includes ``"discounting"``, ``"hazard_rate"``, ``"tree"``, and
///     ``"rates_credit"``.
#[pyfunction]
fn list_models_grouped() -> std::collections::BTreeMap<String, Vec<String>> {
    finstack_quant_valuations::pricer::list_models_grouped()
}

/// Return the maintained liquid listed-derivatives coverage catalog.
///
/// Parameters
/// ----------
/// exchange : str | None, optional
///     Exact venue filter: ``"cme"``, ``"eurex"``, ``"montreal"``, or
///     ``"sgx"``. ``None`` returns all venues.
///
/// Returns
/// -------
/// list[dict[str, object]]
///     Product-family rows with the canonical instrument type, exercised
///     features, source URL, and any residual modelling gap.
///
/// Raises
/// ------
/// ValueError
///     If ``exchange`` is not one of the accepted canonical venue names, or
///     if the embedded listed-product sidecar is invalid.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.market import listed_product_catalog
/// >>> rows = listed_product_catalog("cme")
/// >>> all(row["exchange"] == "cme" for row in rows)
/// True
#[pyfunction(signature = (exchange=None))]
fn listed_product_catalog<'py>(
    py: Python<'py>,
    exchange: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let exchange = exchange
        .map(str::parse::<finstack_quant_valuations::market::listed::ListedExchange>)
        .transpose()
        .map_err(value_error)?;
    let rows = finstack_quant_valuations::market::listed::listed_product_catalog(exchange)
        .map_err(core_to_py)?;
    serde_to_py(py, &rows)
}

/// Per-flow cashflow envelope (DF / survival / PV) for a discountable instrument.
///
/// Supported ``model`` values are ``"discounting"`` (DF-only PV) and
/// ``"hazard_rate"`` (DF × survival + recovery on principal). Any other model
/// key, or an instrument type that isn't priced under the chosen model in the
/// standard registry, raises ``ValueError``. Hazard-rate export also rejects a
/// bond with call, put, or return-floor rights because static rows cannot
/// represent its exercise-contingent value. For supported static-flow
/// combinations, the returned envelope's ``total_pv`` reconciles with the
/// instrument's ``base_value``.
///
/// Parameters
/// ----------
/// instrument : str | Bond | TermLoan | InterestRateSwap | Swaption |
///     CapFloor | CreditDefaultSwap | CdsIndex | FxForward | FxOption |
///     CdsTranche | ConvertibleBond | EquityOption | StructuredCredit |
///     CompositeInstrument
///     A typed instrument instance or a ``finstack_quant.instrument/1``
///     JSON envelope.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date, either a date-like object or an ISO 8601 string.
/// model : str
///     ``"discounting"`` or ``"hazard_rate"``. ``"default"`` is not accepted.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``InstrumentCashflowEnvelope``; the typed twin
///     :func:`instrument_cashflows` returns the same envelope as an
///     ``InstrumentCashflowEnvelope`` object.
///
/// Raises
/// ------
/// KeyError
///     If a curve or fixing series the instrument depends on is missing.
/// ValueError
///     If ``model`` is unsupported, the instrument/model pair is not
///     registered, a bond with embedded exercise rights is requested under a
///     static cashflow model, or a payload is malformed.
/// RuntimeError
///     If the pricer fails numerically.
#[pyfunction]
#[pyo3(text_signature = "(instrument, market, as_of, model)")]
fn instrument_cashflows_json(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
) -> PyResult<String> {
    let envelope = cashflow_envelope(py, instrument, market, as_of, model)?;
    serde_json::to_string(&envelope).map_err(display_to_py)
}

/// Parse the instrument (before the market) and run the Rust
/// `instrument_cashflows` export shared by both cashflow entry points.
fn cashflow_envelope(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
) -> PyResult<InstrumentCashflowEnvelope> {
    let instrument_json = extract_instrument_json(instrument)?;
    let instrument = py.detach(move || {
        finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(&instrument_json, None)
            .map_err(core_to_py)
    })?;
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    py.detach(move || {
        finstack_quant_valuations::instruments::cashflow_export::instrument_cashflows(
            &instrument,
            &market,
            &as_of,
            &model,
        )
        .map_err(core_to_py)
    })
}

/// Per-flow cashflow envelope (DF / survival / PV) for a discountable instrument.
///
/// Typed twin of :func:`instrument_cashflows_json`: the same Rust
/// ``instrument_cashflows`` export, returned as an
/// :class:`InstrumentCashflowEnvelope` (``to_dataframe()`` gives one row per
/// flow). Supported ``model`` values are ``"discounting"`` (DF-only PV) and
/// ``"hazard_rate"`` (DF × survival + recovery on principal). Hazard-rate
/// export rejects bonds with call, put, or return-floor rights because static
/// rows cannot represent their exercise-contingent value. For supported
/// static-flow combinations ``total_pv`` reconciles with the instrument's
/// ``base_value``.
///
/// Parameters
/// ----------
/// instrument : str | Bond | TermLoan | InterestRateSwap | Swaption |
///     CapFloor | CreditDefaultSwap | CdsIndex | FxForward | FxOption |
///     CdsTranche | CdsOption | ConvertibleBond | EquityOption |
///     StructuredCredit | CompositeInstrument
///     A typed instrument instance or a ``finstack_quant.instrument/1``
///     JSON envelope.
/// market : MarketContext | str
///     A ``MarketContext`` object or a JSON string.
/// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
///     Valuation date, either a date-like object or an ISO 8601 string.
/// model : str
///     ``"discounting"`` or ``"hazard_rate"``. ``"default"`` is not accepted.
///
/// Returns
/// -------
/// InstrumentCashflowEnvelope
///     Header fields plus one ``flows`` row per cashflow.
///
/// Raises
/// ------
/// KeyError
///     If a curve or fixing series the instrument depends on is missing.
/// ValueError
///     If ``model`` is unsupported, the instrument/model pair is not
///     registered, a bond with embedded exercise rights is requested under a
///     static cashflow model, or a payload is malformed.
/// RuntimeError
///     If the pricer fails numerically.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.core.currency import Currency
/// >>> from finstack_quant.core.dates import StubKind
/// >>> from finstack_quant.core.market_data import DiscountCurve, MarketContext
/// >>> from finstack_quant.core.money import Money
/// >>> from finstack_quant.core.types import Rate
/// >>> from finstack_quant.valuations.instruments import Bond, instrument_cashflows
/// >>> as_of = datetime.date(2024, 1, 1)
/// >>> bond = Bond.fixed(
/// ...     "B", Money(1000.0, Currency("USD")), Rate(0.05), as_of, datetime.date(2026, 1, 1), StubKind.NONE, "USD-OIS"
/// ... )
/// >>> market = MarketContext().insert(DiscountCurve.flat("USD-OIS", as_of, 0.04))
/// >>> envelope = instrument_cashflows(bond, market, as_of, "discounting")
/// >>> (envelope.instrument_id, len(envelope.to_dataframe()))
/// ('B', 6)
#[pyfunction]
#[pyo3(text_signature = "(instrument, market, as_of, model)")]
fn instrument_cashflows(
    py: Python<'_>,
    instrument: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    model: &str,
) -> PyResult<PyInstrumentCashflowEnvelope> {
    cashflow_envelope(py, instrument, market, as_of, model)
        .map(|inner| PyInstrumentCashflowEnvelope { inner })
}

/// Cashflow export for one instrument: header fields plus per-flow rows.
///
/// Typed wrapper of the Rust ``InstrumentCashflowEnvelope`` returned by
/// :func:`instrument_cashflows`. ``flows`` holds one dict per cashflow;
/// ``to_dataframe()`` is the tabular view.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import InstrumentCashflowEnvelope
/// >>> envelope = InstrumentCashflowEnvelope.from_json(
/// ...     '{"instrument_id": "B", "currency": "USD", "model": "discounting",'
/// ...     ' "as_of": "2025-01-15", "discount_curve_id": "USD-OIS", "flows": [],'
/// ...     ' "total_pv": 0.0, "reconciles_with_base_value": true}')
/// >>> (envelope.instrument_id, envelope.currency, len(envelope.to_dataframe()))
/// ('B', 'USD', 0)
#[pyclass(
    name = "InstrumentCashflowEnvelope",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyInstrumentCashflowEnvelope {
    pub(crate) inner: InstrumentCashflowEnvelope,
}

/// Documented column order of ``InstrumentCashflowEnvelope.to_dataframe``:
/// every ``CashflowRow`` field, in struct order, with its empty-frame dtype.
const CASHFLOW_ROW_COLUMNS: &[(&str, &str)] = &[
    ("date", "str"),
    ("amount", "float64"),
    ("currency", "str"),
    ("kind", "str"),
    ("accrual_factor", "float64"),
    ("year_fraction", "float64"),
    ("rate", "float64"),
    ("reset_date", "str"),
    ("discount_factor", "float64"),
    ("discount_curve_id", "str"),
    ("survival_probability", "float64"),
    ("conditional_default_prob", "float64"),
    ("inflation_index_ratio", "float64"),
    ("prepayment_smm", "float64"),
    ("beginning_balance", "float64"),
    ("ending_balance", "float64"),
    ("pv", "float64"),
];

#[pymethods]
impl PyInstrumentCashflowEnvelope {
    /// Instrument identifier.
    #[getter]
    fn instrument_id(&self) -> String {
        self.inner.instrument_id.clone()
    }

    /// Reporting currency code of the row PVs and ``total_pv``.
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.to_string()
    }

    /// Model key used: ``"discounting"`` or ``"hazard_rate"``.
    #[getter]
    fn model(&self) -> String {
        self.inner.model.clone()
    }

    /// Valuation date, as ``datetime.date``.
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.as_of)
    }

    /// Discount curve identifier used for every row.
    #[getter]
    fn discount_curve_id(&self) -> String {
        self.inner.discount_curve_id.to_string()
    }

    /// Hazard curve identifier (``None`` under ``"discounting"``).
    #[getter]
    fn credit_curve_id(&self) -> Option<String> {
        self.inner.credit_curve_id.as_ref().map(ToString::to_string)
    }

    /// Recovery rate of the hazard curve as a decimal (``None`` under ``"discounting"``).
    #[getter]
    fn recovery_rate(&self) -> Option<f64> {
        self.inner.recovery_rate
    }

    /// Per-flow rows as a list of dicts (the serde form of ``CashflowRow``).
    #[getter]
    fn flows<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.flows)
    }

    /// Sum of the row ``pv`` values in the envelope currency.
    #[getter]
    fn total_pv(&self) -> f64 {
        self.inner.total_pv
    }

    /// ``True`` when ``total_pv`` agrees with the instrument's ``base_value``.
    #[getter]
    fn reconciles_with_base_value(&self) -> bool {
        self.inner.reconciles_with_base_value
    }

    /// One row per cashflow as a pandas ``DataFrame``.
    ///
    /// Columns, in order: ``date``, ``amount``, ``currency``, ``kind``,
    /// ``accrual_factor``, ``year_fraction``, ``rate``, ``reset_date``,
    /// ``discount_factor``, ``discount_curve_id``, ``survival_probability``,
    /// ``conditional_default_prob``, ``inflation_index_ratio``,
    /// ``prepayment_smm``, ``beginning_balance``, ``ending_balance``, ``pv``.
    /// ``date`` and ``reset_date`` are ``datetime64``; a field the model does
    /// not populate is null.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let frame =
            serde_rows_to_dataframe_with_schema(py, &self.inner.flows, CASHFLOW_ROW_COLUMNS)?;
        let to_datetime = py.import("pandas")?.getattr("to_datetime")?;
        let parsed = pyo3::types::PyDict::new(py);
        for column in ["date", "reset_date"] {
            parsed.set_item(column, to_datetime.call1((frame.get_item(column)?,))?)?;
        }
        frame.call_method("assign", (), Some(&parsed))
    }

    /// Deserialize an envelope from the JSON produced by ``to_json`` or
    /// ``instrument_cashflows_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Serialized ``InstrumentCashflowEnvelope``.
    ///
    /// Returns
    /// -------
    /// InstrumentCashflowEnvelope
    ///     The parsed envelope.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or misses a required field.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json).map_err(|e| {
                crate::errors::serde_json_to_py(e, "invalid InstrumentCashflowEnvelope JSON")
            })?,
        })
    }

    /// Serialize to compact JSON, identical to ``instrument_cashflows_json``.
    ///
    /// Returns
    /// -------
    /// str
    ///     The envelope JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the envelope cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Support ``pickle`` (and therefore ``multiprocessing``, ``joblib``, ``dask``).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "InstrumentCashflowEnvelope(instrument_id={:?}, model={:?}, flows=<{} rows>, total_pv={})",
            self.inner.instrument_id,
            self.inner.model,
            self.inner.flows.len(),
            self.inner.total_pv
        )
    }
}

/// Register pricing functions on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMetricPricingOverrides>()?;
    m.add_class::<PyMarketHistory>()?;
    m.add_class::<PyInstrumentCashflowEnvelope>()?;
    m.add_function(pyo3::wrap_pyfunction!(instrument_cashflows, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(price_instrument, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_models, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_models_grouped, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_standard_metrics, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_standard_metrics_grouped, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(metric_metadata, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(instrument_cashflows_json, m)?)?;
    Ok(())
}

/// Register listed-market catalog functions on the valuations market submodule.
pub fn register_market(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(listed_product_catalog, m)?)?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &[
    "InstrumentCashflowEnvelope",
    "MarketHistory",
    "MetricPricingOverrides",
];
