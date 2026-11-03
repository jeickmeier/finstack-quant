//! Relative value report (`relative_value_report`): the comps table, peer
//! statistics, regression scatter and composite score of one subject. The request
//! is the Rust serde form, a dict or JSON text, the same document WASM takes.

use super::comps::{
    extract_company_metrics, extractor_label, period_basis_label, PyPeerStats, PyRegressionResult,
    PyRelativeValueResult,
};
use super::{extract_serde_any, py_bool};
use crate::bindings::macros::wire_methods;
use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::bindings::pydoc::pydoc;
use crate::bindings::valuations::convert::enum_to_py_string;
use crate::errors::core_to_py;
use finstack_quant_statements_analytics::analysis::{
    self, RelativeValueCompany, RelativeValueDimension, RelativeValueMultiple, RelativeValuePoint,
    RelativeValueReport, RelativeValueReportConfig,
};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueCompany")]
#[pyclass(
    module = "finstack_quant.statements_analytics",
    name = "RelativeValueCompany",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRelativeValueCompany {
    /// Inner canonical Rust value.
    pub(crate) inner: RelativeValueCompany,
}

wire_methods!(
    PyRelativeValueCompany,
    RelativeValueCompany,
    "RelativeValueCompany",
    module = "finstack_quant.statements_analytics._relative_value",
    to_dict
);

#[pymethods]
impl PyRelativeValueCompany {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RelativeValueCompany(id={:?}, is_subject={}, values={:?})",
            self.inner.id.as_str(),
            py_bool(self.inner.is_subject),
            self.inner.values
        )
    }
}

crate::bindings::macros::py_getters!(PyRelativeValueCompany |s| {
    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueCompany.id")]
    id -> String = s.inner.id.to_string();

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueCompany.is_subject")]
    is_subject -> bool = s.inner.is_subject;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueCompany.values")]
    values -> Vec<Option<f64>> = s.inner.values.clone();
});

#[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueMultiple")]
#[pyclass(
    module = "finstack_quant.statements_analytics",
    name = "RelativeValueMultiple",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRelativeValueMultiple {
    /// Inner canonical Rust value.
    pub(crate) inner: RelativeValueMultiple,
}

wire_methods!(
    PyRelativeValueMultiple,
    RelativeValueMultiple,
    "RelativeValueMultiple",
    module = "finstack_quant.statements_analytics._relative_value",
    to_dict
);

#[pymethods]
impl PyRelativeValueMultiple {
    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueMultiple.multiple")]
    #[getter]
    fn multiple(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.multiple)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RelativeValueMultiple(multiple={:?}, subject_value={:?})",
            self.inner.multiple, self.inner.subject_value
        )
    }
}

crate::bindings::macros::py_getters!(PyRelativeValueMultiple |s| {
    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueMultiple.subject_value")]
    subject_value -> Option<f64> = s.inner.subject_value;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueMultiple.peer_stats")]
    peer_stats -> Option<PyPeerStats> = s.inner.peer_stats.clone().map(|inner| PyPeerStats { inner });

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueMultiple.percentile")]
    percentile -> Option<f64> = s.inner.percentile;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueMultiple.z_score")]
    z_score -> Option<f64> = s.inner.z_score;
});

#[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValuePoint")]
#[pyclass(
    module = "finstack_quant.statements_analytics",
    name = "RelativeValuePoint",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRelativeValuePoint {
    /// Inner canonical Rust value.
    pub(crate) inner: RelativeValuePoint,
}

wire_methods!(
    PyRelativeValuePoint,
    RelativeValuePoint,
    "RelativeValuePoint",
    module = "finstack_quant.statements_analytics._relative_value",
    to_dict
);

#[pymethods]
impl PyRelativeValuePoint {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RelativeValuePoint(company_id={:?}, is_subject={}, x={}, y={}, fitted_y={:?})",
            self.inner.company_id.as_str(),
            py_bool(self.inner.is_subject),
            self.inner.x,
            self.inner.y,
            self.inner.fitted_y
        )
    }
}

crate::bindings::macros::py_getters!(PyRelativeValuePoint |s| {
    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValuePoint.company_id")]
    company_id -> String = s.inner.company_id.to_string();

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValuePoint.is_subject")]
    is_subject -> bool = s.inner.is_subject;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValuePoint.x")]
    x -> f64 = s.inner.x;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValuePoint.y")]
    y -> f64 = s.inner.y;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValuePoint.fitted_y")]
    fitted_y -> Option<f64> = s.inner.fitted_y;
});

#[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension")]
#[pyclass(
    module = "finstack_quant.statements_analytics",
    name = "RelativeValueDimension",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRelativeValueDimension {
    /// Inner canonical Rust value.
    pub(crate) inner: RelativeValueDimension,
}

wire_methods!(
    PyRelativeValueDimension,
    RelativeValueDimension,
    "RelativeValueDimension",
    module = "finstack_quant.statements_analytics._relative_value",
    to_dict
);

#[pymethods]
impl PyRelativeValueDimension {
    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.direction")]
    #[getter]
    fn direction(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.direction)
    }

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.excluded_reason")]
    #[getter]
    fn excluded_reason(&self) -> PyResult<Option<String>> {
        self.inner
            .excluded_reason
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RelativeValueDimension(label={:?}, weight={}, signal={:?}, excluded_reason={:?})",
            self.inner.label, self.inner.weight, self.inner.signal, self.inner.excluded_reason
        )
    }
}

crate::bindings::macros::py_getters!(PyRelativeValueDimension |s| {
    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.label")]
    label -> String = s.inner.label.clone();

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.weight")]
    weight -> f64 = s.inner.weight;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.y")]
    y -> String = extractor_label(&s.inner.y);

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.x")]
    x -> Option<String> = s.inner.x.as_ref().map(extractor_label);

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.subject_y")]
    subject_y -> Option<f64> = s.inner.subject_y;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.subject_x")]
    subject_x -> Option<f64> = s.inner.subject_x;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.peer_stats")]
    peer_stats -> Option<PyPeerStats> = s.inner.peer_stats.clone().map(|inner| PyPeerStats { inner });

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.percentile")]
    percentile -> Option<f64> = s.inner.percentile;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.z_score")]
    z_score -> Option<f64> = s.inner.z_score;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.regression")]
    regression -> Option<PyRegressionResult> = s.inner.regression.clone().map(|inner| PyRegressionResult { inner });

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.signal")]
    signal -> Option<f64> = s.inner.signal;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueDimension.points")]
    points -> Vec<PyRelativeValuePoint> = s
        .inner
        .points
        .iter()
        .map(|inner| PyRelativeValuePoint { inner: inner.clone() })
        .collect();
});

#[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport")]
#[pyclass(
    module = "finstack_quant.statements_analytics",
    name = "RelativeValueReport",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRelativeValueReport {
    /// Inner canonical Rust value.
    pub(crate) inner: RelativeValueReport,
}

wire_methods!(
    PyRelativeValueReport,
    RelativeValueReport,
    "RelativeValueReport",
    module = "finstack_quant.statements_analytics._relative_value",
    to_dict
);

#[pymethods]
impl PyRelativeValueReport {
    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport.composite_excluded_reason")]
    #[getter]
    fn composite_excluded_reason(&self) -> PyResult<Option<String>> {
        self.inner
            .composite_excluded_reason
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels = self
            .inner
            .multiples
            .iter()
            .map(|column| enum_to_py_string(&column.multiple))
            .collect::<PyResult<Vec<_>>>()?;
        let rows: Vec<serde_json::Value> = self
            .inner
            .companies
            .iter()
            .map(|company| {
                let mut row = serde_json::Map::new();
                row.insert("id".to_string(), company.id.as_str().into());
                row.insert("is_subject".to_string(), company.is_subject.into());
                for (label, value) in labels.iter().zip(&company.values) {
                    row.insert(label.clone(), (*value).into());
                }
                serde_json::Value::Object(row)
            })
            .collect();
        let mut columns = vec![("id", "str"), ("is_subject", "bool")];
        columns.extend(labels.iter().map(|label| (label.as_str(), "float64")));
        serde_rows_to_dataframe_with_schema(py, &rows, &columns)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RelativeValueReport(subject_id={:?}, peer_count={}, companies={}, dimensions={})",
            self.inner.subject_id.as_str(),
            self.inner.peer_count,
            self.inner.companies.len(),
            self.inner.dimensions.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyRelativeValueReport |s| {
    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport.subject_id")]
    subject_id -> String = s.inner.subject_id.to_string();

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport.period_basis")]
    period_basis -> String = period_basis_label(&s.inner.period_basis);

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport.peer_count")]
    peer_count -> usize = s.inner.peer_count;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport.excluded_count")]
    excluded_count -> usize = s.inner.excluded_count;

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport.companies")]
    companies -> Vec<PyRelativeValueCompany> = s
        .inner
        .companies
        .iter()
        .map(|inner| PyRelativeValueCompany { inner: inner.clone() })
        .collect();

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport.multiples")]
    multiples -> Vec<PyRelativeValueMultiple> = s
        .inner
        .multiples
        .iter()
        .map(|inner| PyRelativeValueMultiple { inner: inner.clone() })
        .collect();

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport.dimensions")]
    dimensions -> Vec<PyRelativeValueDimension> = s
        .inner
        .dimensions
        .iter()
        .map(|inner| PyRelativeValueDimension { inner: inner.clone() })
        .collect();

    #[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "RelativeValueReport.composite")]
    composite -> Option<PyRelativeValueResult> = s.inner.composite.clone().map(|inner| PyRelativeValueResult { inner });
});

#[doc = pydoc!("finstack_quant.statements_analytics._relative_value", "relative_value_report")]
#[pyfunction]
#[pyo3(text_signature = "(subject, universe, config)")]
fn relative_value_report<'py>(
    py: Python<'py>,
    subject: &Bound<'py, PyAny>,
    universe: Vec<Bound<'py, PyAny>>,
    config: &Bound<'py, PyAny>,
) -> PyResult<PyRelativeValueReport> {
    let subject = extract_company_metrics(py, subject)?;
    let universe = universe
        .iter()
        .map(|company| extract_company_metrics(py, company))
        .collect::<PyResult<Vec<_>>>()?;
    let config: RelativeValueReportConfig = extract_serde_any(py, config, "config")?;
    let inner =
        analysis::relative_value_report(&subject, &universe, &config).map_err(core_to_py)?;
    Ok(PyRelativeValueReport { inner })
}

/// Register the report on the `statements_analytics` submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRelativeValueCompany>()?;
    m.add_class::<PyRelativeValueMultiple>()?;
    m.add_class::<PyRelativeValuePoint>()?;
    m.add_class::<PyRelativeValueDimension>()?;
    m.add_class::<PyRelativeValueReport>()?;
    m.add_function(pyo3::wrap_pyfunction!(relative_value_report, m)?)?;
    Ok(())
}
