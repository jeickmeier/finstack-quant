//! Typed residual-class analytics (`EquityMetrics`).

use pyo3::prelude::*;

use crate::bindings::date_utils::date_to_py;
use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::bindings::valuations::convert::opt_repr;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::EquityMetrics;

/// Residual-class return analytics of one deterministic projection
/// (:meth:`StructuredCredit.equity_metrics`'s return value): the XIRR of the
/// invested amount against every projected equity distribution, the
/// multiple on invested capital, the NAV and the cash-on-cash series.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import EquityMetrics
/// >>> metrics = EquityMetrics.from_json(
/// ...     '{"tranche_id": "E", "currency": "USD", "invested": 100.0, "irr": 0.12,'
/// ...     ' "moic": 1.5, "nav_pct": 90.0, "cash_on_cash": [["2025-01-15", 0.1]]}'
/// ... )
/// >>> metrics.moic, list(metrics.to_dataframe().columns)
/// (1.5, ['date', 'cash_on_cash'])
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "EquityMetrics",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyEquityMetrics {
    /// Inner canonical Rust metrics.
    pub(crate) inner: EquityMetrics,
}

sc_wire_methods!(PyEquityMetrics, EquityMetrics, "EquityMetrics");

#[pymethods]
impl PyEquityMetrics {
    /// Identifier of the residual class.
    #[getter]
    fn tranche_id(&self) -> String {
        self.inner.tranche_id.clone()
    }

    /// ISO-4217 code of the currency ``invested`` is denominated in.
    #[getter]
    fn currency(&self) -> String {
        self.inner.currency.clone()
    }

    /// Amount invested on the valuation date (balance × purchase price), in
    /// currency units.
    #[getter]
    fn invested(&self) -> f64 {
        self.inner.invested
    }

    /// Annualized XIRR of the equity flows as a decimal, or ``None`` when no
    /// rate solves (for example no positive distribution).
    #[getter]
    fn irr(&self) -> Option<f64> {
        self.inner.irr
    }

    /// Multiple on invested capital: total distributions over ``invested``.
    #[getter]
    fn moic(&self) -> f64 {
        self.inner.moic
    }

    /// Discounted value of the distributions as a percent of the invested
    /// balance.
    #[getter]
    fn nav_pct(&self) -> f64 {
        self.inner.nav_pct
    }

    /// Per-distribution cash-on-cash yield (distribution over ``invested``) as
    /// ``(datetime.date, float)`` pairs.
    #[getter]
    fn cash_on_cash<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .cash_on_cash
            .iter()
            .map(|(date, value)| Ok((date_to_py(py, *date)?, *value)))
            .collect()
    }

    /// Cash-on-cash series as a pandas ``DataFrame``.
    ///
    /// Columns: ``date`` (ISO 8601 string) and ``cash_on_cash`` (decimal
    /// share of the invested amount distributed on that date).
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     One row per distribution date.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the rows cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .cash_on_cash
            .iter()
            .map(|(date, value)| {
                serde_json::json!({
                    "date": date.to_string(),
                    "cash_on_cash": value,
                })
            })
            .collect();
        serde_rows_to_dataframe_with_schema(
            py,
            &rows,
            &[("date", "str"), ("cash_on_cash", "float64")],
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "EquityMetrics(tranche_id={:?}, irr={}, moic={}, nav_pct={})",
            self.inner.tranche_id,
            opt_repr(self.inner.irr),
            self.inner.moic,
            self.inner.nav_pct
        )
    }
}
