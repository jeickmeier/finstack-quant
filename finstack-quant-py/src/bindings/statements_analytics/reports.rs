//! Python bindings for the statement reports: structured credit assessment
//! and the P&L summary.
//!
//! `credit_assessment` returns the typed `CreditAssessment` (with a
//! per-period `series`); `pl_summary_report` returns a `PLSummaryReport`
//! that renders to text, a table envelope, or a pandas frame from one Rust
//! implementation. The `*_text` twins remain for WASM parity.

use crate::bindings::extract::extract_results_ref;
use crate::bindings::pandas_utils::{
    serde_rows_to_dataframe_with_schema, table_to_dataframe, ColumnSchema,
};
use crate::errors::{display_to_py, serde_json_to_py};
use finstack_quant_core::dates::PeriodId;
use finstack_quant_statements::evaluator::StatementResult;
use finstack_quant_statements_analytics::analysis::{
    CreditAssessment, CreditAssessmentPoint, CreditAssessmentReport, PLSummaryReport,
};
use pyo3::prelude::*;

/// Column schema for `CreditAssessment.to_dataframe`.
const SERIES_COLUMNS: [ColumnSchema<'static>; 4] = [
    ("period", "str"),
    ("leverage_ratio", "float64"),
    ("interest_coverage", "float64"),
    ("free_cash_flow", "float64"),
];

fn parse_period(period: &str) -> PyResult<PeriodId> {
    period.parse().map_err(display_to_py)
}

/// One period's structured credit metrics.
#[pyclass(
    name = "CreditAssessmentPoint",
    module = "finstack_quant.statements_analytics",
    eq,
    frozen,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyCreditAssessmentPoint {
    pub(crate) inner: CreditAssessmentPoint,
}

#[pymethods]
impl PyCreditAssessmentPoint {
    /// Period-id string (e.g. ``"2025Q4"``).
    #[getter]
    fn period(&self) -> &str {
        &self.inner.period
    }

    /// Total debt / TTM EBITDA in turns, or ``None`` without a full window.
    #[getter]
    fn leverage_ratio(&self) -> Option<f64> {
        self.inner.leverage_ratio
    }

    /// TTM EBITDA / TTM interest expense in turns, or ``None``.
    #[getter]
    fn interest_coverage(&self) -> Option<f64> {
        self.inner.interest_coverage
    }

    /// Free cash flow at the period in model units, or ``None``.
    #[getter]
    fn free_cash_flow(&self) -> Option<f64> {
        self.inner.free_cash_flow
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditAssessmentPoint", &self.inner)
    }
}

/// Structured credit assessment: leverage, coverage and free cash flow at a
/// period plus the ascending per-period series.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements_analytics import CreditAssessment
/// >>> a = CreditAssessment.from_json('{"period":"2025Q4","leverage_ratio":3.0,"interest_coverage":null,'
/// ...     '"free_cash_flow":null,"series":[]}')
/// >>> a.period, a.leverage_ratio
/// ('2025Q4', 3.0)
#[pyclass(
    name = "CreditAssessment",
    module = "finstack_quant.statements_analytics",
    eq,
    from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyCreditAssessment {
    pub(crate) inner: CreditAssessment,
}

#[pymethods]
impl PyCreditAssessment {
    /// Assessment period-id string (e.g. ``"2025Q4"``).
    #[getter]
    fn period(&self) -> &str {
        &self.inner.period
    }

    /// Leverage ratio at ``period`` in turns, or ``None``.
    #[getter]
    fn leverage_ratio(&self) -> Option<f64> {
        self.inner.leverage_ratio
    }

    /// Interest coverage at ``period`` in turns, or ``None``.
    #[getter]
    fn interest_coverage(&self) -> Option<f64> {
        self.inner.interest_coverage
    }

    /// Free cash flow at ``period``, or ``None``.
    #[getter]
    fn free_cash_flow(&self) -> Option<f64> {
        self.inner.free_cash_flow
    }

    /// Ascending per-period points up to and including ``period``.
    #[getter]
    fn series(&self) -> Vec<PyCreditAssessmentPoint> {
        self.inner
            .series
            .iter()
            .cloned()
            .map(|inner| PyCreditAssessmentPoint { inner })
            .collect()
    }

    /// Export the per-period series as a pandas ``DataFrame``.
    ///
    /// Columns: ``period`` (period-id string), ``leverage_ratio``,
    /// ``interest_coverage`` (turns), ``free_cash_flow`` (model units);
    /// ``NaN`` where a metric is unavailable. One row per period, ascending.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .series
            .iter()
            .map(|point| {
                serde_json::json!({
                    "period": point.period,
                    "leverage_ratio": point.leverage_ratio,
                    "interest_coverage": point.interest_coverage,
                    "free_cash_flow": point.free_cash_flow,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(py, &rows, &SERIES_COLUMNS)
    }

    /// Serialize to canonical JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "CreditAssessment"))
    }

    /// Deserialize from canonical JSON.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``json`` is not a valid ``CreditAssessment`` document.
    #[staticmethod]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid CreditAssessment JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("CreditAssessment", &self.inner)
    }

    /// Render as an HTML table in Jupyter notebooks.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Compute a structured credit assessment (leverage, interest coverage, FCF).
///
/// Parameters
/// ----------
/// results : StatementResult | str
///     A ``StatementResult`` object or a JSON string with ``ebitda``,
///     ``total_debt``, ``interest_expense`` and ``free_cash_flow`` nodes.
/// period : str
///     Period identifier for the assessment (``"2025Q4"``, ``"2025M03"``,
///     ``"FY2025"``). This is a ``PeriodId``, not a date.
///
/// Returns
/// -------
/// CreditAssessment
///     Point-in-time ratios at ``period`` plus the ascending ``series``.
///     Ratios are ``None`` when their inputs have mixed scalar/money
///     representations, different currencies, or non-finite arithmetic.
///     Trailing-year ratios require all annual, semiannual, quarterly or monthly
///     periods, including fiscal periods, or the complete Gregorian daily year
///     ending after the assessment date. Weekly and fiscal-daily ratios are
///     unavailable because the result does not contain their date calendar.
///
/// Raises
/// ------
/// ValueError
///     If ``period`` is not a valid period identifier or ``results`` is
///     malformed JSON.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder, Evaluator
/// >>> from finstack_quant.statements_analytics import credit_assessment
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q4", None)
/// >>> b.value("ebitda", [(f"2025Q{q}", 25.0) for q in range(1, 5)])
/// >>> b.value("total_debt", [(f"2025Q{q}", 300.0) for q in range(1, 5)])
/// >>> credit_assessment(Evaluator().evaluate(b.build()), "2025Q4").leverage_ratio
/// 3.0
#[pyfunction]
#[pyo3(text_signature = "(results, period)")]
fn credit_assessment(results: &Bound<'_, PyAny>, period: &str) -> PyResult<PyCreditAssessment> {
    let results = extract_results_ref(results)?;
    let period = parse_period(period)?;
    Ok(PyCreditAssessment {
        inner: CreditAssessment::compute(&results, period),
    })
}

/// Generate a credit assessment report as formatted text.
///
/// Parameters
/// ----------
/// results : StatementResult | str
///     A ``StatementResult`` object or a JSON string.
/// period : str
///     Period identifier for the assessment (a ``PeriodId``, not a date).
///
/// Returns
/// -------
/// str
///     Formatted credit assessment report text. Trailing-year ratios require a
///     complete annual, semiannual, quarterly, monthly or Gregorian daily year;
///     weekly and fiscal-daily ratios are reported as unavailable.
///
/// Raises
/// ------
/// ValueError
///     If ``period`` is not a valid period identifier or ``results`` is
///     malformed JSON.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder, Evaluator
/// >>> from finstack_quant.statements_analytics import credit_assessment_report_text
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q2", None); b.value("revenue", [("2025Q1", 1.0), ("2025Q2", 2.0)])
/// >>> credit_assessment_report_text(Evaluator().evaluate(b.build()), "2025Q2").startswith("Credit Assessment")
/// True
#[pyfunction]
#[pyo3(text_signature = "(results, period)")]
fn credit_assessment_report_text(results: &Bound<'_, PyAny>, period: &str) -> PyResult<String> {
    let results = extract_results_ref(results)?;
    let period = parse_period(period)?;
    Ok(CreditAssessmentReport::new(&results, period).to_string())
}

/// P&L summary of selected line items across periods.
///
/// Built by ``pl_summary_report``; renders to text, an ``ArrowTable`` or a
/// pandas frame from the same Rust implementation.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder, Evaluator
/// >>> from finstack_quant.statements_analytics import pl_summary_report
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q2", None); b.value("revenue", [("2025Q1", 1.0), ("2025Q2", 2.0)])
/// >>> report = pl_summary_report(Evaluator().evaluate(b.build()), ["revenue"], ["2025Q1", "2025Q2"])
/// >>> list(report.to_dataframe()["value"])
/// [1.0, 2.0]
#[pyclass(
    name = "PLSummaryReport",
    module = "finstack_quant.statements_analytics",
    from_py_object
)]
#[derive(Clone)]
pub struct PyPLSummaryReport {
    results: StatementResult,
    line_items: Vec<String>,
    periods: Vec<PeriodId>,
}

impl PyPLSummaryReport {
    fn report(&self) -> PLSummaryReport<'_> {
        PLSummaryReport::new(&self.results, self.line_items.clone(), self.periods.clone())
    }
}

#[pymethods]
impl PyPLSummaryReport {
    /// Node ids shown as rows.
    #[getter]
    fn line_items(&self) -> Vec<String> {
        self.line_items.clone()
    }

    /// Period-id strings shown as columns.
    #[getter]
    fn periods(&self) -> Vec<String> {
        self.periods.iter().map(ToString::to_string).collect()
    }

    /// Render the box-drawn text table.
    fn to_text(&self) -> String {
        self.report().to_string()
    }

    /// Export the report as a long ``ArrowTable``.
    ///
    /// Columns: ``line_item``, ``period``, ``value`` (nullable; missing line
    /// items are null rather than ``0.0``).
    fn to_table(&self) -> PyResult<crate::bindings::core::table::PyArrowTable> {
        let table = self
            .report()
            .to_table()
            .map_err(crate::errors::core_to_py)?;
        crate::bindings::core::table::PyArrowTable::from_envelope(&table)
    }

    /// Export the report as a long pandas ``DataFrame``.
    ///
    /// Columns: ``line_item``, ``period`` (period-id string), ``value`` (the
    /// node's value in its own units; ``NaN`` where the line item is missing).
    /// One row per (line item, period). Pivot with
    /// ``df.pivot(index="line_item", columns="period", values="value")`` for
    /// the line-items-by-periods layout of the text report.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let table = self
            .report()
            .to_table()
            .map_err(crate::errors::core_to_py)?;
        table_to_dataframe(py, &table)
    }

    fn __str__(&self) -> String {
        self.to_text()
    }

    fn __repr__(&self) -> String {
        format!(
            "PLSummaryReport(line_items={:?}, periods={:?})",
            self.line_items,
            self.periods()
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// Build a P&L summary report for selected line items and periods.
///
/// Parameters
/// ----------
/// results : StatementResult | str
///     A ``StatementResult`` object or a JSON string.
/// line_items : list[str]
///     Node ids to include as rows.
/// periods : list[str]
///     Period-id strings for columns (e.g. ``["2025Q1", "2025Q2"]``).
///
/// Returns
/// -------
/// PLSummaryReport
///     Report with ``to_text()``, ``to_table()`` and ``to_dataframe()``.
///
/// Raises
/// ------
/// ValueError
///     If a period does not parse or ``results`` is malformed JSON.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder, Evaluator
/// >>> from finstack_quant.statements_analytics import pl_summary_report
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q2", None); b.value("revenue", [("2025Q1", 1.0), ("2025Q2", 2.0)])
/// >>> pl_summary_report(Evaluator().evaluate(b.build()), ["revenue"], ["2025Q1"]).periods
/// ['2025Q1']
#[pyfunction]
#[pyo3(text_signature = "(results, line_items, periods)")]
fn pl_summary_report(
    results: &Bound<'_, PyAny>,
    line_items: Vec<String>,
    periods: Vec<String>,
) -> PyResult<PyPLSummaryReport> {
    let results = extract_results_ref(results)?.into_owned();
    let periods = periods
        .iter()
        .map(|p| parse_period(p))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyPLSummaryReport {
        results,
        line_items,
        periods,
    })
}

/// Generate a P&L summary report as formatted text.
///
/// Parameters
/// ----------
/// results : StatementResult | str
///     A ``StatementResult`` object or a JSON string.
/// line_items : list[str]
///     Node ids to include as rows.
/// periods : list[str]
///     Period-id strings for columns.
///
/// Returns
/// -------
/// str
///     Formatted P&L summary report text (same as
///     ``pl_summary_report(...).to_text()``).
///
/// Raises
/// ------
/// ValueError
///     If a period does not parse or ``results`` is malformed JSON.
///
/// Examples
/// --------
/// >>> from finstack_quant.statements import ModelBuilder, Evaluator
/// >>> from finstack_quant.statements_analytics import pl_summary_report_text
/// >>> b = ModelBuilder("m"); b.periods("2025Q1..Q2", None); b.value("revenue", [("2025Q1", 1.0), ("2025Q2", 2.0)])
/// >>> pl_summary_report_text(Evaluator().evaluate(b.build()), ["revenue"], ["2025Q1"]).startswith("P&L Summary")
/// True
#[pyfunction]
#[pyo3(text_signature = "(results, line_items, periods)")]
fn pl_summary_report_text(
    results: &Bound<'_, PyAny>,
    line_items: Vec<String>,
    periods: Vec<String>,
) -> PyResult<String> {
    Ok(pl_summary_report(results, line_items, periods)?.to_text())
}

/// Register report types and functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCreditAssessmentPoint>()?;
    m.add_class::<PyCreditAssessment>()?;
    m.add_class::<PyPLSummaryReport>()?;
    m.add_function(pyo3::wrap_pyfunction!(credit_assessment, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(credit_assessment_report_text, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(pl_summary_report, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(pl_summary_report_text, m)?)?;
    Ok(())
}
