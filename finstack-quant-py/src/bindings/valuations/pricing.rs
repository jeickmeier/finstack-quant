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
use finstack_quant_valuations::instruments::{MetricPricingOverrides, PricingOptions};
use finstack_quant_valuations::metrics::risk::{MarketHistory, MarketScenario};
use pyo3::prelude::*;
use pyo3::types::PyString;
use std::sync::Arc;

/// Attach the host-owned cached recalibration provider.
///
/// Lives here rather than in `finstack-quant-valuations` because that crate
/// cannot depend on `finstack-quant-calibration`.
pub(super) fn binding_pricing_options() -> PricingOptions {
    PricingOptions::default().with_recalibration_provider(Arc::new(
        finstack_quant_calibration::recalibration::CachedRecalibrationProvider::new(),
    ))
}

/// Metric-time pricing overrides merged into an instrument before pricing.
///
/// Typed twin of the ``pricing_options`` JSON accepted by ``price_instrument``.
/// Every field mirrors the Rust ``MetricPricingOverrides`` struct; omitted
/// fields keep the instrument's own overrides.
///
/// Parameters
/// ----------
/// bump_config : dict | None
///     Finite-difference bump sizes (``spot_bump_pct``, ``vol_bump_pct``,
///     ``rate_bump_bp``, ``credit_spread_bump_bp``, ``ytm_bump_decimal``,
///     ``rho_bump_decimal``, ``adaptive_bumps``). ``None`` keeps defaults.
/// mc_seed_scenario : str | None
///     Scenario name used to derive deterministic Monte Carlo seeds for
///     finite-difference Greeks (e.g. ``"delta_up"``).
/// theta_period : str | None
///     Theta / carry horizon such as ``"1D"``, ``"1W"``, ``"1M"``, ``"3M"``.
/// breakeven_config : dict | None
///     Breakeven solve configuration, e.g.
///     ``{"target": "z_spread", "mode": "linear"}``.
/// bond_risk_basis : str | None
///     ``"bullet_discountable"`` (Bloomberg workout risk, default) or
///     ``"callable_oas"``.
/// var_config : dict | None
///     Historical VaR / expected-shortfall configuration override.
/// quoted_price_pct : float | None
///     External quoted price as a percentage of original balance (``100.0`` =
///     par), required by structured-credit spread metrics.
///
/// Raises
/// ------
/// ValueError
///     If a sub-document is malformed or ``theta_period`` is not
///     ``<digits><D|W|M|Y>``.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import MetricPricingOverrides
/// >>> opts = MetricPricingOverrides(theta_period="1W")
/// >>> opts.theta_period
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
    #[pyo3(signature = (*, bump_config=None, mc_seed_scenario=None, theta_period=None, breakeven_config=None, bond_risk_basis=None, var_config=None, quoted_price_pct=None))]
    #[pyo3(
        text_signature = "(*, bump_config=None, mc_seed_scenario=None, theta_period=None, breakeven_config=None, bond_risk_basis=None, var_config=None, quoted_price_pct=None)"
    )]
    // PyO3 binding: the argument list mirrors the Rust struct's public
    // fields as a keyword API, so it cannot be collapsed into a params struct
    // without changing that API.
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        bump_config: Option<&Bound<'_, PyAny>>,
        mc_seed_scenario: Option<String>,
        theta_period: Option<String>,
        breakeven_config: Option<&Bound<'_, PyAny>>,
        bond_risk_basis: Option<&str>,
        var_config: Option<&Bound<'_, PyAny>>,
        quoted_price_pct: Option<f64>,
    ) -> PyResult<Self> {
        let inner = MetricPricingOverrides {
            bump_config: opt_serde_from_py(py, bump_config, "bump_config")?.unwrap_or_default(),
            mc_seed_scenario,
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
            var_config: opt_serde_from_py(py, var_config, "var_config")?,
            quoted_price_pct,
        };
        inner.validate().map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Finite-difference bump configuration as a dict (empty when defaulted).
    #[getter]
    fn bump_config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.bump_config)
    }

    /// Monte Carlo seed scenario name, or ``None``.
    #[getter]
    fn mc_seed_scenario(&self) -> Option<String> {
        self.inner.mc_seed_scenario.clone()
    }

    /// Theta / carry horizon (``"1D"``, ``"1W"``, ...), or ``None``.
    #[getter]
    fn theta_period(&self) -> Option<String> {
        self.inner.theta_period.clone()
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

    /// Historical VaR configuration dict, or ``None``.
    #[getter]
    fn var_config<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .var_config
            .as_ref()
            .map(|cfg| serde_to_py(py, cfg))
            .transpose()
    }

    /// Externally quoted price as a percentage of original balance, or ``None``.
    #[getter]
    fn quoted_price_pct(&self) -> Option<f64> {
        self.inner.quoted_price_pct
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
            "MetricPricingOverrides(bump_config={}, mc_seed_scenario={}, theta_period={}, breakeven_config={}, bond_risk_basis={}, var_config={}, quoted_price_pct={})",
            bump,
            quoted(&self.inner.mc_seed_scenario),
            quoted(&self.inner.theta_period),
            json_or_none(
                self.inner
                    .breakeven_config
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(display_to_py)?
            ),
            quoted(&self.bond_risk_basis()?),
            json_or_none(
                self.inner
                    .var_config
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(display_to_py)?
            ),
            super::convert::opt_repr(self.inner.quoted_price_pct),
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

/// Coerce ``dict | str | MetricPricingOverrides | None`` into the JSON the
/// Rust pricing entry point accepts.
pub(crate) fn pricing_overrides_json(
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
    py_to_json_string(py, obj, "pricing_options").map(Some)
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
///     CapFloor | CreditDefaultSwap | CDSIndex | FxForward | FxOption |
///     CDSTranche | ConvertibleBond | EquityOption | StructuredCredit |
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
///     only.
/// pricing_options : MetricPricingOverrides | dict | str | None
///     Metric-time overrides merged into the instrument's ``pricing_overrides``
///     before pricing: ``theta_period`` (``"1D"``, ``"1W"``, ``"1M"``),
///     ``breakeven_config`` (``{"target": "z_spread", "mode": "linear"}``),
///     ``bump_config``, ``bond_risk_basis``, ``var_config``,
///     ``quoted_price_pct``. ``None`` keeps the instrument's own overrides.
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
///     If the model or a metric solver fails numerically (calibration or
///     convergence failure).
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
#[pyo3(signature = (instrument, market, as_of, model="default", metrics=None, pricing_options=None, market_history=None))]
#[pyo3(
    text_signature = "(instrument, market, as_of, model='default', metrics=None, pricing_options=None, market_history=None)"
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
    pricing_options: Option<&Bound<'_, PyAny>>,
    market_history: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyValuationResult> {
    let instrument_json = extract_instrument_json(instrument)?;
    let pricing_options = pricing_overrides_json(py, pricing_options)?;
    let instrument = py.detach(move || {
        finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(
            &instrument_json,
            pricing_options.as_deref(),
        )
        .map_err(core_to_py)
    })?;
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();
    let metrics = metrics.unwrap_or_default();
    let market_history = market_history_json(py, market_history)?;

    let inner = py
        .detach(move || {
            finstack_quant_valuations::pricer::price_instrument(
                &instrument,
                &market,
                &as_of,
                &model,
                &metrics,
                market_history.as_deref(),
                binding_pricing_options(),
            )
        })
        .map_err(core_to_py)?;
    Ok(PyValuationResult { inner })
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
///     CapFloor | CreditDefaultSwap | CDSIndex | FxForward | FxOption |
///     CDSTranche | ConvertibleBond | EquityOption | StructuredCredit |
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
///     JSON-serialized ``InstrumentCashflowEnvelope``. Parse and wrap in a
///     DataFrame via :func:`finstack_quant.valuations.instrument_cashflows`.
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
    let instrument_json = extract_instrument_json(instrument)?;
    let instrument = py.detach(move || {
        finstack_quant_valuations::pricer::parse_boxed_instrument_from_json(&instrument_json, None)
            .map_err(core_to_py)
    })?;
    let market = extract_market(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date_iso(as_of)?;
    let model = model.to_owned();

    py.detach(move || {
        let envelope =
            finstack_quant_valuations::instruments::cashflow_export::instrument_cashflows(
                &instrument,
                &market,
                &as_of,
                &model,
            )
            .map_err(core_to_py)?;
        serde_json::to_string(&envelope).map_err(display_to_py)
    })
}

/// Register pricing functions on the valuations submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMetricPricingOverrides>()?;
    m.add_class::<PyMarketHistory>()?;
    m.add_function(pyo3::wrap_pyfunction!(price_instrument, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_models, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_models_grouped, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_standard_metrics, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(list_standard_metrics_grouped, m)?)?;
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
pub(crate) const EXPORTS: &[&str] = &["MarketHistory", "MetricPricingOverrides"];
