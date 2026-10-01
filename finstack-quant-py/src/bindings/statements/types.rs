//! Python wrappers for statement model types and enums.

use crate::bindings::date_utils::date_to_py;
use crate::bindings::pandas_utils::{
    serde_rows_to_dataframe_with_schema, serde_to_py, ColumnSchema,
};
use crate::errors::{core_to_py, serde_json_to_py, statements_to_py};
use finstack_quant_statements::types::{NodeSpec, NodeValueType};
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Columns emitted by `FinancialModelSpec.to_dataframe`.
const NODE_COLUMNS: [ColumnSchema<'static>; 8] = [
    ("node_id", "str"),
    ("node_type", "str"),
    ("name", "str"),
    ("formula_text", "str"),
    ("forecast_method", "str"),
    ("value_type", "str"),
    ("currency", "str"),
    ("where_text", "str"),
];

/// Available forecast methods for projecting node values.
#[pyclass(
    name = "ForecastMethod",
    module = "finstack_quant.statements",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyForecastMethod {
    pub(super) inner: finstack_quant_statements::types::ForecastMethod,
}

#[pymethods]
impl PyForecastMethod {
    /// Carry the last observed value forward unchanged: ``v[t] = v[t-1]``.
    ///
    /// Takes no parameters and never changes the level, so it is the safe
    /// default for stock-like nodes (balances) that should hold flat when no
    /// explicit assumption is supplied.
    #[staticmethod]
    fn forward_fill() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::ForwardFill,
        }
    }

    /// Compound growth at a single rate: ``v[t] = v[t-1] * (1 + rate)``.
    ///
    /// ``rate`` is a **decimal fraction per period**, not a percentage — 0.05
    /// means 5% growth per period, and the period is whatever cadence the
    /// model's period list uses (quarters, months, years). Rates above 100%
    /// per period raise an evaluation warning.
    #[staticmethod]
    fn growth_pct() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::GrowthPct,
        }
    }

    /// Period-specific compound growth: ``v[t] = v[t-1] * (1 + curve[t])``.
    ///
    /// ``curve`` supplies one growth rate per forecast period, each a decimal
    /// fraction per period (0.05 = 5%), consumed in forecast-period order.
    #[staticmethod]
    fn curve_pct() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::CurvePct,
        }
    }

    /// Additive normal random walk:
    /// ``v[t] = v[t-1] + mean + std_dev * z[t]``.
    ///
    /// ``mean`` and ``std_dev`` are per-period **level** quantities in the
    /// node's own units (currency amounts for ``Money`` nodes, unitless for
    /// scalar nodes), not rates. ``z[t]`` is a deterministic standard-normal
    /// draw derived from the configured seed, so runs are reproducible.
    /// ``std_dev`` must be non-negative.
    #[staticmethod]
    fn normal() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::Normal,
        }
    }

    /// Multiplicative log-normal path:
    /// ``v[t] = v[t-1] * exp(mean - 0.5 * std_dev**2 + std_dev * z[t])``.
    ///
    /// ``mean`` is the per-period continuously compounded expected-growth
    /// drift: the expected growth factor is ``exp(mean)``, while the expected
    /// log increment is ``mean - 0.5 * std_dev**2``. ``std_dev`` is log-return
    /// volatility, expressed as a decimal fraction per period. Evaluation
    /// requires a finite, strictly positive base; zero and negative bases
    /// raise ``ValueError``. Use an additive normal forecast for those series.
    #[staticmethod]
    fn log_normal() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::LogNormal,
        }
    }

    /// Explicit per-period overrides, forward-filling the periods in between.
    ///
    /// The method reads an ``overrides`` parameter mapping period-identifier
    /// strings (e.g. ``"2025Q2"``) to values; any forecast period without an
    /// entry inherits the most recent value.
    #[staticmethod]
    #[pyo3(name = "override")]
    fn override_() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::Override,
        }
    }

    /// Project an externally supplied historical series forward with trend
    /// detection.
    ///
    /// The history is passed through the spec's parameters rather than read
    /// from the model graph, so the same series can drive several nodes.
    #[staticmethod]
    fn time_series() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::TimeSeries,
        }
    }

    /// Seasonal decomposition forecast (additive or multiplicative).
    ///
    /// Requires a ``season_length`` parameter counted in **periods** (4 for a
    /// quarterly model, 12 for monthly) and at least two full seasonal cycles
    /// of history. Use additive mode when seasonal swings are constant in
    /// absolute terms and multiplicative when they scale with the level;
    /// prefer additive for series that cross zero.
    #[staticmethod]
    fn seasonal() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::Seasonal,
        }
    }

    /// Glide from the base value to a ``target`` level over the forecast
    /// horizon.
    ///
    /// Shapes (``shape`` parameter, default ``"linear"``): ``"linear"``
    /// reaches the target exactly at the final period; ``"geometric"`` is
    /// constant compound growth (CAGR-to-terminal, requires base and target
    /// non-zero with the same sign); ``"exponential"`` decays the remaining
    /// gap by half every ``half_life`` periods and never quite reaches the
    /// target. The workhorse for fading margins/ratios to a long-run
    /// assumption (NIM normalization, cost-income convergence, ROE fade).
    #[staticmethod]
    fn fade_to_target() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::FadeToTarget,
        }
    }

    /// Mean-reverting AR(1) path:
    /// ``v[t] = v[t-1] + reversion_speed * (long_run_mean - v[t-1]) + std_dev * z[t]``.
    ///
    /// ``long_run_mean`` and ``std_dev`` are in the node's own units;
    /// ``reversion_speed`` is the fraction of the gap closed each period, in
    /// ``(0, 1]``. ``z[t]`` is a deterministic standard-normal draw derived
    /// from the configured seed. Use for autocorrelated series that revert
    /// toward a through-the-cycle level (spreads, charge-off rates, NIM).
    #[staticmethod]
    fn mean_reverting() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::MeanReverting,
        }
    }

    /// Historical bootstrap: resample observed per-period changes with
    /// replacement (deterministic with seed).
    ///
    /// ``mode = "growth"`` (default) resamples period-over-period growth
    /// rates from a strictly positive ``historical`` series and compounds
    /// them; ``mode = "diff"`` resamples additive level changes and works
    /// for series that cross zero. Reproduces the empirical distribution of
    /// changes — including fat tails — without a normality assumption.
    #[staticmethod]
    fn bootstrap() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::Bootstrap,
        }
    }

    /// Canonical snake_case wire discriminant (``"growth_pct"``,
    /// ``"log_normal"``, ``"override"``, ...), derived from the Rust serde
    /// rename so the string can never drift from the JSON schema.
    #[getter]
    fn kind(&self) -> String {
        crate::bindings::statements_analytics::serde_variant_str(&self.inner)
    }

    /// Return the canonical snake_case discriminant (same as ``kind``).
    fn __str__(&self) -> String {
        self.kind()
    }

    /// Return ``ForecastMethod('growth_pct')`` style representation.
    fn __repr__(&self) -> String {
        format!("ForecastMethod({:?})", self.kind())
    }
}

/// Forecast configuration for a statement model node.
#[pyclass(
    name = "ForecastSpec",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyForecastSpec {
    pub(super) inner: finstack_quant_statements::types::ForecastSpec,
}

impl PyForecastSpec {
    fn wrap(inner: finstack_quant_statements::types::ForecastSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyForecastSpec {
    /// Build a forecast spec from a method plus its method-specific parameters.
    ///
    /// The parameter map is validated on construction: keys the method does
    /// not understand and values of the wrong type (a string ``rate``, a
    /// non-list ``curve``) raise ``ValueError`` here rather than at
    /// evaluation.
    ///
    /// Parameters
    /// ----------
    /// method : ForecastMethod
    ///     Projection rule applied to forecast periods.
    /// params : dict | str | None
    ///     Method-specific parameters as a ``dict`` or a JSON object string
    ///     (``rate``, ``curve``, ``mean``, ``std_dev``, ``seed``,
    ///     ``overrides``, ``season_length``, ``target``, ``shape``,
    ///     ``half_life``, ``long_run_mean``, ``reversion_speed``,
    ///     ``historical``, ``mode``, ``phi``, ...). Rates are decimal
    ///     fractions per period. Every method also accepts optional
    ///     ``min`` / ``max`` bounds that clamp generated values to a band
    ///     (in the node's own units). ``None`` means no parameters, which
    ///     is only valid for ``forward_fill``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``params`` is not a JSON object / dict, names a parameter the
    ///     method does not accept, or carries a value of the wrong type.
    #[new]
    #[pyo3(signature = (method, params=None), text_signature = "(method, params=None)")]
    fn new(
        py: Python<'_>,
        method: PyRef<'_, PyForecastMethod>,
        params: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let params = parse_params(py, params)?;
        let inner = finstack_quant_statements::types::ForecastSpec {
            method: method.inner,
            params,
        };
        inner.validate().map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    /// Forward-fill spec: hold the last observed value flat, no parameters.
    #[staticmethod]
    fn forward_fill() -> Self {
        Self::wrap(finstack_quant_statements::types::ForecastSpec::forward_fill())
    }

    /// Constant compound-growth spec: ``v[t] = v[t-1] * (1 + rate)``.
    ///
    /// Parameters
    /// ----------
    /// rate : float
    ///     Growth rate as a **decimal fraction per period** (0.05 = 5% per
    ///     period, on the model's own period cadence). Negative values decay
    ///     the series.
    #[staticmethod]
    fn growth(rate: f64) -> Self {
        Self::wrap(finstack_quant_statements::types::ForecastSpec::growth(rate))
    }

    /// Period-by-period growth spec: ``v[t] = v[t-1] * (1 + curve[t])``.
    ///
    /// Parameters
    /// ----------
    /// curve : list[float]
    ///     One growth rate per forecast period, each a decimal fraction per
    ///     period (0.05 = 5%), consumed in forecast-period order.
    #[staticmethod]
    fn curve(curve: Vec<f64>) -> Self {
        Self::wrap(finstack_quant_statements::types::ForecastSpec::curve(curve))
    }

    /// Additive normal random-walk spec:
    /// ``v[t] = v[t-1] + mean + std_dev * z[t]``.
    ///
    /// Parameters
    /// ----------
    /// mean : float
    ///     Per-period additive drift, in the node's own units (currency
    ///     amount for ``Money`` nodes, unitless for scalar nodes).
    /// std_dev : float
    ///     Per-period additive volatility in the same units; must be
    ///     non-negative. Zero gives a deterministic drift path.
    /// seed : int
    ///     Seed for the deterministic standard-normal draws. The evaluator
    ///     mixes a stable hash of the node id into it, so two nodes sharing a
    ///     seed still receive independent shocks.
    #[staticmethod]
    fn normal(mean: f64, std_dev: f64, seed: u64) -> Self {
        Self::wrap(finstack_quant_statements::types::ForecastSpec::normal(
            mean, std_dev, seed,
        ))
    }

    /// Multiplicative log-normal spec:
    /// ``v[t] = v[t-1] * exp(mean - 0.5 * std_dev**2 + std_dev * z[t])``.
    ///
    /// Parameters
    /// ----------
    /// mean : float
    ///     Per-period continuously compounded expected-growth drift. The
    ///     expected growth factor is ``exp(mean)`` and expected log increment
    ///     is ``mean - 0.5 * std_dev**2``. For example, ``mean=0.02`` gives
    ///     expected simple growth ``exp(0.02) - 1``, approximately 2.0201%.
    /// std_dev : float
    ///     Per-period log-return volatility as a decimal fraction; must be
    ///     non-negative.
    /// seed : int
    ///     Seed for the deterministic standard-normal draws.
    ///
    /// Notes
    /// -----
    /// This constructor does not raise; it only stores the supplied parameters.
    /// Evaluation requires a finite, strictly positive base value and finite
    /// parameters; invalid inputs raise ``ValueError`` when the model is
    /// validated or evaluated.
    #[staticmethod]
    fn log_normal(mean: f64, std_dev: f64, seed: u64) -> Self {
        Self::wrap(finstack_quant_statements::types::ForecastSpec::log_normal(
            mean, std_dev, seed,
        ))
    }

    /// Explicit per-period override spec.
    ///
    /// Forecast periods listed in ``overrides`` take the supplied value; any
    /// forecast period without an entry inherits the most recent value
    /// (forward fill), so a sparse mapping pins a few anchor periods.
    ///
    /// Parameters
    /// ----------
    /// overrides : Mapping[str, float] | Sequence[tuple[str, float]] | pd.Series
    ///     Period identifier (``"2025Q3"``) to value, in the node's own
    ///     units (a currency amount for monetary nodes, a decimal ratio for
    ///     scalar nodes). Periods must belong to the model timeline.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a period identifier does not parse or a value is not numeric.
    #[staticmethod]
    #[pyo3(name = "override", text_signature = "(overrides)")]
    fn override_(overrides: &Bound<'_, PyAny>) -> PyResult<Self> {
        let pairs = super::extract_scalar_series(overrides)?;
        let map: IndexMap<finstack_quant_core::dates::PeriodId, f64> = pairs.into_iter().collect();
        Ok(Self::wrap(
            finstack_quant_statements::types::ForecastSpec::overrides(map),
        ))
    }

    /// Seasonal-decomposition spec over an external history.
    ///
    /// The history is split into trend and a repeating seasonal pattern of
    /// ``season_length`` periods, then projected forward.
    ///
    /// Parameters
    /// ----------
    /// historical : list[float]
    ///     Historical values in the node's own units, oldest first; must
    ///     cover at least ``2 * season_length`` periods.
    /// season_length : int
    ///     Length of one seasonal cycle counted in **model periods** (4 for
    ///     quarterly, 12 for monthly data); must be positive.
    /// mode : str, default "additive"
    ///     ``"additive"`` (constant seasonal swings; safe for series that
    ///     cross zero) or ``"multiplicative"`` (swings scale with the level).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``mode`` is not ``"additive"`` or ``"multiplicative"``.
    #[staticmethod]
    #[pyo3(signature = (historical, season_length, mode="additive"), text_signature = "(historical, season_length, mode='additive')")]
    fn seasonal(historical: Vec<f64>, season_length: usize, mode: &str) -> PyResult<Self> {
        let mode: finstack_quant_statements::types::SeasonalMode =
            finstack_quant_core::wire::serde_parse(mode).map_err(|e| {
                crate::errors::value_error(format!(
                    "invalid seasonal mode {mode:?}: {e}; expected additive or multiplicative"
                ))
            })?;
        Ok(Self::wrap(
            finstack_quant_statements::types::ForecastSpec::seasonal(
                historical,
                season_length,
                mode,
            ),
        ))
    }

    /// Trend-detection time-series spec over an external history (linear
    /// trend by default).
    ///
    /// For Holt / damped-Holt or moving-average projections construct
    /// ``ForecastSpec(ForecastMethod.time_series(), {...})`` with ``method``
    /// (``"linear"``, ``"exponential"``, ``"moving_average"``) and its
    /// tuning parameters (``alpha``, ``beta``, ``phi``, ``window``).
    ///
    /// Parameters
    /// ----------
    /// historical : list[float]
    ///     At least 2 historical values in the node's own units, oldest
    ///     first, from which the trend is estimated.
    #[staticmethod]
    fn time_series(historical: Vec<f64>) -> Self {
        Self::wrap(finstack_quant_statements::types::ForecastSpec::time_series(
            historical,
        ))
    }

    /// Linear fade-to-target spec:
    /// ``v[t] = base + (target - base) * t / N``.
    ///
    /// Values glide from the last observed value to ``target`` in equal
    /// steps, reaching it exactly at the final forecast period. For the
    /// ``"geometric"`` (CAGR-to-terminal) or ``"exponential"`` (half-life)
    /// shapes, construct ``ForecastSpec(ForecastMethod.fade_to_target(),
    /// {...})`` with ``shape`` and, for exponential, ``half_life``.
    ///
    /// Parameters
    /// ----------
    /// target : float
    ///     Terminal level in the node's own units (a currency amount for
    ///     monetary nodes, a decimal ratio for scalar nodes such as margins).
    #[staticmethod]
    fn fade_to_target(target: f64) -> Self {
        Self::wrap(finstack_quant_statements::types::ForecastSpec::fade_to_target(target))
    }

    /// Mean-reverting AR(1) spec:
    /// ``v[t] = v[t-1] + reversion_speed * (long_run_mean - v[t-1]) + std_dev * z[t]``.
    ///
    /// Parameters
    /// ----------
    /// long_run_mean : float
    ///     Level the series reverts toward, in the node's own units
    ///     (currency amount for monetary nodes, decimal ratio for scalars).
    /// reversion_speed : float
    ///     Fraction of the gap closed each period, in ``(0, 1]``; 1.0
    ///     reverts fully every period, values near 0 revert slowly.
    /// std_dev : float
    ///     Per-period additive shock volatility in the node's own units;
    ///     must be non-negative. Zero gives deterministic geometric decay of
    ///     the gap.
    /// seed : int
    ///     Seed for the deterministic standard-normal draws. The evaluator
    ///     mixes a stable hash of the node id into it, so two nodes sharing
    ///     a seed still receive independent shocks.
    #[staticmethod]
    fn mean_reverting(long_run_mean: f64, reversion_speed: f64, std_dev: f64, seed: u64) -> Self {
        Self::wrap(
            finstack_quant_statements::types::ForecastSpec::mean_reverting(
                long_run_mean,
                reversion_speed,
                std_dev,
                seed,
            ),
        )
    }

    /// Growth-mode bootstrap spec: resample historical growth rates and
    /// compound them from the base value.
    ///
    /// The history must be strictly positive in growth mode. For additive
    /// resampling of level changes (series that cross zero), construct
    /// ``ForecastSpec(ForecastMethod.bootstrap(), {...})`` with
    /// ``mode = "diff"``.
    ///
    /// Parameters
    /// ----------
    /// historical : list[float]
    ///     At least 2 historical values in the node's own units, oldest
    ///     first; consecutive pairs define the resampled growth rates.
    /// seed : int
    ///     Seed for the deterministic resampling draws.
    #[staticmethod]
    fn bootstrap(historical: Vec<f64>, seed: u64) -> Self {
        Self::wrap(finstack_quant_statements::types::ForecastSpec::bootstrap(
            historical, seed,
        ))
    }

    /// Projection rule this spec applies.
    #[getter]
    fn method(&self) -> PyForecastMethod {
        PyForecastMethod {
            inner: self.inner.method,
        }
    }

    /// Method-specific parameters as a plain ``dict`` (JSON-shaped values).
    ///
    /// Returns
    /// -------
    /// dict[str, object]
    ///     Parameter name to value, in insertion order — numbers for rates
    ///     and levels, lists for ``curve`` / ``historical``, a nested dict
    ///     for ``overrides``.
    #[getter]
    fn params<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.params)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a forecast spec from its canonical JSON form.
    ///
    /// Unknown fields are rejected, so a typo in ``method`` or ``params``
    /// fails loudly rather than being silently dropped.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid ForecastSpec JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this forecast spec to canonical JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize ForecastSpec"))
    }

    /// Return ``ForecastSpec(method='growth_pct', params={'rate': 0.05})``.
    fn __repr__(&self) -> String {
        let params = serde_json::Value::Object(
            self.inner
                .params
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );
        format!(
            "ForecastSpec(method={:?}, params={})",
            crate::bindings::statements_analytics::serde_variant_str(&self.inner.method),
            super::python_literal(&params)
        )
    }
}

/// Parse the ``params`` argument of ``ForecastSpec`` from a dict, a JSON
/// object string, or ``None``.
fn parse_params(
    py: Python<'_>,
    params: Option<&Bound<'_, PyAny>>,
) -> PyResult<IndexMap<String, serde_json::Value>> {
    let Some(params) = params else {
        return Ok(IndexMap::new());
    };
    let value = crate::bindings::module_utils::py_to_json_value(py, params, "forecast params")?;
    match value {
        serde_json::Value::Object(map) => Ok(map.into_iter().collect()),
        other => Err(crate::errors::value_error(format!(
            "forecast params must be a dict or JSON object, got {other}"
        ))),
    }
}

/// Node computation type.
#[pyclass(
    name = "NodeType",
    module = "finstack_quant.statements",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyNodeType {
    pub(super) inner: finstack_quant_statements::types::NodeType,
}

#[pymethods]
impl PyNodeType {
    /// Value node: explicit per-period values only (actuals, assumptions).
    ///
    /// The node has no formula and no forecast, so periods without an
    /// explicit value stay unset.
    #[staticmethod]
    fn value() -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeType::Value,
        }
    }

    /// Calculated node: derived from its formula in every period.
    #[staticmethod]
    fn calculated() -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeType::Calculated,
        }
    }

    /// Mixed node: explicit value, else forecast, else formula.
    ///
    /// This is the crate's core precedence invariant — **Value > Forecast >
    /// Formula** — resolved independently for each period, so a node can be
    /// an actual in historical periods, a forecast in projected periods, and
    /// a formula wherever neither is supplied.
    #[staticmethod]
    fn mixed() -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeType::Mixed,
        }
    }

    /// The canonical snake_case wire discriminant (``"value"``,
    /// ``"calculated"``, or ``"mixed"``), derived from the Rust serde
    /// rename so the string can never drift from the JSON schema.
    #[getter]
    fn kind(&self) -> String {
        crate::bindings::statements_analytics::serde_variant_str(&self.inner)
    }

    /// Return the canonical snake_case discriminant (same as :attr:`kind`).
    fn __str__(&self) -> String {
        self.kind()
    }

    /// Return ``NodeType('mixed')`` style representation.
    fn __repr__(&self) -> String {
        format!("NodeType({:?})", self.kind())
    }
}

/// Type-safe identifier for a node in a financial model.
#[pyclass(
    name = "NodeId",
    module = "finstack_quant.statements",
    eq,
    hash,
    frozen,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyNodeId {
    pub(super) inner: finstack_quant_statements::types::NodeId,
}

#[pymethods]
impl PyNodeId {
    /// Wrap a raw node identifier string.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Node identifier as it appears in the model graph and in formulas
    ///     (e.g. ``"revenue"``). The value is stored verbatim — no case
    ///     folding or trimming — so identifiers are matched exactly.
    #[new]
    #[pyo3(text_signature = "(id)")]
    fn new(id: &str) -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeId::new(id),
        }
    }

    /// Return the underlying identifier string.
    ///
    /// Returns
    /// -------
    /// str
    ///     The identifier exactly as supplied at construction.
    #[pyo3(text_signature = "($self)")]
    fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Return the representation, e.g. ``NodeId('revenue')``.
    fn __repr__(&self) -> String {
        format!("NodeId({:?})", self.inner.as_str())
    }

    /// Return the bare identifier string (no quoting or wrapper).
    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Numeric evaluation mode.
#[pyclass(
    name = "NumericMode",
    module = "finstack_quant.statements",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyNumericMode {
    pub(super) inner: finstack_quant_statements::evaluator::NumericMode,
}

#[pymethods]
impl PyNumericMode {
    /// f64 floating-point evaluation — the mode the evaluator actually emits.
    ///
    /// Statement arithmetic runs in f64, so identity checks carry rounding
    /// proportional to the magnitudes involved; that is why check tolerances
    /// blend an absolute floor with a relative component.
    #[staticmethod]
    fn float64() -> Self {
        Self {
            inner: finstack_quant_statements::evaluator::NumericMode::Float64,
        }
    }

    /// Return ``NumericMode('float64')`` style representation.
    fn __repr__(&self) -> String {
        format!(
            "NumericMode({:?})",
            crate::bindings::statements_analytics::serde_variant_str(&self.inner)
        )
    }
}

/// Specification of a single node (line item / metric) in a financial model.
///
/// A node is one of three kinds: a **value** node (explicit per-period
/// data), a **calculated** node (formula only), or a **mixed** node that
/// resolves each period as *Value > Forecast > Formula*. Nodes are normally
/// produced by ``ModelBuilder``; construct one directly for
/// ``ModelBuilder.insert_node`` when a template needs full control.
#[pyclass(
    name = "NodeSpec",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyNodeSpec {
    pub(crate) inner: NodeSpec,
}

#[pymethods]
impl PyNodeSpec {
    /// Build a node specification.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Unique node identifier as referenced from formulas.
    /// node_type : NodeType
    ///     ``NodeType.value()``, ``NodeType.calculated()`` or
    ///     ``NodeType.mixed()``; determines which of the other fields may be
    ///     set (a value node cannot carry a formula, a calculated node cannot
    ///     carry values or a forecast).
    /// name : str | None
    ///     Optional human-readable label used in reports.
    /// values : Mapping[str, float | Money] | Sequence[tuple[str, float | Money]] | pd.Series | None
    ///     Explicit per-period values keyed by period id (``"2025Q1"``);
    ///     ``Money`` cells make the node monetary in that currency, floats
    ///     make it scalar. Mixing is rejected at model build.
    /// forecast : ForecastSpec | None
    ///     Projection rule for forecast periods (mixed nodes).
    /// formula_text : str | None
    ///     Statements DSL expression (calculated and mixed nodes).
    /// where_text : str | None
    ///     Optional DSL predicate; periods where it is false evaluate to 0.
    /// tags : list[str] | None
    ///     Free-form labels for grouping and filtering.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a period id does not parse or a value is neither numeric nor
    ///     ``Money``.
    #[new]
    #[pyo3(
        signature = (node_id, node_type, name=None, values=None, forecast=None, formula_text=None, where_text=None, tags=None),
        text_signature = "(node_id, node_type, name=None, values=None, forecast=None, formula_text=None, where_text=None, tags=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        node_id: &str,
        node_type: PyRef<'_, PyNodeType>,
        name: Option<String>,
        values: Option<&Bound<'_, PyAny>>,
        forecast: Option<PyRef<'_, PyForecastSpec>>,
        formula_text: Option<String>,
        where_text: Option<String>,
        tags: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let mut inner = NodeSpec::new(node_id, node_type.inner);
        inner.name = name;
        if let Some(values) = values {
            let pairs = super::extract_value_series(values)?;
            inner.values = Some(pairs.into_iter().collect());
        }
        inner.forecast = forecast.map(|f| f.inner.clone());
        inner.formula_text = formula_text;
        inner.where_text = where_text;
        inner.tags = tags.unwrap_or_default();
        Ok(Self { inner })
    }

    /// Support `pickle` via the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a node spec from its canonical JSON form (unknown fields
    /// are rejected).
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner =
            serde_json::from_str(json).map_err(|e| serde_json_to_py(e, "invalid NodeSpec JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this node spec to canonical JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize NodeSpec"))
    }

    /// Node identifier.
    #[getter]
    fn node_id(&self) -> &str {
        self.inner.node_id.as_str()
    }

    /// Human-readable name, or ``None``.
    #[getter]
    fn name(&self) -> Option<&str> {
        self.inner.name.as_deref()
    }

    /// Computation type (value / calculated / mixed).
    #[getter]
    fn node_type(&self) -> PyNodeType {
        PyNodeType {
            inner: self.inner.node_type,
        }
    }

    /// Explicit per-period values as floats, or ``None`` when the node
    /// carries no explicit data.
    ///
    /// Returns
    /// -------
    /// dict[str, float] | None
    ///     Period id to value in the node's own units; monetary amounts are
    ///     returned as their float amount (see :attr:`currency`).
    #[getter]
    fn values<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(values) = &self.inner.values else {
            return Ok(None);
        };
        let dict = PyDict::new(py);
        for (period, value) in values {
            dict.set_item(period.to_string(), value.value())?;
        }
        Ok(Some(dict))
    }

    /// Point-in-time availability dates for explicit observations.
    ///
    /// Returns
    /// -------
    /// dict[str, datetime.date]
    ///     Period id to the date the observation became available; empty
    ///     when every observation defaults to its period's exclusive end.
    #[getter]
    fn availability_dates<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (period, date) in &self.inner.availability_dates {
            dict.set_item(period.to_string(), date_to_py(py, *date)?)?;
        }
        Ok(dict)
    }

    /// Forecast specification, or ``None``.
    #[getter]
    fn forecast(&self) -> Option<PyForecastSpec> {
        self.inner
            .forecast
            .clone()
            .map(|inner| PyForecastSpec { inner })
    }

    /// Statements DSL formula, or ``None`` for value-only nodes.
    #[getter]
    fn formula_text(&self) -> Option<&str> {
        self.inner.formula_text.as_deref()
    }

    /// Conditional ``where`` predicate, or ``None``.
    #[getter]
    fn where_text(&self) -> Option<&str> {
        self.inner.where_text.as_deref()
    }

    /// Free-form tags.
    #[getter]
    fn tags(&self) -> Vec<String> {
        self.inner.tags.clone()
    }

    /// Declared or inferred value type: ``"monetary"``, ``"scalar"``, or
    /// ``None`` when not yet resolved (resolved at model build).
    #[getter]
    fn value_type(&self) -> Option<&'static str> {
        self.inner.value_type.map(|value_type| match value_type {
            NodeValueType::Monetary { .. } => "monetary",
            NodeValueType::Scalar => "scalar",
        })
    }

    /// ISO-4217 currency code for monetary nodes, else ``None``.
    #[getter]
    fn currency(&self) -> Option<String> {
        match self.inner.value_type {
            Some(NodeValueType::Monetary { currency }) => Some(currency.to_string()),
            _ => None,
        }
    }

    /// Node-level metadata as a plain ``dict``.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Return ``NodeSpec(node_id='revenue', node_type='mixed', ...)``.
    fn __repr__(&self) -> String {
        format!(
            "NodeSpec(node_id={:?}, node_type={:?}, values={}, forecast={}, formula_text={})",
            self.inner.node_id.as_str(),
            crate::bindings::statements_analytics::serde_variant_str(&self.inner.node_type),
            self.inner.values.as_ref().map_or(0, IndexMap::len),
            self.inner.forecast.is_some(),
            self.inner
                .formula_text
                .as_deref()
                .map_or_else(|| "None".to_string(), |f| format!("{f:?}")),
        )
    }
}

/// Top-level financial model specification.
#[pyclass(
    name = "FinancialModelSpec",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFinancialModelSpec {
    pub(crate) inner: finstack_quant_statements::FinancialModelSpec,
}

impl PyFinancialModelSpec {
    pub(crate) fn from_inner(inner: finstack_quant_statements::FinancialModelSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFinancialModelSpec {
    /// Start a staged model build.
    ///
    /// The canonical entry point, mirroring Rust's
    /// `FinancialModelSpec::builder(id)` and the ``Type.builder()`` form every
    /// other builder-backed type uses. Constructing
    /// :class:`~finstack_quant.statements.ModelBuilder` directly is equivalent.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Stable model identifier.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     A fresh builder awaiting ``periods(...)``.
    #[staticmethod]
    #[pyo3(text_signature = "(id)")]
    fn builder(id: &str) -> crate::bindings::statements::builder::PyModelBuilder {
        crate::bindings::statements::builder::PyModelBuilder::start(id)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from a JSON string and run semantic validation.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed, or the model violates a semantic
    ///     invariant (empty timeline, reserved node id, invalid formula,
    ///     mixed currencies, invalid waterfall).
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let mut inner: finstack_quant_statements::FinancialModelSpec =
            serde_json::from_str(json)
                .map_err(|e| serde_json_to_py(e, "invalid FinancialModelSpec JSON"))?;
        inner.validate_semantics().map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize FinancialModelSpec"))
    }

    /// Unique model identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Number of periods in the model timeline.
    ///
    /// Counted in **periods** on the model's own cadence (quarters, months,
    /// years), not months. Their declared order *is* the evaluation timeline.
    #[getter]
    fn period_count(&self) -> usize {
        self.inner.periods.len()
    }

    /// Period identifiers in timeline order (``["2025Q1", "2025Q2", ...]``).
    #[getter]
    fn periods(&self) -> Vec<String> {
        self.inner
            .periods
            .iter()
            .map(|p| p.id.to_string())
            .collect()
    }

    /// Period identifiers flagged as actuals (historical), in timeline order.
    #[getter]
    fn actual_periods(&self) -> Vec<String> {
        self.inner
            .periods
            .iter()
            .filter(|p| p.is_actual)
            .map(|p| p.id.to_string())
            .collect()
    }

    /// Period identifiers flagged as forecasts, in timeline order.
    #[getter]
    fn forecast_periods(&self) -> Vec<String> {
        self.inner
            .periods
            .iter()
            .filter(|p| !p.is_actual)
            .map(|p| p.id.to_string())
            .collect()
    }

    /// Number of nodes (line items / metrics) declared in the model.
    #[getter]
    fn node_count(&self) -> usize {
        self.inner.nodes.len()
    }

    /// Node identifiers in declaration order.
    #[pyo3(text_signature = "($self)")]
    fn node_ids(&self) -> Vec<String> {
        self.inner.nodes.keys().map(|k| k.to_string()).collect()
    }

    /// Whether the model has a node with the given ID.
    #[pyo3(text_signature = "($self, node_id)")]
    fn has_node(&self, node_id: &str) -> bool {
        self.inner.has_node(node_id)
    }

    /// Look up one node specification.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier.
    ///
    /// Returns
    /// -------
    /// NodeSpec | None
    ///     The node's specification (type, values, forecast, formula, ...),
    ///     or ``None`` when the model has no such node.
    #[pyo3(text_signature = "($self, node_id)")]
    fn get_node(&self, node_id: &str) -> Option<PyNodeSpec> {
        self.inner.get_node(node_id).map(|node| PyNodeSpec {
            inner: node.clone(),
        })
    }

    /// All node specifications in declaration order.
    #[getter]
    fn nodes(&self) -> Vec<PyNodeSpec> {
        self.inner
            .nodes
            .values()
            .map(|node| PyNodeSpec {
                inner: node.clone(),
            })
            .collect()
    }

    /// Model-level metadata as a plain ``dict`` (e.g. ``{"currency": "USD"}``).
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Capital-structure specification in its JSON shape, or ``None``.
    ///
    /// Returns
    /// -------
    /// dict | None
    ///     ``{"debt_instruments": [{"id", "spec": {"type", "spec"}}],
    ///     "reporting_currency", "fx_policy", "waterfall", "meta"}`` as
    ///     plain Python containers, matching ``to_json()``.
    #[getter]
    fn capital_structure<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .capital_structure
            .as_ref()
            .map(|cs| serde_to_py(py, cs))
            .transpose()
    }

    /// Versioned SHA-256 hash of the model's canonical JSON.
    ///
    /// Returns
    /// -------
    /// str
    ///     Stable content hash; two models with identical canonical JSON
    ///     share a hash regardless of key order.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the model contains a non-finite number.
    #[pyo3(text_signature = "($self)")]
    fn content_hash(&self) -> PyResult<String> {
        self.inner.content_hash().map_err(core_to_py)
    }

    /// Wire-format schema version of this model spec.
    ///
    /// Only version ``1`` is accepted today; the field exists so persisted
    /// models can be migrated rather than silently misread.
    #[getter]
    fn schema_version(&self) -> u32 {
        self.inner.schema_version.into()
    }

    /// Export one row per node as a pandas ``DataFrame``.
    ///
    /// Columns: ``node_id``, ``node_type`` (``value`` / ``calculated`` /
    /// ``mixed``), ``name``, ``formula_text``, ``forecast_method`` (snake
    /// case method name or ``None``), ``value_type`` (``monetary`` /
    /// ``scalar`` / ``None``), ``currency`` (ISO code for monetary nodes),
    /// ``where_text``. Rows follow declaration order.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .nodes
            .values()
            .map(|node| {
                let (value_type, currency) = match node.value_type {
                    Some(NodeValueType::Monetary { currency }) => {
                        (Some("monetary"), Some(currency.to_string()))
                    }
                    Some(NodeValueType::Scalar) => (Some("scalar"), None),
                    None => (None, None),
                };
                serde_json::json!({
                    "node_id": node.node_id.as_str(),
                    "node_type": crate::bindings::statements_analytics::serde_variant_str(&node.node_type),
                    "name": node.name,
                    "formula_text": node.formula_text,
                    "forecast_method": node.forecast.as_ref().map(|f| {
                        crate::bindings::statements_analytics::serde_variant_str(&f.method)
                    }),
                    "value_type": value_type,
                    "currency": currency,
                    "where_text": node.where_text,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &NODE_COLUMNS)
    }

    /// Return the representation with the id, period and node counts.
    fn __repr__(&self) -> String {
        format!(
            "FinancialModelSpec(id={:?}, periods={}, nodes={})",
            self.inner.id,
            self.inner.periods.len(),
            self.inner.nodes.len()
        )
    }
}

/// Register type classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyForecastMethod>()?;
    m.add_class::<PyForecastSpec>()?;
    m.add_class::<PyNodeType>()?;
    m.add_class::<PyNodeId>()?;
    m.add_class::<PyNodeSpec>()?;
    m.add_class::<PyNumericMode>()?;
    m.add_class::<PyFinancialModelSpec>()?;
    Ok(())
}
