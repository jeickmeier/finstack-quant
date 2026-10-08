use pyo3::prelude::*;
use pyo3::types::PyType;

use finstack_quant_models::factor::risk::DecompositionConfig;

use crate::bindings::pickle_support::reduce_via_json;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::{core_to_py, serde_json_to_py};

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
        finstack_quant_core::wire::serde_label(&self.inner.method).map_err(core_to_py)
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
