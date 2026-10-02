use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use finstack_quant_models::factor::risk::{
    FactorContribution, PositionEsContribution, PositionFactorContribution,
    PositionResidualContribution, PositionRiskDecomposition, PositionVarContribution,
    ResidualContributionSource, RiskDecomposition,
};

use crate::bindings::pandas_utils::{
    dict_to_dataframe, serde_object_to_single_row_dataframe, serde_to_py,
};
use crate::bindings::repr_support::repr_from_serde;

use super::super::json_bridge::{deserialize_json, serialize_json};
use super::config::decomposition_method_label;

/// Python-style rendering of an optional float (`None` or the number).
fn py_opt(value: Option<f64>) -> String {
    value.map_or_else(|| "None".to_owned(), |v| v.to_string())
}

/// Aggregate contribution of a single factor to portfolio risk.
#[pyclass(
    name = "FactorContribution",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyFactorContribution {
    pub(crate) inner: FactorContribution,
}

impl PyFactorContribution {
    fn from_inner(inner: FactorContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorContribution {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: FactorContribution = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Identifier of the factor being reported.
    #[getter]
    fn factor_id(&self) -> String {
        self.inner.factor_id.as_str().to_owned()
    }

    /// Absolute contribution of the factor, in the units of the decomposition's
    /// risk measure. Sign follows the measure's convention.
    #[getter]
    fn absolute_risk(&self) -> f64 {
        self.inner.absolute_risk
    }

    /// Contribution as a dimensionless **fraction** (not a percentage) of total
    /// portfolio risk; non-negative for a standard long-risk portfolio.
    #[getter]
    fn relative_risk(&self) -> f64 {
        self.inner.relative_risk
    }

    /// Marginal sensitivity of portfolio risk to this factor, same sign
    /// convention as :attr:`absolute_risk`.
    #[getter]
    fn marginal_risk(&self) -> f64 {
        self.inner.marginal_risk
    }

    /// Export this contribution as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``factor_id``, ``absolute_risk``, ``relative_risk``,
    /// ``marginal_risk``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("FactorContribution", &self.inner)
    }
}

/// Per-position contribution to a specific factor bucket.
#[pyclass(
    name = "PositionFactorContribution",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPositionFactorContribution {
    pub(crate) inner: PositionFactorContribution,
}

impl PyPositionFactorContribution {
    fn from_inner(inner: PositionFactorContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionFactorContribution {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: PositionFactorContribution = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Portfolio position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Identifier of the contributing factor.
    #[getter]
    fn factor_id(&self) -> String {
        self.inner.factor_id.as_str().to_owned()
    }

    /// Risk attributed to this position-factor pair, in the units of the
    /// decomposition's risk measure.
    #[getter]
    fn risk_contribution(&self) -> f64 {
        self.inner.risk_contribution
    }

    /// Export this contribution as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``factor_id``, ``risk_contribution``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("PositionFactorContribution", &self.inner)
    }
}

/// Annualized residual variance contributed by a single position.
#[pyclass(
    name = "PositionResidualContribution",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPositionResidualContribution {
    pub(crate) inner: PositionResidualContribution,
}

impl PyPositionResidualContribution {
    fn from_inner(inner: PositionResidualContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionResidualContribution {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: PositionResidualContribution = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Portfolio position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Annualized **variance** (not volatility) contributed by this position's
    /// idiosyncratic risk. Always non-negative; take the square root only after
    /// summing the subset of interest.
    #[getter]
    fn residual_variance(&self) -> f64 {
        self.inner.residual_variance
    }

    /// Source kind: ``"from_credit_model"`` or ``"other"``.
    #[getter]
    fn source_kind(&self) -> &'static str {
        match &self.inner.source {
            ResidualContributionSource::FromCreditModel { .. } => "from_credit_model",
            ResidualContributionSource::Other => "other",
        }
    }

    /// Issuer ID when ``source_kind == "from_credit_model"``, ``None`` otherwise.
    #[getter]
    fn source_issuer_id(&self) -> Option<String> {
        match &self.inner.source {
            ResidualContributionSource::FromCreditModel { issuer_id } => {
                Some(issuer_id.as_str().to_owned())
            }
            ResidualContributionSource::Other => None,
        }
    }

    /// Export this contribution as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``residual_variance``, ``source_kind``,
    /// ``source_issuer_id`` (``None`` unless ``source_kind`` is
    /// ``"from_credit_model"``).
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("position_id", vec![self.inner.position_id.as_str()])?;
        data.set_item("residual_variance", vec![self.inner.residual_variance])?;
        data.set_item("source_kind", vec![self.source_kind()])?;
        data.set_item("source_issuer_id", vec![self.source_issuer_id()])?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionResidualContribution(position_id={:?}, residual_variance={}, source_kind={:?}, source_issuer_id={})",
            self.inner.position_id.as_str(),
            self.inner.residual_variance,
            self.source_kind(),
            self.source_issuer_id()
                .map_or_else(|| "None".to_owned(), |id| format!("{id:?}")),
        )
    }
}

/// Portfolio-level decomposition of total risk across common factors and residuals.
#[pyclass(
    name = "RiskDecomposition",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(crate) struct PyRiskDecomposition {
    pub(crate) inner: RiskDecomposition,
}

impl PyRiskDecomposition {
    pub(crate) fn from_inner(inner: RiskDecomposition) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRiskDecomposition {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: RiskDecomposition = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Total portfolio risk under the selected measure.
    ///
    /// Returns
    /// -------
    /// float
    ///     Value in the units of :attr:`measure_json` (variance units for
    ///     ``"variance"``, return units for ``"volatility"``/VaR/ES). Sign
    ///     follows the measure's convention.
    #[getter]
    fn total_risk(&self) -> f64 {
        self.inner.total_risk
    }

    /// Risk measure used for aggregation, as its canonical Python value:
    /// ``"variance"``, ``"volatility"``, ``{"var": {"confidence": c}}`` or
    /// ``{"expected_shortfall": {"confidence": c}}`` — the same shape as
    /// ``FactorModelConfig.risk_measure``.
    #[getter]
    fn measure<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.measure)
    }

    /// Risk measure used for aggregation (serialized as a JSON string).
    #[getter]
    fn measure_json(&self) -> PyResult<String> {
        serialize_json(&self.inner.measure)
    }

    /// Unattributed (idiosyncratic) risk left after factor aggregation, in the
    /// same units and sign convention as :attr:`total_risk`.
    #[getter]
    fn residual_risk(&self) -> f64 {
        self.inner.residual_risk
    }

    /// Aggregate factor-level contributions to portfolio risk.
    #[getter]
    fn factor_contributions(&self) -> Vec<PyFactorContribution> {
        self.inner
            .factor_contributions
            .iter()
            .cloned()
            .map(PyFactorContribution::from_inner)
            .collect()
    }

    /// Per-position, per-factor contributions that roll up into the portfolio view.
    #[getter]
    fn position_factor_contributions(&self) -> Vec<PyPositionFactorContribution> {
        self.inner
            .position_factor_contributions
            .iter()
            .cloned()
            .map(PyPositionFactorContribution::from_inner)
            .collect()
    }

    /// Per-position residual (idiosyncratic) variance contributions.
    ///
    /// Returns
    /// -------
    /// list[PositionResidualContribution]
    ///     Empty unless the decomposer had per-issuer idiosyncratic vol
    ///     estimates (credit-aware position decomposers).
    #[getter]
    fn position_residual_contributions(&self) -> Vec<PyPositionResidualContribution> {
        self.inner
            .position_residual_contributions
            .iter()
            .cloned()
            .map(PyPositionResidualContribution::from_inner)
            .collect()
    }

    /// Primary table: the factor-level risk decomposition.
    ///
    /// Alias of :meth:`to_factor_dataframe`. Every tabular result type in the
    /// library answers ``to_dataframe()``; the position-level views stay on
    /// :meth:`to_position_factor_dataframe` and
    /// :meth:`to_position_residual_dataframe`.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.to_factor_dataframe(py)
    }

    /// Export factor contributions as a pandas ``DataFrame``.
    ///
    /// Columns: ``factor_id``, ``absolute_risk``, ``relative_risk``,
    /// ``marginal_risk``.
    #[pyo3(text_signature = "(self)")]
    fn to_factor_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = &self.inner.factor_contributions;
        let factor_ids: Vec<&str> = rows.iter().map(|c| c.factor_id.as_str()).collect();
        let absolute_risks: Vec<f64> = rows.iter().map(|c| c.absolute_risk).collect();
        let relative_risks: Vec<f64> = rows.iter().map(|c| c.relative_risk).collect();
        let marginal_risks: Vec<f64> = rows.iter().map(|c| c.marginal_risk).collect();
        let data = PyDict::new(py);
        data.set_item("factor_id", factor_ids)?;
        data.set_item("absolute_risk", absolute_risks)?;
        data.set_item("relative_risk", relative_risks)?;
        data.set_item("marginal_risk", marginal_risks)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Export position × factor contributions as a pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``factor_id``, ``risk_contribution``.
    #[pyo3(text_signature = "(self)")]
    fn to_position_factor_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = &self.inner.position_factor_contributions;
        let position_ids: Vec<&str> = rows.iter().map(|c| c.position_id.as_str()).collect();
        let factor_ids: Vec<&str> = rows.iter().map(|c| c.factor_id.as_str()).collect();
        let risk_contributions: Vec<f64> = rows.iter().map(|c| c.risk_contribution).collect();
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("factor_id", factor_ids)?;
        data.set_item("risk_contribution", risk_contributions)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Export per-position residual variance contributions as a pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``residual_variance`` (annualized variance,
    /// non-negative), ``source_kind`` (``"from_credit_model"`` or ``"other"``),
    /// ``source_issuer_id`` (``None`` unless ``source_kind`` is
    /// ``"from_credit_model"``).
    ///
    /// The frame has zero rows — with the columns still present — when the
    /// decomposer produced no position-level residual allocation.
    #[pyo3(text_signature = "(self)")]
    fn to_position_residual_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = &self.inner.position_residual_contributions;
        let position_ids: Vec<&str> = rows.iter().map(|c| c.position_id.as_str()).collect();
        let residual_variances: Vec<f64> = rows.iter().map(|c| c.residual_variance).collect();
        let source_kinds: Vec<&str> = rows
            .iter()
            .map(|c| match &c.source {
                ResidualContributionSource::FromCreditModel { .. } => "from_credit_model",
                ResidualContributionSource::Other => "other",
            })
            .collect();
        let source_issuer_ids: Vec<Option<&str>> = rows
            .iter()
            .map(|c| match &c.source {
                ResidualContributionSource::FromCreditModel { issuer_id } => {
                    Some(issuer_id.as_str())
                }
                ResidualContributionSource::Other => None,
            })
            .collect();
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("residual_variance", residual_variances)?;
        data.set_item("source_kind", source_kinds)?;
        data.set_item("source_issuer_id", source_issuer_ids)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "RiskDecomposition(total_risk={}, residual_risk={}, factors={}, position_factors={}, position_residuals={})",
            self.inner.total_risk,
            self.inner.residual_risk,
            self.inner.factor_contributions.len(),
            self.inner.position_factor_contributions.len(),
            self.inner.position_residual_contributions.len(),
        )
    }
}

/// Per-position component VaR and marginal VaR.
#[pyclass(
    name = "PositionVarContribution",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPositionVarContribution {
    pub(crate) inner: PositionVarContribution,
}

impl PyPositionVarContribution {
    fn from_inner(inner: PositionVarContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionVarContribution {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: PositionVarContribution = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Portfolio position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Euler-allocated share of portfolio VaR for this position.
    ///
    /// Returns
    /// -------
    /// float
    ///     Portfolio-currency amount following the workspace loss convention
    ///     (losses are **negative**). Component VaRs sum to portfolio VaR.
    #[getter]
    fn component_var(&self) -> f64 {
        self.inner.component_var
    }

    /// Component VaR as a **fraction** (not a percentage) of portfolio VaR.
    ///
    /// Returns
    /// -------
    /// float
    ///     Sums to 1.0 across positions. A negative value marks a diversifier.
    #[getter]
    fn relative_var(&self) -> f64 {
        self.inner.relative_var
    }

    /// Per-unit sensitivity of portfolio VaR to this position.
    ///
    /// Returns
    /// -------
    /// float | None
    ///     Same loss sign convention as :attr:`component_var`; ``None`` when
    ///     the engine could not produce a true gradient.
    #[getter]
    fn marginal_var(&self) -> Option<f64> {
        self.inner.marginal_var
    }

    /// Change in portfolio VaR from removing this position entirely.
    ///
    /// Returns
    /// -------
    /// float | None
    ///     ``None`` unless incremental VaR was requested in the decomposition
    ///     config (it costs one portfolio revaluation per position).
    #[getter]
    fn incremental_var(&self) -> Option<f64> {
        self.inner.incremental_var
    }

    /// Export this contribution as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``component_var``, ``relative_var``,
    /// ``marginal_var``, ``incremental_var``.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionVarContribution(position_id={:?}, component_var={}, relative_var={}, marginal_var={}, incremental_var={})",
            self.inner.position_id.as_str(),
            self.inner.component_var,
            self.inner.relative_var,
            py_opt(self.inner.marginal_var),
            py_opt(self.inner.incremental_var),
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Per-position component ES and marginal ES.
#[pyclass(
    name = "PositionEsContribution",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPositionEsContribution {
    pub(crate) inner: PositionEsContribution,
}

impl PyPositionEsContribution {
    fn from_inner(inner: PositionEsContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionEsContribution {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: PositionEsContribution = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Portfolio position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// This position's contribution to portfolio Expected Shortfall, in
    /// portfolio currency and following the loss (negative) convention.
    #[getter]
    fn component_es(&self) -> f64 {
        self.inner.component_es
    }

    /// Component ES as a **fraction** (not a percentage) of portfolio ES.
    #[getter]
    fn relative_es(&self) -> f64 {
        self.inner.relative_es
    }

    /// Per-unit sensitivity of portfolio ES to this position; ``None`` when the
    /// engine cannot produce a true gradient.
    #[getter]
    fn marginal_es(&self) -> Option<f64> {
        self.inner.marginal_es
    }

    /// Export this contribution as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``component_es``, ``relative_es``,
    /// ``marginal_es``.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionEsContribution(position_id={:?}, component_es={}, relative_es={}, marginal_es={})",
            self.inner.position_id.as_str(),
            self.inner.component_es,
            self.inner.relative_es,
            py_opt(self.inner.marginal_es),
        )
    }
}

/// Complete position-level risk decomposition.
#[pyclass(
    name = "PositionRiskDecomposition",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPositionRiskDecomposition {
    pub(crate) inner: PositionRiskDecomposition,
}

impl PyPositionRiskDecomposition {
    pub(super) fn from_inner(inner: PositionRiskDecomposition) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionRiskDecomposition {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: PositionRiskDecomposition = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Total portfolio Value-at-Risk.
    ///
    /// Returns
    /// -------
    /// float
    ///     Portfolio-currency amount under the workspace loss convention, so
    ///     losses are reported as **negative** numbers.
    #[getter]
    fn portfolio_var(&self) -> f64 {
        self.inner.portfolio_var
    }

    /// Total portfolio Expected Shortfall.
    ///
    /// Returns
    /// -------
    /// float
    ///     Same loss convention as :attr:`portfolio_var`; ES sits at or beyond
    ///     VaR in the loss tail (``portfolio_es <= portfolio_var``).
    #[getter]
    fn portfolio_es(&self) -> f64 {
        self.inner.portfolio_es
    }

    /// Confidence level used for both VaR and ES.
    ///
    /// Returns
    /// -------
    /// float
    ///     A **fraction** strictly inside ``(0.5, 1)`` — ``0.99``, not ``99``.
    #[getter]
    fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    /// Number of positions included in the decomposition.
    #[getter]
    fn n_positions(&self) -> usize {
        self.inner.n_positions
    }

    /// Decomposition method: ``"parametric"`` or ``"historical"``.
    #[getter]
    fn method(&self) -> PyResult<String> {
        decomposition_method_label(self.inner.method)
    }

    /// Parametric-mode numerical residual; ``None`` in historical mode.
    #[getter]
    fn euler_residual(&self) -> Option<f64> {
        self.inner.euler_residual
    }

    /// Per-position VaR decomposition.
    #[getter]
    fn var_contributions(&self) -> Vec<PyPositionVarContribution> {
        self.inner
            .var_contributions
            .iter()
            .cloned()
            .map(PyPositionVarContribution::from_inner)
            .collect()
    }

    /// Per-position Expected Shortfall decomposition.
    #[getter]
    fn es_contributions(&self) -> Vec<PyPositionEsContribution> {
        self.inner
            .es_contributions
            .iter()
            .cloned()
            .map(PyPositionEsContribution::from_inner)
            .collect()
    }

    /// Export the joined per-position VaR and ES decomposition as a pandas
    /// ``DataFrame``.
    ///
    /// ``var_contributions`` and ``es_contributions`` are both keyed by
    /// position, so they are joined on ``position_id`` into one frame. Rows
    /// follow ``var_contributions`` order; an ES column is ``None`` for a
    /// position that has no matching ES entry.
    ///
    /// The portfolio-level scalars (:attr:`portfolio_var`,
    /// :attr:`portfolio_es`, :attr:`confidence`, :attr:`method`) are header
    /// metadata and are deliberately **not** repeated on every row.
    ///
    /// Columns: ``position_id``, ``component_var``, ``relative_var``,
    /// ``marginal_var``, ``incremental_var``, ``component_es``,
    /// ``relative_es``, ``marginal_es``.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let es_rows = &self.inner.es_contributions;
        let es_by_position: HashMap<&str, &PositionEsContribution> = es_rows
            .iter()
            .map(|c| (c.position_id.as_str(), c))
            .collect();
        let rows = &self.inner.var_contributions;
        let position_ids: Vec<&str> = rows.iter().map(|c| c.position_id.as_str()).collect();
        let component_var: Vec<f64> = rows.iter().map(|c| c.component_var).collect();
        let relative_var: Vec<f64> = rows.iter().map(|c| c.relative_var).collect();
        let marginal_var: Vec<Option<f64>> = rows.iter().map(|c| c.marginal_var).collect();
        let incremental_var: Vec<Option<f64>> = rows.iter().map(|c| c.incremental_var).collect();
        let matched: Vec<Option<&PositionEsContribution>> = position_ids
            .iter()
            .map(|pid| es_by_position.get(pid).copied())
            .collect();
        let component_es: Vec<Option<f64>> =
            matched.iter().map(|e| e.map(|c| c.component_es)).collect();
        let relative_es: Vec<Option<f64>> =
            matched.iter().map(|e| e.map(|c| c.relative_es)).collect();
        let marginal_es: Vec<Option<f64>> = matched
            .iter()
            .map(|e| e.and_then(|c| c.marginal_es))
            .collect();

        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("component_var", component_var)?;
        data.set_item("relative_var", relative_var)?;
        data.set_item("marginal_var", marginal_var)?;
        data.set_item("incremental_var", incremental_var)?;
        data.set_item("component_es", component_es)?;
        data.set_item("relative_es", relative_es)?;
        data.set_item("marginal_es", marginal_es)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionRiskDecomposition(portfolio_var={}, portfolio_es={}, confidence={}, n_positions={}, method={:?}, euler_residual={})",
            self.inner.portfolio_var,
            self.inner.portfolio_es,
            self.inner.confidence,
            self.inner.n_positions,
            decomposition_method_label(self.inner.method).unwrap_or_else(|_| "?".to_string()),
            py_opt(self.inner.euler_residual),
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}
