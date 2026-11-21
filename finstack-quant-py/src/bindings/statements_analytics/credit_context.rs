//! Dated credit context inputs, result evidence and tabular presentation.

use crate::bindings::core::money::PyMoney;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::extract::extract_results_ref;
use crate::bindings::macros::tabular_methods;
use crate::bindings::module_utils::fields_from_py;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::statements_to_py;
use finstack_quant_statements_analytics::analysis as native;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyModule};

#[pyclass(
    name = "CreditContextSpec",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextSpec")]
#[derive(Clone)]
pub(super) struct PyCreditContextSpec {
    pub(super) inner: native::CreditContextSpec,
}

#[pyclass(
    name = "CreditContextReport",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextReport")]
#[derive(Clone)]
pub(super) struct PyCreditContextReport {
    pub(super) inner: native::CreditContextReport,
}

#[pyclass(
    name = "CreditContextMetrics",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics")]
#[derive(Clone)]
pub(super) struct PyCreditContextMetrics {
    pub(super) inner: native::CreditContextMetrics,
}

#[pyclass(
    name = "CreditMeasureRow",
    module = "finstack_quant.statements_analytics",
    frozen,
    from_py_object
)]
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditMeasureRow")]
#[derive(Clone)]
pub(super) struct PyCreditMeasureRow {
    pub(super) inner: native::CreditMeasureRow,
}

#[pymethods]
impl PyCreditContextSpec {
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, borrowing_id, run_id, currency, periods, measures, reference_values, cfads_node=None, earnings_node=None, recognized_interest_node=None, cash_fees_node=None)"
    )]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextSpec.__init__")]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        Ok(Self {
            inner: fields_from_py(py, fields, "CreditContextSpec")?,
        })
    }
}

#[pymethods]
impl PyCreditContextReport {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextReport.borrowing_id")]
    fn borrowing_id(&self) -> &str {
        &self.inner.borrowing_id
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextReport.rows")]
    fn rows(&self) -> Vec<PyCreditMeasureRow> {
        self.inner
            .rows
            .iter()
            .cloned()
            .map(|inner| PyCreditMeasureRow { inner })
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextReport.coverage_to_table")]
    fn coverage_to_table(&self) -> PyResult<PyArrowTable> {
        const COLUMNS: [(&str, &str); 6] = [
            ("measure", "str"),
            ("measured", "int64"),
            ("inapplicable", "int64"),
            ("unavailable", "int64"),
            ("observed_minimum", "float64"),
            ("full_path_minimum", "float64"),
        ];
        serde_rows_to_arrow_table(&self.inner.coverage, &COLUMNS)
    }
}

impl PyCreditContextReport {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        const COLUMNS: [(&str, &str); 6] = [
            ("period", "str"),
            ("measure", "str"),
            ("status", "str"),
            ("ratio", "float64"),
            ("source_id", "str"),
            ("reason", "str"),
        ];
        let rows = self
            .inner
            .rows
            .iter()
            .map(|row| {
                let (status, ratio, reason) = match &row.value {
                    native::CreditMeasureValue::Measured { ratio, .. } => {
                        ("measured", Some(*ratio), None)
                    }
                    native::CreditMeasureValue::Inapplicable => ("inapplicable", None, None),
                    native::CreditMeasureValue::Unavailable { reason } => {
                        ("unavailable", None, Some(reason))
                    }
                };
                serde_json::json!({
                    "period": row.period.to_string(), "measure": row.measure, "status": status,
                    "ratio": ratio, "source_id": row.reference_source_id, "reason": reason,
                })
            })
            .collect::<Vec<_>>();
        serde_rows_to_arrow_table(&rows, &COLUMNS)
    }
}

tabular_methods!(
    PyCreditContextReport,
    "CreditContextReport",
    module = "finstack_quant.statements_analytics._credit_context"
);

#[pymethods]
impl PyCreditMeasureRow {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditMeasureRow.period")]
    fn period(&self) -> String {
        self.inner.period.to_string()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditMeasureRow.measure")]
    fn measure<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.measure)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditMeasureRow.date")]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::date_utils::date_to_py(py, self.inner.date)
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditMeasureRow.ratio")]
    fn ratio(&self) -> Option<f64> {
        match self.inner.value {
            native::CreditMeasureValue::Measured { ratio, .. } => Some(ratio),
            _ => None,
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditMeasureRow.numerator")]
    fn numerator(&self) -> Option<PyMoney> {
        match self.inner.value {
            native::CreditMeasureValue::Measured { numerator, .. } => {
                Some(PyMoney::from_inner(numerator))
            }
            _ => None,
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditMeasureRow.denominator")]
    fn denominator(&self) -> Option<PyMoney> {
        match self.inner.value {
            native::CreditMeasureValue::Measured { denominator, .. } => {
                Some(PyMoney::from_inner(denominator))
            }
            _ => None,
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditMeasureRow.reason")]
    fn reason(&self) -> Option<&str> {
        match &self.inner.value {
            native::CreditMeasureValue::Unavailable { reason } => Some(reason),
            _ => None,
        }
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditMeasureRow.reference_source_id")]
    fn reference_source_id(&self) -> Option<&str> {
        self.inner.reference_source_id.as_deref()
    }
}

#[pymethods]
impl PyCreditContextMetrics {
    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics.dscr")]
    fn dscr(&self) -> Vec<(String, f64)> {
        self.inner
            .dscr
            .iter()
            .map(|(period, ratio)| (period.to_string(), *ratio))
            .collect()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics.dscr_total")]
    fn dscr_total(&self) -> Vec<(String, f64)> {
        self.inner
            .dscr_total
            .iter()
            .map(|(period, ratio)| (period.to_string(), *ratio))
            .collect()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics.dscr_incl_fees")]
    fn dscr_incl_fees(&self) -> Vec<(String, f64)> {
        self.inner
            .dscr_incl_fees
            .iter()
            .map(|(period, ratio)| (period.to_string(), *ratio))
            .collect()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics.interest_coverage")]
    fn interest_coverage(&self) -> Vec<(String, f64)> {
        self.inner
            .interest_coverage
            .iter()
            .map(|(period, ratio)| (period.to_string(), *ratio))
            .collect()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics.ltv")]
    fn ltv(&self) -> Vec<(String, f64)> {
        self.inner
            .ltv
            .iter()
            .map(|(period, ratio)| (period.to_string(), *ratio))
            .collect()
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics.dscr_min")]
    fn dscr_min(&self) -> Option<f64> {
        self.inner.dscr_min
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics.dscr_total_min")]
    fn dscr_total_min(&self) -> Option<f64> {
        self.inner.dscr_total_min
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics.dscr_incl_fees_min")]
    fn dscr_incl_fees_min(&self) -> Option<f64> {
        self.inner.dscr_incl_fees_min
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics.interest_coverage_min")]
    fn interest_coverage_min(&self) -> Option<f64> {
        self.inner.interest_coverage_min
    }

    #[getter]
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "CreditContextMetrics.skipped_periods")]
    fn skipped_periods(&self) -> Vec<String> {
        self.inner
            .skipped_periods
            .iter()
            .map(ToString::to_string)
            .collect()
    }
}

impl PyCreditContextMetrics {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        const COLUMNS: [(&str, &str); 6] = [
            ("period", "str"),
            ("dscr", "float64"),
            ("dscr_total", "float64"),
            ("dscr_incl_fees", "float64"),
            ("interest_coverage", "float64"),
            ("ltv", "float64"),
        ];
        let rows = observations(&self.inner)
            .into_iter()
            .map(|(period, values)| {
                serde_json::json!({
                    "period": period.to_string(), "dscr": values[0], "dscr_total": values[1],
                    "dscr_incl_fees": values[2], "interest_coverage": values[3], "ltv": values[4],
                })
            })
            .collect::<Vec<_>>();
        serde_rows_to_arrow_table(&rows, &COLUMNS)
    }
}

tabular_methods!(
    PyCreditContextMetrics,
    "CreditContextMetrics",
    module = "finstack_quant.statements_analytics._credit_context"
);

pub(super) fn observations(
    metrics: &native::CreditContextMetrics,
) -> std::collections::BTreeMap<finstack_quant_core::dates::PeriodId, [Option<f64>; 5]> {
    let mut rows: std::collections::BTreeMap<_, [Option<f64>; 5]> =
        std::collections::BTreeMap::new();
    let series = [
        &metrics.dscr,
        &metrics.dscr_total,
        &metrics.dscr_incl_fees,
        &metrics.interest_coverage,
        &metrics.ltv,
    ];
    for (slot, values) in series.into_iter().enumerate() {
        for (period, ratio) in values {
            rows.entry(*period).or_default()[slot] = Some(*ratio);
        }
    }
    rows
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements_analytics._credit_context", "evaluate_credit_context")]
#[pyfunction]
fn evaluate_credit_context(
    py: Python<'_>,
    results: &Bound<'_, PyAny>,
    spec: &PyCreditContextSpec,
) -> PyResult<PyCreditContextReport> {
    let input = extract_results_ref(results)?.into_owned();
    let spec = spec.inner.clone();
    let inner = py
        .detach(move || native::evaluate_credit_context(&input, &spec))
        .map_err(statements_to_py)?;
    Ok(PyCreditContextReport { inner })
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCreditContextSpec>()?;
    module.add_class::<PyCreditContextReport>()?;
    module.add_class::<PyCreditContextMetrics>()?;
    module.add_class::<PyCreditMeasureRow>()?;
    module.add_function(wrap_pyfunction!(evaluate_credit_context, module)?)
}
