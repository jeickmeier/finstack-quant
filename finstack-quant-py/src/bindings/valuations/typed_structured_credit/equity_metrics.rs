//! Typed residual-class analytics (`EquityMetrics`).

use pyo3::prelude::*;

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::table_builders::TableBuilder;
use crate::bindings::valuations::convert::opt_repr;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::EquityMetrics;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics", "EquityMetrics")]
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

crate::bindings::macros::wire_methods!(
    PyEquityMetrics,
    EquityMetrics,
    "EquityMetrics",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics",
    to_dict
);

#[pymethods]
impl PyEquityMetrics {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics", "EquityMetrics.cash_on_cash")]
    #[getter]
    fn cash_on_cash<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, Option<f64>)>> {
        self.inner
            .cash_on_cash
            .iter()
            .map(|(date, value)| Ok((date_to_py(py, *date)?, *value)))
            .collect()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "EquityMetrics(tranche_id={:?}, irr={}, moic={:?}, nav_pct={})",
            self.inner.tranche_id,
            opt_repr(self.inner.irr),
            self.inner.moic,
            self.inner.nav_pct
        )
    }
}

impl PyEquityMetrics {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let mut dates = Vec::with_capacity(self.inner.cash_on_cash.len());
        let mut values = Vec::with_capacity(self.inner.cash_on_cash.len());
        for (date, value) in &self.inner.cash_on_cash {
            dates.push(date.to_string());
            values.push(*value);
        }
        TableBuilder::new()
            .utf8("date", dates)?
            .nullable_f64("cash_on_cash", values)?
            .build()
    }
}

crate::bindings::macros::tabular_methods!(
    PyEquityMetrics,
    "EquityMetrics",
    module = "finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics"
);

crate::bindings::macros::py_getters!(PyEquityMetrics |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics", "EquityMetrics.tranche_id")]
    tranche_id -> String = s.inner.tranche_id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics", "EquityMetrics.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics", "EquityMetrics.invested")]
    invested -> f64 = s.inner.invested;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics", "EquityMetrics.irr")]
    irr -> Option<f64> = s.inner.irr;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics", "EquityMetrics.moic")]
    moic -> Option<f64> = s.inner.moic;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._typed_structured_credit.equity_metrics", "EquityMetrics.nav_pct")]
    nav_pct -> f64 = s.inner.nav_pct;
});
