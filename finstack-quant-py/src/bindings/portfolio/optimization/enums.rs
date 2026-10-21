use pyo3::prelude::*;

use crate::errors::core_to_py;
use pyo3::types::PyType;

use finstack_quant_portfolio::optimization::{
    Inequality, MissingMetricPolicy, TradeDirection, TradeType, WeightingScheme,
};

/// How optimization weights are defined.
#[pyclass(
    name = "WeightingScheme",
    module = "finstack_quant.portfolio",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PyWeightingScheme {
    pub(crate) inner: WeightingScheme,
}

impl std::hash::Hash for PyWeightingScheme {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag: u8 = match self.inner {
            WeightingScheme::ValueWeight => 0,
            WeightingScheme::NotionalWeight => 1,
            WeightingScheme::UnitScaling => 2,
        };
        tag.hash(state);
    }
}

#[pymethods]
impl PyWeightingScheme {
    #[classmethod]
    fn value_weight(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: WeightingScheme::ValueWeight,
        }
    }

    #[classmethod]
    fn notional_weight(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: WeightingScheme::NotionalWeight,
        }
    }

    #[classmethod]
    fn unit_scaling(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: WeightingScheme::UnitScaling,
        }
    }

    #[getter]
    fn label(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("WeightingScheme.{}()", self.label()?))
    }
}

/// Policy for handling positions missing required metrics.
#[pyclass(
    name = "MissingMetricPolicy",
    module = "finstack_quant.portfolio",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PyMissingMetricPolicy {
    pub(crate) inner: MissingMetricPolicy,
}

impl std::hash::Hash for PyMissingMetricPolicy {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag: u8 = match self.inner {
            MissingMetricPolicy::Zero => 0,
            MissingMetricPolicy::Exclude => 1,
            MissingMetricPolicy::Strict => 2,
        };
        tag.hash(state);
    }
}

#[pymethods]
impl PyMissingMetricPolicy {
    #[classmethod]
    fn zero(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: MissingMetricPolicy::Zero,
        }
    }

    /// Freeze existing quantities when an in-scope standard metric, custom key,
    /// or numeric attribute is missing. Omit each missing input from the
    /// expression's coefficients and value-weighted-average denominator.
    #[classmethod]
    fn exclude(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: MissingMetricPolicy::Exclude,
        }
    }

    #[classmethod]
    fn strict(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: MissingMetricPolicy::Strict,
        }
    }

    #[getter]
    fn label(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("MissingMetricPolicy.{}()", self.label()?))
    }
}

/// Inequality / equality operator (`<=`, `>=`, `==`).
#[pyclass(
    name = "Inequality",
    module = "finstack_quant.portfolio",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PyInequality {
    pub(crate) inner: Inequality,
}

impl std::hash::Hash for PyInequality {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag: u8 = match self.inner {
            Inequality::Le => 0,
            Inequality::Ge => 1,
            Inequality::Eq => 2,
        };
        tag.hash(state);
    }
}

#[pymethods]
impl PyInequality {
    /// Parse from a string: ``"le"``/``"<="``, ``"ge"``/``">="`` or
    /// ``"eq"``/``"=="``. Raises ``ValueError`` for anything else.
    #[new]
    #[pyo3(text_signature = "(op)")]
    fn new(op: &str) -> PyResult<Self> {
        Ok(Self {
            inner: op.parse().map_err(crate::errors::portfolio_to_py)?,
        })
    }

    #[classmethod]
    fn le(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Inequality::Le,
        }
    }

    #[classmethod]
    fn ge(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Inequality::Ge,
        }
    }

    #[classmethod]
    fn eq(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: Inequality::Eq,
        }
    }

    /// Canonical inequality operator label such as ``le``, ``ge``, or ``eq``.
    #[getter]
    fn label(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("Inequality.{}()", self.label()?))
    }
}

/// Trade direction from the signed quantity change (buy / sell / hold).
#[pyclass(
    name = "TradeDirection",
    module = "finstack_quant.portfolio",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PyTradeDirection {
    pub(crate) inner: TradeDirection,
}

impl std::hash::Hash for PyTradeDirection {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag: u8 = match self.inner {
            TradeDirection::Buy => 0,
            TradeDirection::Sell => 1,
            TradeDirection::Hold => 2,
        };
        tag.hash(state);
    }
}

#[pymethods]
impl PyTradeDirection {
    /// Buy direction for a positive quantity change, including covering a short.
    #[classmethod]
    fn buy(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TradeDirection::Buy,
        }
    }

    /// Sell direction for a negative quantity change, including increasing a short.
    #[classmethod]
    fn sell(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TradeDirection::Sell,
        }
    }

    /// Hold direction for an unchanged instrument quantity.
    #[classmethod]
    fn hold(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TradeDirection::Hold,
        }
    }

    #[getter]
    fn label(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("TradeDirection.{}()", self.label()?))
    }
}

/// Trade type (existing / new position / close-out).
#[pyclass(
    name = "TradeType",
    module = "finstack_quant.portfolio",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PyTradeType {
    pub(crate) inner: TradeType,
}

impl std::hash::Hash for PyTradeType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag: u8 = match self.inner {
            TradeType::Existing => 0,
            TradeType::NewPosition => 1,
            TradeType::CloseOut => 2,
        };
        tag.hash(state);
    }
}

#[pymethods]
impl PyTradeType {
    #[classmethod]
    fn existing(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TradeType::Existing,
        }
    }

    #[classmethod]
    fn new_position(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TradeType::NewPosition,
        }
    }

    #[classmethod]
    fn close_out(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: TradeType::CloseOut,
        }
    }

    #[getter]
    fn label(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("TradeType.{}()", self.label()?))
    }
}
