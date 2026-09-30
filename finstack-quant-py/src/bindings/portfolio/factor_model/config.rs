use pyo3::prelude::*;
use pyo3::types::PyType;

use finstack_quant_models::factor::credit::VolHorizon;
use finstack_quant_models::factor::risk::{DecompositionConfig, DecompositionMethod};

use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, serde_json_to_py, value_error};

/// Serde name of a [`DecompositionMethod`] (`"parametric"` / `"historical"`).
pub(super) fn decomposition_method_label(method: DecompositionMethod) -> PyResult<String> {
    finstack_quant_core::wire::serde_label(&method).map_err(core_to_py)
}

/// Canonical descriptor string of a [`VolHorizon`], accepted back by
/// [`VolHorizon::parse`]. This is the pickle payload.
fn vol_horizon_descriptor(horizon: VolHorizon) -> String {
    match horizon {
        VolHorizon::OneStep => "one_step".to_owned(),
        VolHorizon::Unconditional => "unconditional".to_owned(),
        VolHorizon::NSteps(n) => format!("{{\"n_steps\": {n}}}"),
        VolHorizon::Years(years) => format!("{{\"years\": {years}}}"),
    }
}

/// Extract a [`VolHorizon`] from either a `VolHorizon` instance or a
/// descriptor string (`"one_step"`, `"unconditional"`, `'{"n_steps": N}'`,
/// `'{"years": Y}'`).
pub(crate) fn extract_vol_horizon(obj: &Bound<'_, PyAny>) -> PyResult<VolHorizon> {
    if let Ok(horizon) = obj.extract::<PyRef<'_, PyVolHorizon>>() {
        return Ok(horizon.inner);
    }
    if let Ok(descriptor) = obj.extract::<std::borrow::Cow<'_, str>>() {
        return VolHorizon::parse(&descriptor).map_err(value_error);
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "horizon must be a VolHorizon or a descriptor string such as \"one_step\", \
         \"unconditional\", '{\"n_steps\": N}' or '{\"years\": Y}'",
    ))
}

/// Forecast horizon used to scale a calibrated ``Sample`` vol estimate.
///
/// Construct with one of the classmethods:
///
/// - ``VolHorizon.one_step()`` — calibrated annualized variance unchanged.
/// - ``VolHorizon.unconditional()`` — long-run variance (identical to
///   ``one_step`` for the ``Sample`` and ``Ewma`` vol models).
/// - ``VolHorizon.n_steps(n)`` — variance scaled by ``n`` model periods.
/// - ``VolHorizon.years(years)`` — variance scaled by a fractional year.
/// - ``VolHorizon.parse(s)`` — from a descriptor string.
///
/// Every ``FactorCovarianceForecast`` method also accepts the descriptor
/// string directly, so a ``VolHorizon`` instance is optional.
///
/// Example:
///     >>> from finstack_quant.models.factor.credit import VolHorizon
///     >>> VolHorizon.n_steps(5) == VolHorizon.parse('{"n_steps": 5}')
///     True
#[pyclass(
    name = "VolHorizon",
    module = "finstack_quant.models.factor.credit",
    frozen,
    eq,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PyVolHorizon {
    pub(crate) inner: VolHorizon,
}

impl PyVolHorizon {
    pub(crate) fn from_inner(inner: VolHorizon) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyVolHorizon {
    /// One-period horizon: the calibrated annualized variance unchanged.
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn one_step(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(VolHorizon::OneStep)
    }

    /// Long-run horizon; numerically identical to ``one_step`` for the
    /// ``Sample`` and ``Ewma`` vol models.
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn unconditional(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(VolHorizon::Unconditional)
    }

    /// ``n`` annualized model periods; variance scales linearly with ``n``.
    ///
    /// Args:
    ///     n: Non-negative period count (``0`` yields zero variance).
    #[classmethod]
    #[pyo3(text_signature = "(cls, n)")]
    fn n_steps(_cls: &Bound<'_, PyType>, n: usize) -> Self {
        Self::from_inner(VolHorizon::NSteps(n))
    }

    /// Fractional-year horizon; variance scales linearly with ``years``.
    ///
    /// Args:
    ///     years: Finite, non-negative horizon in years (``10 / 252`` for ten
    ///         trading days of an annualized variance).
    ///
    /// Raises:
    ///     ValueError: If ``years`` is negative or non-finite.
    #[classmethod]
    #[pyo3(text_signature = "(cls, years)")]
    fn years(_cls: &Bound<'_, PyType>, years: f64) -> PyResult<Self> {
        if years.is_finite() && years >= 0.0 {
            Ok(Self::from_inner(VolHorizon::Years(years)))
        } else {
            Err(value_error("years must be finite and non-negative"))
        }
    }

    /// Parse a horizon descriptor string (matches the Rust ``VolHorizon::parse``).
    ///
    /// Args:
    ///     s: ``"one_step"``, ``"unconditional"``, ``'{"n_steps": N}'``,
    ///         ``'{"years": Y}'`` or ``'{"n_steps": N, "periods_per_year": P}'``.
    ///
    /// Raises:
    ///     ValueError: If ``s`` is not one of the accepted forms.
    #[classmethod]
    #[pyo3(text_signature = "(cls, s)")]
    fn parse(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        VolHorizon::parse(s)
            .map(Self::from_inner)
            .map_err(value_error)
    }

    /// Support pickle through the canonical descriptor string.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let parse = py.get_type::<Self>().getattr("parse")?;
        reduce_via_json(parse, vol_horizon_descriptor(self.inner))
    }

    /// Variant label: ``"one_step"`` / ``"unconditional"`` / ``"n_steps"`` / ``"years"``.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            VolHorizon::OneStep => "one_step",
            VolHorizon::Unconditional => "unconditional",
            VolHorizon::NSteps(_) => "n_steps",
            VolHorizon::Years(_) => "years",
        }
    }

    /// Step count when ``kind == "n_steps"``, ``None`` otherwise.
    #[getter]
    fn n(&self) -> Option<usize> {
        match self.inner {
            VolHorizon::NSteps(n) => Some(n),
            _ => None,
        }
    }

    /// Fractional-year horizon when ``kind == "years"``, ``None`` otherwise.
    #[getter]
    fn years_value(&self) -> Option<f64> {
        match self.inner {
            VolHorizon::Years(years) => Some(years),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        match self.inner {
            VolHorizon::OneStep => "VolHorizon.one_step()".to_owned(),
            VolHorizon::Unconditional => "VolHorizon.unconditional()".to_owned(),
            VolHorizon::NSteps(n) => format!("VolHorizon.n_steps({n})"),
            VolHorizon::Years(years) => format!("VolHorizon.years({years})"),
        }
    }
}

/// Configuration for position-level VaR / ES decomposition.
///
/// Holds the tail ``confidence`` (decimal probability in ``(0.5, 1)``), the
/// ``method`` (``"parametric"`` or ``"historical"``) and whether
/// leave-one-out incremental VaR is computed. The decomposition functions
/// take ``confidence`` / ``compute_incremental`` directly and resolve an
/// omitted confidence to the ``parametric_95()`` / ``historical_95()``
/// presets.
///
/// Example:
///     >>> from finstack_quant.models.factor.risk import DecompositionConfig
///     >>> cfg = DecompositionConfig.parametric(0.975).with_incremental()
///     >>> (cfg.confidence, cfg.method, cfg.compute_incremental)
///     (0.975, 'parametric', True)
///     >>> DecompositionConfig.from_json(cfg.to_json()) == cfg
///     True
#[pyclass(
    name = "DecompositionConfig",
    module = "finstack_quant.models.factor.risk",
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub(crate) struct PyDecompositionConfig {
    pub(crate) inner: DecompositionConfig,
}

impl PyDecompositionConfig {
    pub(crate) fn from_inner(inner: DecompositionConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyDecompositionConfig {
    /// Parametric configuration at an arbitrary confidence level.
    ///
    /// Args:
    ///     confidence: Tail confidence as a decimal probability strictly
    ///         inside ``(0.5, 1)``, e.g. ``0.95``.
    #[classmethod]
    #[pyo3(text_signature = "(cls, confidence)")]
    fn parametric(_cls: &Bound<'_, PyType>, confidence: f64) -> Self {
        Self::from_inner(DecompositionConfig::parametric(confidence))
    }

    /// Standard 95% parametric configuration.
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn parametric_95(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(DecompositionConfig::parametric_95())
    }

    /// Standard 99% parametric configuration.
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn parametric_99(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(DecompositionConfig::parametric_99())
    }

    /// Historical-simulation configuration at the given confidence.
    ///
    /// Args:
    ///     confidence: Tail confidence as a decimal probability strictly
    ///         inside ``(0.5, 1)``.
    #[classmethod]
    #[pyo3(text_signature = "(cls, confidence)")]
    fn historical(_cls: &Bound<'_, PyType>, confidence: f64) -> Self {
        Self::from_inner(DecompositionConfig::historical(confidence))
    }

    /// Standard 95% historical-simulation configuration.
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn historical_95(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(DecompositionConfig::historical_95())
    }

    /// Return a copy that also computes leave-one-out incremental VaR
    /// (one full repricing per position).
    #[pyo3(text_signature = "($self)")]
    fn with_incremental(&self) -> Self {
        Self::from_inner(self.inner.clone().with_incremental())
    }

    /// Deserialize from canonical JSON (``confidence``, ``method``,
    /// ``compute_incremental``).
    ///
    /// Raises:
    ///     ValueError: If the JSON is malformed or names an unknown field.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: DecompositionConfig = serde_json::from_str(json_str)
            .map_err(|e| serde_json_to_py(e, "invalid DecompositionConfig JSON"))?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to canonical JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "cannot serialize DecompositionConfig"))
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        reduce_via_json(from_json, self.to_json()?)
    }

    /// Tail confidence as a decimal probability (``0.95``, not ``95``).
    #[getter]
    fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    /// Decomposition method: ``"parametric"`` or ``"historical"``.
    #[getter]
    fn method(&self) -> PyResult<String> {
        decomposition_method_label(self.inner.method)
    }

    /// Whether leave-one-out incremental VaR is computed.
    #[getter]
    fn compute_incremental(&self) -> bool {
        self.inner.compute_incremental
    }

    fn __repr__(&self) -> String {
        repr_from_serde("DecompositionConfig", &self.inner)
    }
}
