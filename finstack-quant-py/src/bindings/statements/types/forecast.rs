//! Forecast method and forecast specification Python bindings.

use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use crate::errors::statements_to_py;
use indexmap::IndexMap;
use pyo3::prelude::*;

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
    pub(crate) inner: finstack_quant_statements::types::ForecastMethod,
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

    #[doc = pydoc!("finstack_quant.statements._types.forecast", "ForecastMethod.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        crate::bindings::extract::label(&self.inner)
    }

    /// Return the canonical snake_case discriminant (same as ``kind``).
    fn __str__(&self) -> PyResult<String> {
        self.kind()
    }

    /// Return ``ForecastMethod('growth_pct')`` style representation.
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("ForecastMethod({:?})", self.kind()?))
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
    pub(crate) inner: finstack_quant_statements::types::ForecastSpec,
}

impl PyForecastSpec {
    fn wrap(inner: finstack_quant_statements::types::ForecastSpec) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyForecastSpec,
    finstack_quant_statements::types::ForecastSpec,
    "ForecastSpec"
);

#[pymethods]
impl PyForecastSpec {
    /// Build a forecast spec from a method plus its method-specific parameters.
    ///
    /// The parameter map is validated on construction: keys the method does
    /// not understand, missing or ``None`` parameters and values of the wrong
    /// type (a string ``rate``, a non-list ``curve``) raise ``ValueError`` here
    /// rather than at evaluation.
    ///
    /// Parameters
    /// ----------
    /// method : ForecastMethod
    ///     Projection rule applied to forecast periods.
    /// params : dict | None
    ///     Method-specific parameters as a ``dict``
    ///     (``rate``, ``curve``, ``mean``, ``std_dev``, ``seed``,
    ///     ``overrides``, ``season_length``, ``target``, ``shape``,
    ///     ``half_life``, ``long_run_mean``, ``reversion_speed``,
    ///     ``historical``, ``mode``, ``phi``, ...). Rates are decimal
    ///     fractions per period. Every method also accepts optional
    ///     ``min`` / ``max`` bounds that clamp the published values to a
    ///     band (in the node's own units), and methods that step from a
    ///     base accept an optional ``start`` level. ``None`` means no
    ///     parameters, which is only valid for ``forward_fill``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``params`` is not a JSON object / dict, names a parameter the
    ///     method does not accept, omits a required one, sets one to ``None``,
    ///     or carries a value of the wrong type.
    #[new]
    #[pyo3(signature = (method, params=None), text_signature = "(method, params=None)")]
    fn new(
        py: Python<'_>,
        method: PyRef<'_, PyForecastMethod>,
        params: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let params = parse_params(py, params)?;
        let inner =
            finstack_quant_statements::types::ForecastSpec::from_params(method.inner, params)
                .map_err(statements_to_py)?;
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

    #[doc = pydoc!("finstack_quant.statements._types.forecast", "ForecastSpec.overrides")]
    #[staticmethod]
    #[pyo3(text_signature = "(overrides)")]
    fn overrides(overrides: &Bound<'_, PyAny>) -> PyResult<Self> {
        let pairs = super::super::extract_scalar_series(overrides)?;
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
    /// mode : str
    ///     Required. ``"additive"`` (constant seasonal swings; safe for series
    ///     that cross zero) or ``"multiplicative"`` (swings scale with the
    ///     level).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``mode`` is not ``"additive"`` or ``"multiplicative"``.
    #[staticmethod]
    #[pyo3(signature = (historical, season_length, mode), text_signature = "(historical, season_length, mode)")]
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

    /// Time-series spec over an external history with the chosen trend
    /// method.
    ///
    /// Tuning parameters (``alpha``, ``beta``, ``phi`` for ``"exponential"``;
    /// ``window`` for ``"moving_average"``) are set through
    /// ``ForecastSpec(ForecastMethod.time_series(), {...})``.
    ///
    /// Parameters
    /// ----------
    /// historical : list[float]
    ///     At least 2 historical values in the node's own units, oldest
    ///     first, from which the trend is estimated.
    /// method : {"linear", "exponential", "moving_average"}
    ///     Trend method. There is no default.
    #[staticmethod]
    fn time_series(historical: Vec<f64>, method: &str) -> PyResult<Self> {
        Ok(Self::wrap(
            finstack_quant_statements::types::ForecastSpec::time_series(
                historical,
                choice(method, "method")?,
            ),
        ))
    }

    /// Fade-to-target spec with the chosen shape.
    ///
    /// Values glide from the last observed value to ``target``. The
    /// ``"linear"`` shape moves in equal steps and reaches the target at the
    /// final forecast period; ``"geometric"`` compounds toward it;
    /// ``"exponential"`` closes the gap with a ``half_life`` set through
    /// ``ForecastSpec(ForecastMethod.fade_to_target(), {...})``.
    ///
    /// Parameters
    /// ----------
    /// target : float
    ///     Terminal level in the node's own units (a currency amount for
    ///     monetary nodes, a decimal ratio for scalar nodes such as margins).
    /// shape : {"linear", "geometric", "exponential"}
    ///     Path of the fade. There is no default.
    #[staticmethod]
    fn fade_to_target(target: f64, shape: &str) -> PyResult<Self> {
        Ok(Self::wrap(
            finstack_quant_statements::types::ForecastSpec::fade_to_target(
                target,
                choice(shape, "shape")?,
            ),
        ))
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

    /// Bootstrap spec: resample historical changes and apply them from the
    /// base value.
    ///
    /// ``"growth"`` resamples growth rates and compounds them, which needs a
    /// strictly positive history; ``"diff"`` resamples level changes and
    /// suits series that cross zero.
    ///
    /// Parameters
    /// ----------
    /// historical : list[float]
    ///     At least 2 historical values in the node's own units, oldest
    ///     first; consecutive pairs define the resampled changes.
    /// mode : {"growth", "diff"}
    ///     What is resampled. There is no default.
    /// seed : int
    ///     Exact unsigned 64-bit seed for the resampling draws.
    #[staticmethod]
    fn bootstrap(historical: Vec<f64>, mode: &str, seed: u64) -> PyResult<Self> {
        Ok(Self::wrap(
            finstack_quant_statements::types::ForecastSpec::bootstrap(
                historical,
                choice(mode, "mode")?,
                seed,
            ),
        ))
    }

    /// Projection rule this spec applies.
    #[getter]
    fn method(&self) -> PyForecastMethod {
        PyForecastMethod {
            inner: self.inner.method(),
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
        serde_to_py(py, &python_params(&self.inner))
    }

    /// Return ``ForecastSpec(method='growth_pct', params={'rate': 0.05})``.
    fn __repr__(&self) -> PyResult<String> {
        let params = serde_json::Value::Object(python_params(&self.inner).into_iter().collect());
        Ok(format!(
            "ForecastSpec(method={:?}, params={})",
            crate::bindings::extract::label(&self.inner.method())?,
            super::super::python_literal(&params)
        ))
    }
}

/// A forecast's parameters as Python reads them: the wire object with the
/// seed as an integer.
fn python_params(
    spec: &finstack_quant_statements::types::ForecastSpec,
) -> IndexMap<String, serde_json::Value> {
    let mut params = spec.params();
    if let Some(seed) = params.get_mut("seed")
        && let Some(exact) = seed.as_str().and_then(|text| text.parse::<u64>().ok())
    {
        *seed = serde_json::Value::from(exact);
    }
    params
}

/// Parse one of a forecast method's named choices (a trend method, fade
/// shape or bootstrap mode) from its wire label.
fn choice<T: serde::de::DeserializeOwned>(label: &str, parameter: &str) -> PyResult<T> {
    serde_json::from_value(serde_json::Value::String(label.to_string())).map_err(|error| {
        crate::errors::value_error(format!("invalid forecast {parameter} '{label}': {error}"))
    })
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
        serde_json::Value::Object(map) => Ok(map
            .into_iter()
            .map(|(key, value)| {
                // A Python seed is an int; the wire form is its decimal text.
                let value = match (key.as_str(), value.as_u64()) {
                    ("seed", Some(seed)) => serde_json::Value::String(seed.to_string()),
                    _ => value,
                };
                (key, value)
            })
            .collect()),
        other => Err(crate::errors::value_error(format!(
            "forecast params must be a dict, got {other}"
        ))),
    }
}
