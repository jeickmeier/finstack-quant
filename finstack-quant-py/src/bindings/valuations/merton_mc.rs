//! Python bindings for Merton Monte Carlo PIK bond pricing types.

use crate::bindings::models::credit::{
    PyDynamicRecoverySpec, PyEndogenousHazardSpec, PyMertonModel, PyToggleExerciseModel,
};
use crate::errors::{core_to_py, display_to_py};
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::merton_mc::{
    BarrierCrossing, MertonMcCalibrationSpec, MertonMcConfig, MertonMcResult, PathStatistics,
    PikMode, PikSchedule,
};
use pyo3::prelude::*;
use pyo3::types::PyList;

/// Per-coupon PIK behavior for the Merton Monte Carlo engine.
#[pyclass(
    name = "PikMode",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
#[derive(Clone, Copy)]
pub(crate) struct PyPikMode {
    pub(crate) inner: PikMode,
}

impl PyPikMode {
    pub(crate) fn from_inner(inner: PikMode) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPikMode {
    /// Coupon paid entirely in cash.
    #[staticmethod]
    fn cash() -> Self {
        Self {
            inner: PikMode::Cash,
        }
    }

    /// Coupon accreted to notional (payment-in-kind).
    #[staticmethod]
    fn pik() -> Self {
        Self {
            inner: PikMode::Pik,
        }
    }

    /// Coupon split between cash and PIK accretion.
    ///
    /// # Arguments
    ///
    /// * `cash_fraction` - Fraction paid in cash as a decimal (e.g. ``0.5`` for 50%).
    /// * `pik_fraction` - Fraction accreted to notional as a decimal.
    #[staticmethod]
    fn split(cash_fraction: f64, pik_fraction: f64) -> Self {
        Self {
            inner: PikMode::Split {
                cash_fraction,
                pik_fraction,
            },
        }
    }

    /// Defer PIK/cash decision to the toggle exercise model on the config.
    #[staticmethod]
    fn toggle() -> Self {
        Self {
            inner: PikMode::Toggle,
        }
    }

    /// Deserialize a PIK mode from canonical JSON.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON produced by ``to_json`` (``'"cash"'``, ``'"pik"'``,
    ///     ``'"toggle"'`` or the tagged ``split`` object).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or names an unknown mode.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid PikMode JSON"))?,
        })
    }

    /// Serialize this PIK mode to compact JSON.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        match self.inner {
            PikMode::Cash => "PikMode.cash()".to_string(),
            PikMode::Pik => "PikMode.pik()".to_string(),
            PikMode::Toggle => "PikMode.toggle()".to_string(),
            PikMode::Split {
                cash_fraction,
                pik_fraction,
            } => {
                format!("PikMode.split(cash_fraction={cash_fraction}, pik_fraction={pik_fraction})")
            }
        }
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Time-varying PIK schedule for the Merton Monte Carlo engine.
#[pyclass(
    name = "PikSchedule",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPikSchedule {
    pub(crate) inner: PikSchedule,
}

#[pymethods]
impl PyPikSchedule {
    /// Apply the same PIK mode at every coupon date.
    ///
    /// # Arguments
    ///
    /// * `mode` - PIK mode applied uniformly across the schedule.
    #[staticmethod]
    fn uniform(mode: PyRef<'_, PyPikMode>) -> Self {
        Self {
            inner: PikSchedule::Uniform(mode.inner),
        }
    }

    /// Step-function PIK schedule keyed by year fraction.
    ///
    /// Each ``(t, mode)`` pair applies ``mode`` from time ``t`` onward.
    /// Entries must be sorted by ``t`` ascending.
    ///
    /// # Arguments
    ///
    /// * `steps` - List of ``(year_fraction, PikMode)`` pairs.
    #[staticmethod]
    fn stepped(steps: &Bound<'_, PyAny>) -> PyResult<Self> {
        let list = steps.cast::<PyList>()?;
        let mut out = Vec::with_capacity(list.len());
        for item in list.iter() {
            let (t, mode): (f64, PyRef<'_, PyPikMode>) = item.extract()?;
            out.push((t, mode.inner));
        }
        Ok(Self {
            inner: PikSchedule::Stepped(out),
        })
    }

    /// Look up the active PIK mode at time ``t`` (year fraction from valuation date).
    ///
    /// # Arguments
    ///
    /// * `t` - Time in years from the valuation date.
    fn mode_at(&self, t: f64) -> PyPikMode {
        PyPikMode::from_inner(self.inner.mode_at(t))
    }

    /// Deserialize a PIK schedule from canonical JSON.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON produced by ``to_json``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid PikSchedule JSON"))?,
        })
    }

    /// Serialize this PIK schedule to compact JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        match &self.inner {
            PikSchedule::Uniform(mode) => format!(
                "PikSchedule.uniform({})",
                PyPikMode::from_inner(*mode).__repr__()
            ),
            PikSchedule::Stepped(steps) => {
                let items: Vec<String> = steps
                    .iter()
                    .map(|(t, mode)| format!("({t}, {})", PyPikMode::from_inner(*mode).__repr__()))
                    .collect();
                format!("PikSchedule.stepped([{}])", items.join(", "))
            }
        }
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Barrier-crossing detection policy for first-passage default simulation.
#[pyclass(
    name = "BarrierCrossing",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
#[derive(Clone, Copy)]
pub(crate) struct PyBarrierCrossing {
    pub(crate) inner: BarrierCrossing,
}

#[pymethods]
impl PyBarrierCrossing {
    /// Discrete monitoring: default if asset value is below the barrier at grid points.
    #[staticmethod]
    fn discrete() -> Self {
        Self {
            inner: BarrierCrossing::Discrete,
        }
    }

    /// Brownian-bridge correction for continuous barrier monitoring between grid points.
    #[staticmethod]
    fn brownian_bridge() -> Self {
        Self {
            inner: BarrierCrossing::BrownianBridge,
        }
    }

    /// Deserialize a barrier-crossing policy from canonical JSON.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     ``'"discrete"'`` or ``'"brownian_bridge"'``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or names an unknown policy.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        Ok(Self {
            inner: serde_json::from_str(json)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid BarrierCrossing JSON"))?,
        })
    }

    /// Serialize this barrier-crossing policy to compact JSON.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        match self.inner {
            BarrierCrossing::Discrete => "BarrierCrossing.discrete()".to_string(),
            BarrierCrossing::BrownianBridge => "BarrierCrossing.brownian_bridge()".to_string(),
        }
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Path-level statistics from a Merton Monte Carlo simulation.
#[pyclass(
    name = "PathStatistics",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyPathStatistics {
    pub(crate) inner: PathStatistics,
}

impl PyPathStatistics {
    pub(crate) fn from_inner(inner: PathStatistics) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPathStatistics {
    /// Fraction of simulated paths that defaulted.
    #[getter]
    fn default_rate(&self) -> f64 {
        self.inner.default_rate
    }

    /// Average default time in years among defaulted paths.
    #[getter]
    fn avg_default_time(&self) -> f64 {
        self.inner.avg_default_time
    }

    /// Average terminal notional, reflecting PIK accretion.
    #[getter]
    fn avg_terminal_notional(&self) -> f64 {
        self.inner.avg_terminal_notional
    }

    /// Average recovery rate (decimal fraction) among defaulted paths.
    #[getter]
    fn avg_recovery_rate(&self) -> f64 {
        self.inner.avg_recovery_rate
    }

    /// Fraction of coupon dates where PIK was elected.
    #[getter]
    fn pik_exercise_rate(&self) -> f64 {
        self.inner.pik_exercise_rate
    }

    /// Identify this value in notebooks and logs.
    ///
    /// The inner type is not `Serialize`, so the headline statistics are
    /// rendered directly.
    fn __repr__(&self) -> String {
        format!(
            "PathStatistics(default_rate={}, avg_recovery_rate={}, pik_exercise_rate={})",
            self.inner.default_rate, self.inner.avg_recovery_rate, self.inner.pik_exercise_rate
        )
    }
}

/// Result from Merton Monte Carlo PIK bond pricing.
#[pyclass(
    name = "MertonMcResult",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMertonMcResult {
    pub(crate) inner: MertonMcResult,
}

impl PyMertonMcResult {
    pub(crate) fn from_inner(inner: MertonMcResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyMertonMcResult {
    /// Clean price as a percentage of par: the dirty price less accrued interest.
    #[getter]
    fn clean_price_pct(&self) -> f64 {
        self.inner.clean_price_pct
    }

    /// Dirty price as a percentage of par: the mean simulated present value.
    #[getter]
    fn dirty_price_pct(&self) -> f64 {
        self.inner.dirty_price_pct
    }

    /// Expected loss as a fraction of PIK-aware risk-free present value.
    #[getter]
    fn expected_loss(&self) -> f64 {
        self.inner.expected_loss
    }

    /// Unexpected loss (standard deviation of path PVs divided by notional).
    #[getter]
    fn unexpected_loss(&self) -> f64 {
        self.inner.unexpected_loss
    }

    /// Expected shortfall at the 95% confidence level.
    #[getter]
    fn expected_shortfall_95(&self) -> f64 {
        self.inner.expected_shortfall_95
    }

    /// Average PIK fraction across all coupon dates and paths.
    #[getter]
    fn average_pik_fraction(&self) -> f64 {
        self.inner.average_pik_fraction
    }

    /// Effective spread in basis points implied by the MC price versus risk-free PV.
    #[getter]
    fn effective_spread_bp(&self) -> f64 {
        self.inner.effective_spread_bp
    }

    /// Path-level simulation statistics.
    #[getter]
    fn path_statistics(&self) -> PyPathStatistics {
        PyPathStatistics::from_inner(self.inner.path_statistics.clone())
    }

    /// Number of independent Monte Carlo estimators (antithetic pairs count once).
    #[getter]
    fn num_paths(&self) -> usize {
        self.inner.num_paths
    }

    /// Standard error of the clean price estimate (percentage of par).
    #[getter]
    fn standard_error(&self) -> f64 {
        self.inner.standard_error
    }

    /// Export the headline results as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``clean_price_pct``, ``dirty_price_pct``, ``expected_loss``,
    /// ``unexpected_loss``, ``expected_shortfall_95``, ``average_pik_fraction``,
    /// ``effective_spread_bp``, ``num_paths``, ``standard_error``,
    /// ``default_rate``, ``avg_default_time``, ``avg_terminal_notional``,
    /// ``avg_recovery_rate``, ``pik_exercise_rate``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let stats = &self.inner.path_statistics;
        let row = serde_json::json!({
            "clean_price_pct": self.inner.clean_price_pct,
            "dirty_price_pct": self.inner.dirty_price_pct,
            "expected_loss": self.inner.expected_loss,
            "unexpected_loss": self.inner.unexpected_loss,
            "expected_shortfall_95": self.inner.expected_shortfall_95,
            "average_pik_fraction": self.inner.average_pik_fraction,
            "effective_spread_bp": self.inner.effective_spread_bp,
            "num_paths": self.inner.num_paths,
            "standard_error": self.inner.standard_error,
            "default_rate": stats.default_rate,
            "avg_default_time": stats.avg_default_time,
            "avg_terminal_notional": stats.avg_terminal_notional,
            "avg_recovery_rate": stats.avg_recovery_rate,
            "pik_exercise_rate": stats.pik_exercise_rate,
        });
        crate::bindings::pandas_utils::serde_object_to_single_row_dataframe_with_schema(
            py,
            &row,
            &[
                "clean_price_pct",
                "dirty_price_pct",
                "expected_loss",
                "unexpected_loss",
                "expected_shortfall_95",
                "average_pik_fraction",
                "effective_spread_bp",
                "num_paths",
                "standard_error",
                "default_rate",
                "avg_default_time",
                "avg_terminal_notional",
                "avg_recovery_rate",
                "pik_exercise_rate",
            ],
        )
    }

    /// Jupyter rich display: the ``to_dataframe()`` table.
    fn _repr_html_<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_dataframe(py)?.call_method0("_repr_html_")
    }

    /// Identify this value in notebooks and logs.
    ///
    /// The inner type is not `Serialize`, so the headline results are
    /// rendered directly.
    fn __repr__(&self) -> String {
        format!(
            "MertonMcResult(clean_price_pct={}, effective_spread_bp={}, expected_loss={})",
            self.inner.clean_price_pct, self.inner.effective_spread_bp, self.inner.expected_loss
        )
    }
}

/// Market calibration of a Merton Monte Carlo configuration.
///
/// Set on a configuration with :meth:`MertonMcConfig.calibration`. The
/// ``merton_mc`` pricing model (``Bond.price(..., model="merton_mc")`` with
/// the configuration in the bond's
/// ``instrument_pricing_overrides.model_config.merton_mc_config``) first
/// solves ``parameter`` by low-path bisection so the cash base-case price
/// matches ``target``, then re-prices with full paths and reports the
/// solved value as the ``calibrated_debt_barrier`` / ``calibrated_asset_vol``
/// measures. ``Bond.price_merton_mc`` prices the configuration as given and
/// does not calibrate.
///
/// Parameters
/// ----------
/// target : dict | str
///     Market quote to match, as a ``BondQuoteInput`` dict or JSON string
///     with one key: ``clean_price_pct`` (percent of par), ``ytm``,
///     ``ytw``, ``z_spread``, ``discount_margin``, ``oas``, ``asw_market``
///     ... Spreads and yields are decimals (``0.02`` = 200bp).
/// parameter : str
///     Structural parameter to solve for: ``"debt_barrier"`` or
///     ``"asset_vol"``.
/// low_paths : int, optional
///     Independent MC estimators per calibration iteration; ``None`` uses
///     the Rust default (2,000).
/// max_iterations : int, optional
///     Maximum bisection iterations; ``None`` uses the Rust default (40).
/// tolerance_pv : float, optional
///     Absolute tolerance on the PV residual, in currency units at the
///     valuation date; ``None`` uses the Rust default (``1e-4``).
/// bracket : tuple[float, float], optional
///     ``(low, high)`` search bracket for ``parameter``; ``None``
///     auto-brackets from the parameter type.
/// seed : int, optional
///     Seed override for the calibration run; ``None`` uses the pricing
///     seed.
///
/// Raises
/// ------
/// ValueError
///     If ``target`` is not a ``BondQuoteInput`` or ``parameter`` is not a
///     calibration parameter.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import MertonMcCalibrationSpec
/// >>> spec = MertonMcCalibrationSpec({"z_spread": 0.03}, "asset_vol", low_paths=500)
/// >>> spec.parameter, spec.low_paths, spec.max_iterations, spec.target
/// ('asset_vol', 500, 40, {'z_spread': 0.03})
#[pyclass(
    name = "MertonMcCalibrationSpec",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMertonMcCalibrationSpec {
    pub(crate) inner: MertonMcCalibrationSpec,
}

#[pymethods]
impl PyMertonMcCalibrationSpec {
    /// Create a calibration specification; omitted settings take the Rust
    /// ``MertonMcCalibrationSpec::default()`` values.
    ///
    /// # Arguments
    ///
    /// * `target` - Market quote to match (``BondQuoteInput`` dict or JSON).
    /// * `parameter` - ``"debt_barrier"`` or ``"asset_vol"``.
    /// * `low_paths` - Estimators per calibration iteration.
    /// * `max_iterations` - Maximum bisection iterations.
    /// * `tolerance_pv` - Absolute PV-residual tolerance in currency units.
    /// * `bracket` - ``(low, high)`` search bracket for the parameter.
    /// * `seed` - Seed override for the calibration run.
    #[new]
    #[pyo3(signature = (target, parameter, low_paths=None, max_iterations=None, tolerance_pv=None, bracket=None, seed=None))]
    #[pyo3(
        text_signature = "(target, parameter, low_paths=None, max_iterations=None, tolerance_pv=None, bracket=None, seed=None)"
    )]
    // PyO3 binding: the argument list mirrors the Rust struct fields.
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        target: &Bound<'_, PyAny>,
        parameter: &str,
        low_paths: Option<usize>,
        max_iterations: Option<usize>,
        tolerance_pv: Option<f64>,
        bracket: Option<(f64, f64)>,
        seed: Option<u64>,
    ) -> PyResult<Self> {
        let target = match target.extract::<String>() {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid BondQuoteInput"))?,
            Err(_) => crate::bindings::module_utils::py_to_serde(py, target, "BondQuoteInput")?,
        };
        let defaults = MertonMcCalibrationSpec::default();
        Ok(Self {
            inner: MertonMcCalibrationSpec {
                target,
                parameter: super::instruments::enum_from_str(parameter, "calibration parameter")?,
                low_paths: low_paths.unwrap_or(defaults.low_paths),
                max_iterations: max_iterations.unwrap_or(defaults.max_iterations),
                tolerance_pv: tolerance_pv.unwrap_or(defaults.tolerance_pv),
                bracket: bracket.or(defaults.bracket),
                seed: seed.or(defaults.seed),
            },
        })
    }

    /// Market quote to match as its ``BondQuoteInput`` dict (one key, e.g.
    /// ``{"z_spread": 0.03}``).
    #[getter]
    fn target<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.target)
    }

    /// Structural parameter solved for: ``"debt_barrier"`` or ``"asset_vol"``.
    #[getter]
    fn parameter(&self) -> PyResult<String> {
        super::convert::enum_to_py_string(&self.inner.parameter)
    }

    /// Independent MC estimators per calibration iteration.
    #[getter]
    fn low_paths(&self) -> usize {
        self.inner.low_paths
    }

    /// Maximum bisection iterations.
    #[getter]
    fn max_iterations(&self) -> usize {
        self.inner.max_iterations
    }

    /// Absolute tolerance on the PV residual, in currency units.
    #[getter]
    fn tolerance_pv(&self) -> f64 {
        self.inner.tolerance_pv
    }

    /// ``(low, high)`` search bracket, or ``None`` to auto-bracket.
    #[getter]
    fn bracket(&self) -> Option<(f64, f64)> {
        self.inner.bracket
    }

    /// Seed override for the calibration run, or ``None``.
    #[getter]
    fn seed(&self) -> Option<u64> {
        self.inner.seed
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     ``MertonMcCalibrationSpec`` JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is malformed or carries unknown fields.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        serde_json::from_str(json)
            .map(|inner| Self { inner })
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid MertonMcCalibrationSpec JSON"))
    }

    /// Serialize to the JSON shape ``from_json`` accepts.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Return every field as a plain ``dict`` (canonical serde shape).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    /// Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "MertonMcCalibrationSpec(target={}, parameter={:?}, low_paths={}, max_iterations={})",
            serde_json::to_string(&self.inner.target).unwrap_or_default(),
            super::convert::enum_to_py_string(&self.inner.parameter).unwrap_or_default(),
            self.inner.low_paths,
            self.inner.max_iterations
        )
    }
}

/// Configuration for Merton Monte Carlo PIK bond pricing.
///
/// Built from a structural ``MertonModel`` plus a flat recovery rate; every
/// setter returns a new configuration (the receiver is unchanged), so calls
/// chain: ``MertonMcConfig(model, 0.4).steps_per_year(24)``. The path count,
/// antithetic flag and seed label come from the bond's
/// ``instrument_pricing_overrides.model_config`` (``mc_paths``,
/// ``mc_antithetic``, ``mc_seed_scenario``).
///
/// Parameters
/// ----------
/// merton : MertonModel
///     Structural credit model driving asset dynamics and default.
/// recovery_rate : float
///     Recovery on default as a decimal fraction in ``[0.0, 1.0]``.
///
/// Raises
/// ------
/// ValueError
///     If ``recovery_rate`` is outside ``[0.0, 1.0]``.
/// RuntimeError
///     If the embedded Monte Carlo defaults cannot be loaded.
#[pyclass(
    name = "MertonMcConfig",
    module = "finstack_quant.valuations.instruments",
    skip_from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyMertonMcConfig {
    pub(crate) inner: MertonMcConfig,
}

#[pymethods]
impl PyMertonMcConfig {
    /// Create a Merton MC configuration with explicit recovery.
    ///
    /// # Arguments
    ///
    /// * `merton` - Structural credit model driving asset dynamics and default.
    /// * `recovery_rate` - Recovery on default as a decimal fraction in
    ///   ``[0.0, 1.0]``.
    #[new]
    #[pyo3(text_signature = "(merton, recovery_rate)")]
    fn new(merton: PyRef<'_, PyMertonModel>, recovery_rate: f64) -> PyResult<Self> {
        Ok(Self {
            inner: MertonMcConfig::new(merton.inner.clone(), recovery_rate).map_err(core_to_py)?,
        })
    }

    /// Set the PIK schedule controlling per-coupon cash/PIK/toggle behavior.
    ///
    /// # Arguments
    ///
    /// * `s` - PIK schedule applied across coupon dates.
    #[pyo3(text_signature = "($self, s)")]
    fn pik_schedule(&self, s: PyRef<'_, PyPikSchedule>) -> Self {
        Self {
            inner: self.inner.clone().pik_schedule(s.inner.clone()),
        }
    }

    /// Set the number of time steps simulated per year.
    ///
    /// # Arguments
    ///
    /// * `n` - Grid density for asset evolution and barrier monitoring.
    #[pyo3(text_signature = "($self, n)")]
    fn steps_per_year(&self, n: usize) -> Self {
        Self {
            inner: self.inner.clone().steps_per_year(n),
        }
    }

    /// Set the barrier-crossing policy for first-passage default monitoring.
    ///
    /// # Arguments
    ///
    /// * `p` - Discrete grid checks or Brownian-bridge continuous correction.
    #[pyo3(text_signature = "($self, p)")]
    fn barrier_crossing(&self, p: PyRef<'_, PyBarrierCrossing>) -> Self {
        Self {
            inner: self.inner.clone().barrier_crossing(p.inner),
        }
    }

    /// Set the flat recovery rate used when no dynamic recovery model is configured.
    ///
    /// # Arguments
    ///
    /// * `r` - Recovery rate as a decimal in ``[0, 1]``.
    #[pyo3(text_signature = "($self, r)")]
    fn recovery_rate(&self, r: f64) -> PyResult<Self> {
        Ok(Self {
            inner: self.inner.clone().recovery_rate(r).map_err(core_to_py)?,
        })
    }

    /// Set an endogenous (leverage-dependent) hazard rate model.
    ///
    /// # Arguments
    ///
    /// * `h` - Endogenous hazard specification applied pathwise at default.
    #[pyo3(text_signature = "($self, h)")]
    fn endogenous_hazard(&self, h: PyRef<'_, PyEndogenousHazardSpec>) -> Self {
        Self {
            inner: self.inner.clone().endogenous_hazard(h.inner.clone()),
        }
    }

    /// Set a dynamic (notional-dependent) recovery rate model.
    ///
    /// # Arguments
    ///
    /// * `r` - Dynamic recovery specification evaluated at accreted notional.
    #[pyo3(text_signature = "($self, r)")]
    fn dynamic_recovery(&self, r: PyRef<'_, PyDynamicRecoverySpec>) -> Self {
        Self {
            inner: self.inner.clone().dynamic_recovery(r.inner),
        }
    }

    /// Set the toggle exercise model for PIK/cash coupon decisions.
    ///
    /// Active only on coupon dates where the PIK schedule resolves to
    /// ``PikMode.toggle()``.
    ///
    /// # Arguments
    ///
    /// * `t` - Toggle exercise model deciding cash versus PIK at each toggle date.
    #[pyo3(text_signature = "($self, t)")]
    fn toggle_model(&self, t: PyRef<'_, PyToggleExerciseModel>) -> Self {
        Self {
            inner: self.inner.clone().toggle_model(t.inner.clone()),
        }
    }

    /// Set the market-calibration specification.
    ///
    /// Honoured by the ``merton_mc`` pricing model (``Bond.price(...,
    /// model="merton_mc")`` with this configuration in the bond's
    /// ``instrument_pricing_overrides.model_config.merton_mc_config``), which
    /// solves the structural parameter to the target quote before the
    /// full-path price. ``Bond.price_merton_mc`` prices the configuration as
    /// given and does not calibrate.
    ///
    /// # Arguments
    ///
    /// * `spec` - Calibration target, solved parameter and solver settings.
    #[pyo3(text_signature = "($self, spec)")]
    fn calibration(&self, spec: PyRef<'_, PyMertonMcCalibrationSpec>) -> Self {
        Self {
            inner: self.inner.clone().calibration(spec.inner.clone()),
        }
    }

    /// Set term-structure discount factors for cashflow discounting.
    ///
    /// When set, cashflows are discounted by log-linear interpolation of
    /// these factors instead of the flat ``discount_rate``; the flat rate
    /// still drives the risk-neutral asset drift. The ``merton_mc`` pricing
    /// model fills them from the discount curve when they are not set.
    ///
    /// # Arguments
    ///
    /// * `dfs` - ``(year_fraction, discount_factor)`` pairs sorted by time,
    ///   with year fractions measured from the valuation date.
    #[pyo3(text_signature = "($self, dfs)")]
    fn cashflow_dfs(&self, dfs: Vec<(f64, f64)>) -> Self {
        Self {
            inner: self.inner.clone().cashflow_dfs(dfs),
        }
    }

    /// Return every configuration field as a plain ``dict`` (canonical serde
    /// shape: ``merton``, ``pik_schedule``, ``steps_per_year``,
    /// ``barrier_crossing``, ``recovery_rate``, ``calibration``,
    /// ``cashflow_dfs``, ...).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the configuration cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    /// Deserialize a Merton MC configuration from canonical JSON.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON produced by ``to_json``.
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
                .map_err(|e| crate::errors::serde_json_to_py(e, "invalid MertonMcConfig JSON"))?,
        })
    }

    /// Serialize this configuration to compact JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        let bool_repr = super::convert::bool_repr;
        format!(
            "MertonMcConfig(steps_per_year={}, barrier_crossing={}, recovery_rate={}, pik_schedule={}, endogenous_hazard={}, dynamic_recovery={}, toggle_model={}, calibration={}, cashflow_dfs={})",
            self.inner.steps_per_year,
            PyBarrierCrossing {
                inner: self.inner.barrier_crossing
            }
            .__repr__(),
            self.inner.recovery_rate,
            PyPikSchedule {
                inner: self.inner.pik_schedule.clone()
            }
            .__repr__(),
            bool_repr(self.inner.endogenous_hazard.is_some()),
            bool_repr(self.inner.dynamic_recovery.is_some()),
            bool_repr(self.inner.toggle_model.is_some()),
            bool_repr(self.inner.calibration.is_some()),
            bool_repr(self.inner.cashflow_dfs.is_some()),
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

pub(crate) const EXPORTS: &[&str] = &[
    "BarrierCrossing",
    "MertonMcCalibrationSpec",
    "MertonMcConfig",
    "MertonMcResult",
    "PathStatistics",
    "PikMode",
    "PikSchedule",
];

/// Register Merton MC types on the instruments submodule.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBarrierCrossing>()?;
    m.add_class::<PyMertonMcCalibrationSpec>()?;
    m.add_class::<PyMertonMcConfig>()?;
    m.add_class::<PyMertonMcResult>()?;
    m.add_class::<PyPathStatistics>()?;
    m.add_class::<PyPikMode>()?;
    m.add_class::<PyPikSchedule>()?;
    Ok(())
}
