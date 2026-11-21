use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::core_to_py;
use finstack_quant_core::wire::serde_label;
use finstack_quant_factor::risk::{
    FactorContribution, PositionEsContribution, PositionFactorContribution,
    PositionResidualContribution, PositionRiskDecomposition, PositionVarContribution,
    ResidualContributionSource, RiskDecomposition,
};
use pyo3::prelude::*;
use std::collections::HashMap;

/// Source kind label and issuer id of a residual contribution.
fn residual_source_parts(source: &ResidualContributionSource) -> (&'static str, Option<&str>) {
    match source {
        ResidualContributionSource::FromCreditModel { issuer_id } => {
            ("from_credit_model", Some(issuer_id.as_str()))
        }
        ResidualContributionSource::Other => ("other", None),
    }
}

fn factor_contributions_table(rows: &[FactorContribution]) -> PyResult<PyArrowTable> {
    let factor_ids: Vec<String> = rows
        .iter()
        .map(|c| c.factor_id.as_str().to_owned())
        .collect();
    let absolute_risks: Vec<f64> = rows.iter().map(|c| c.absolute_risk).collect();
    let relative_risks: Vec<f64> = rows.iter().map(|c| c.relative_risk).collect();
    let marginal_risks: Vec<f64> = rows.iter().map(|c| c.marginal_risk).collect();
    TableBuilder::new()
        .utf8("factor_id", factor_ids)?
        .f64("absolute_risk", absolute_risks)?
        .f64("relative_risk", relative_risks)?
        .f64("marginal_risk", marginal_risks)?
        .build()
}

fn position_factor_contributions_table(
    rows: &[PositionFactorContribution],
) -> PyResult<PyArrowTable> {
    let position_ids: Vec<String> = rows
        .iter()
        .map(|c| c.position_id.as_str().to_owned())
        .collect();
    let factor_ids: Vec<String> = rows
        .iter()
        .map(|c| c.factor_id.as_str().to_owned())
        .collect();
    let risk_contributions: Vec<f64> = rows.iter().map(|c| c.risk_contribution).collect();
    TableBuilder::new()
        .utf8("position_id", position_ids)?
        .utf8("factor_id", factor_ids)?
        .f64("risk_contribution", risk_contributions)?
        .build()
}

fn position_residual_contributions_table(
    rows: &[PositionResidualContribution],
) -> PyResult<PyArrowTable> {
    let position_ids: Vec<String> = rows
        .iter()
        .map(|c| c.position_id.as_str().to_owned())
        .collect();
    let residual_variances: Vec<f64> = rows.iter().map(|c| c.residual_variance).collect();
    let (source_kinds, source_issuer_ids): (Vec<String>, Vec<Option<String>>) = rows
        .iter()
        .map(|c| {
            let (kind, issuer) = residual_source_parts(&c.source);
            (kind.to_owned(), issuer.map(str::to_owned))
        })
        .unzip();
    TableBuilder::new()
        .utf8("position_id", position_ids)?
        .f64("residual_variance", residual_variances)?
        .utf8("source_kind", source_kinds)?
        .nullable_utf8("source_issuer_id", source_issuer_ids)?
        .build()
}

#[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "FactorContribution")]
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

wire_methods!(
    PyFactorContribution,
    FactorContribution,
    "FactorContribution",
    module = "finstack_quant.models.factor.risk._contributions"
);

py_getters!(PyFactorContribution |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "FactorContribution.factor_id")]
    factor_id -> String = s.inner.factor_id.as_str().to_owned();
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "FactorContribution.absolute_risk")]
    absolute_risk -> f64 = s.inner.absolute_risk;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "FactorContribution.relative_risk")]
    relative_risk -> f64 = s.inner.relative_risk;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "FactorContribution.marginal_risk")]
    marginal_risk -> f64 = s.inner.marginal_risk;
});

#[pymethods]
impl PyFactorContribution {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "FactorContribution.__repr__")]
    fn __repr__(&self) -> String {
        repr_from_serde("FactorContribution", &self.inner)
    }
}

impl PyFactorContribution {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        factor_contributions_table(std::slice::from_ref(&self.inner))
    }
}

tabular_methods!(
    PyFactorContribution,
    "FactorContribution",
    module = "finstack_quant.models.factor.risk._contributions"
);

#[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionFactorContribution")]
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

wire_methods!(
    PyPositionFactorContribution,
    PositionFactorContribution,
    "PositionFactorContribution",
    module = "finstack_quant.models.factor.risk._contributions"
);

py_getters!(PyPositionFactorContribution |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionFactorContribution.position_id")]
    position_id -> String = s.inner.position_id.as_str().to_owned();
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionFactorContribution.factor_id")]
    factor_id -> String = s.inner.factor_id.as_str().to_owned();
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionFactorContribution.risk_contribution")]
    risk_contribution -> f64 = s.inner.risk_contribution;
});

#[pymethods]
impl PyPositionFactorContribution {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionFactorContribution.__repr__")]
    fn __repr__(&self) -> String {
        repr_from_serde("PositionFactorContribution", &self.inner)
    }
}

impl PyPositionFactorContribution {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        position_factor_contributions_table(std::slice::from_ref(&self.inner))
    }
}

tabular_methods!(
    PyPositionFactorContribution,
    "PositionFactorContribution",
    module = "finstack_quant.models.factor.risk._contributions"
);

#[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionResidualContribution")]
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

wire_methods!(
    PyPositionResidualContribution,
    PositionResidualContribution,
    "PositionResidualContribution",
    module = "finstack_quant.models.factor.risk._contributions"
);

py_getters!(PyPositionResidualContribution |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionResidualContribution.position_id")]
    position_id -> String = s.inner.position_id.as_str().to_owned();
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionResidualContribution.residual_variance")]
    residual_variance -> f64 = s.inner.residual_variance;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionResidualContribution.source_kind")]
    source_kind -> &'static str = residual_source_parts(&s.inner.source).0;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionResidualContribution.source_issuer_id")]
    source_issuer_id -> Option<String> = residual_source_parts(&s.inner.source).1.map(str::to_owned);
});

#[pymethods]
impl PyPositionResidualContribution {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionResidualContribution.__repr__")]
    fn __repr__(&self) -> String {
        repr_from_serde("PositionResidualContribution", &self.inner)
    }
}

impl PyPositionResidualContribution {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        position_residual_contributions_table(std::slice::from_ref(&self.inner))
    }
}

tabular_methods!(
    PyPositionResidualContribution,
    "PositionResidualContribution",
    module = "finstack_quant.models.factor.risk._contributions"
);

#[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition")]
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

wire_methods!(
    PyRiskDecomposition,
    RiskDecomposition,
    "RiskDecomposition",
    module = "finstack_quant.models.factor.risk._contributions"
);

py_getters!(PyRiskDecomposition |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition.total_risk")]
    total_risk -> f64 = s.inner.total_risk;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition.residual_risk")]
    residual_risk -> f64 = s.inner.residual_risk;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition.factor_contributions")]
    factor_contributions -> Vec<PyFactorContribution> = s
        .inner
        .factor_contributions
        .iter()
        .cloned()
        .map(PyFactorContribution::from_inner)
        .collect();
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition.position_factor_contributions")]
    position_factor_contributions -> Vec<PyPositionFactorContribution> = s
        .inner
        .position_factor_contributions
        .iter()
        .cloned()
        .map(PyPositionFactorContribution::from_inner)
        .collect();
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition.position_residual_contributions")]
    position_residual_contributions -> Vec<PyPositionResidualContribution> = s
        .inner
        .position_residual_contributions
        .iter()
        .cloned()
        .map(PyPositionResidualContribution::from_inner)
        .collect();
});

#[pymethods]
impl PyRiskDecomposition {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition.measure")]
    #[getter]
    fn measure<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.measure)
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition.to_factor_table")]
    #[pyo3(text_signature = "($self)")]
    fn to_factor_table(&self) -> PyResult<PyArrowTable> {
        factor_contributions_table(&self.inner.factor_contributions)
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition.to_position_factor_table")]
    #[pyo3(text_signature = "($self)")]
    fn to_position_factor_table(&self) -> PyResult<PyArrowTable> {
        position_factor_contributions_table(&self.inner.position_factor_contributions)
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition.to_position_residual_table")]
    #[pyo3(text_signature = "($self)")]
    fn to_position_residual_table(&self) -> PyResult<PyArrowTable> {
        position_residual_contributions_table(&self.inner.position_residual_contributions)
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "RiskDecomposition.__repr__")]
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

impl PyRiskDecomposition {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        self.to_factor_table()
    }
}

tabular_methods!(
    PyRiskDecomposition,
    "RiskDecomposition",
    module = "finstack_quant.models.factor.risk._contributions"
);

#[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionVarContribution")]
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

wire_methods!(
    PyPositionVarContribution,
    PositionVarContribution,
    "PositionVarContribution",
    module = "finstack_quant.models.factor.risk._contributions"
);

py_getters!(PyPositionVarContribution |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionVarContribution.position_id")]
    position_id -> String = s.inner.position_id.as_str().to_owned();
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionVarContribution.component_var")]
    component_var -> f64 = s.inner.component_var;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionVarContribution.relative_var")]
    relative_var -> f64 = s.inner.relative_var;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionVarContribution.marginal_var")]
    marginal_var -> Option<f64> = s.inner.marginal_var;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionVarContribution.incremental_var")]
    incremental_var -> Option<f64> = s.inner.incremental_var;
});

#[pymethods]
impl PyPositionVarContribution {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionVarContribution.__repr__")]
    fn __repr__(&self) -> String {
        repr_from_serde("PositionVarContribution", &self.inner)
    }
}

impl PyPositionVarContribution {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .utf8(
                "position_id",
                vec![self.inner.position_id.as_str().to_owned()],
            )?
            .f64("component_var", vec![self.inner.component_var])?
            .f64("relative_var", vec![self.inner.relative_var])?
            .nullable_f64("marginal_var", vec![self.inner.marginal_var])?
            .nullable_f64("incremental_var", vec![self.inner.incremental_var])?
            .build()
    }
}

tabular_methods!(
    PyPositionVarContribution,
    "PositionVarContribution",
    module = "finstack_quant.models.factor.risk._contributions"
);

#[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionEsContribution")]
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

wire_methods!(
    PyPositionEsContribution,
    PositionEsContribution,
    "PositionEsContribution",
    module = "finstack_quant.models.factor.risk._contributions"
);

py_getters!(PyPositionEsContribution |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionEsContribution.position_id")]
    position_id -> String = s.inner.position_id.as_str().to_owned();
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionEsContribution.component_es")]
    component_es -> f64 = s.inner.component_es;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionEsContribution.relative_es")]
    relative_es -> f64 = s.inner.relative_es;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionEsContribution.marginal_es")]
    marginal_es -> Option<f64> = s.inner.marginal_es;
});

#[pymethods]
impl PyPositionEsContribution {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionEsContribution.__repr__")]
    fn __repr__(&self) -> String {
        repr_from_serde("PositionEsContribution", &self.inner)
    }
}

impl PyPositionEsContribution {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .utf8(
                "position_id",
                vec![self.inner.position_id.as_str().to_owned()],
            )?
            .f64("component_es", vec![self.inner.component_es])?
            .f64("relative_es", vec![self.inner.relative_es])?
            .nullable_f64("marginal_es", vec![self.inner.marginal_es])?
            .build()
    }
}

tabular_methods!(
    PyPositionEsContribution,
    "PositionEsContribution",
    module = "finstack_quant.models.factor.risk._contributions"
);

#[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionRiskDecomposition")]
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
    pub(crate) fn from_inner(inner: PositionRiskDecomposition) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyPositionRiskDecomposition,
    PositionRiskDecomposition,
    "PositionRiskDecomposition",
    module = "finstack_quant.models.factor.risk._contributions"
);

py_getters!(PyPositionRiskDecomposition |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionRiskDecomposition.portfolio_var")]
    portfolio_var -> f64 = s.inner.portfolio_var;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionRiskDecomposition.portfolio_es")]
    portfolio_es -> f64 = s.inner.portfolio_es;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionRiskDecomposition.confidence")]
    confidence -> f64 = s.inner.confidence;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionRiskDecomposition.n_positions")]
    n_positions -> usize = s.inner.n_positions;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionRiskDecomposition.euler_residual")]
    euler_residual -> Option<f64> = s.inner.euler_residual;
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionRiskDecomposition.var_contributions")]
    var_contributions -> Vec<PyPositionVarContribution> = s
        .inner
        .var_contributions
        .iter()
        .cloned()
        .map(PyPositionVarContribution::from_inner)
        .collect();
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionRiskDecomposition.es_contributions")]
    es_contributions -> Vec<PyPositionEsContribution> = s
        .inner
        .es_contributions
        .iter()
        .cloned()
        .map(PyPositionEsContribution::from_inner)
        .collect();
});

#[pymethods]
impl PyPositionRiskDecomposition {
    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionRiskDecomposition.method")]
    #[getter]
    fn method(&self) -> PyResult<String> {
        serde_label(&self.inner.method).map_err(core_to_py)
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._contributions", "PositionRiskDecomposition.__repr__")]
    fn __repr__(&self) -> String {
        format!(
            "PositionRiskDecomposition(portfolio_var={}, portfolio_es={}, confidence={}, n_positions={}, method={:?}, euler_residual={})",
            self.inner.portfolio_var,
            self.inner.portfolio_es,
            self.inner.confidence,
            self.inner.n_positions,
            serde_label(&self.inner.method).unwrap_or_else(|_| "?".to_owned()),
            self.inner
                .euler_residual
                .map_or_else(|| "None".to_owned(), |v| v.to_string()),
        )
    }
}

impl PyPositionRiskDecomposition {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let es_rows = &self.inner.es_contributions;
        let es_by_position: HashMap<&str, &PositionEsContribution> = es_rows
            .iter()
            .map(|c| (c.position_id.as_str(), c))
            .collect();
        let rows = &self.inner.var_contributions;
        let position_ids: Vec<String> = rows
            .iter()
            .map(|c| c.position_id.as_str().to_owned())
            .collect();
        let component_var: Vec<f64> = rows.iter().map(|c| c.component_var).collect();
        let relative_var: Vec<f64> = rows.iter().map(|c| c.relative_var).collect();
        let marginal_var: Vec<Option<f64>> = rows.iter().map(|c| c.marginal_var).collect();
        let incremental_var: Vec<Option<f64>> = rows.iter().map(|c| c.incremental_var).collect();
        let matched: Vec<Option<&PositionEsContribution>> = position_ids
            .iter()
            .map(|pid| es_by_position.get(pid.as_str()).copied())
            .collect();
        let component_es: Vec<Option<f64>> =
            matched.iter().map(|e| e.map(|c| c.component_es)).collect();
        let relative_es: Vec<Option<f64>> =
            matched.iter().map(|e| e.map(|c| c.relative_es)).collect();
        let marginal_es: Vec<Option<f64>> = matched
            .iter()
            .map(|e| e.and_then(|c| c.marginal_es))
            .collect();

        TableBuilder::new()
            .utf8("position_id", position_ids)?
            .f64("component_var", component_var)?
            .f64("relative_var", relative_var)?
            .nullable_f64("marginal_var", marginal_var)?
            .nullable_f64("incremental_var", incremental_var)?
            .nullable_f64("component_es", component_es)?
            .nullable_f64("relative_es", relative_es)?
            .nullable_f64("marginal_es", marginal_es)?
            .build()
    }
}

tabular_methods!(
    PyPositionRiskDecomposition,
    "PositionRiskDecomposition",
    module = "finstack_quant.models.factor.risk._contributions"
);
