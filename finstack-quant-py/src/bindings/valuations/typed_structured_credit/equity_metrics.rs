//! Typed residual-class analytics (`EquityMetrics`).

use pyo3::prelude::*;

use crate::bindings::date_utils::date_to_py;
use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::bindings::valuations::convert::opt_repr;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::EquityMetrics;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "EquityMetrics")]
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
    module = "finstack_quant.valuations.instruments",
    to_dict
);

#[pymethods]
impl PyEquityMetrics {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "EquityMetrics.cash_on_cash")]
    #[getter]
    fn cash_on_cash<'py>(&self, py: Python<'py>) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        self.inner
            .cash_on_cash
            .iter()
            .map(|(date, value)| Ok((date_to_py(py, *date)?, *value)))
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "EquityMetrics.to_dataframe")]
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

crate::bindings::macros::py_getters!(PyEquityMetrics |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "EquityMetrics.tranche_id")]
    tranche_id -> String = s.inner.tranche_id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "EquityMetrics.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "EquityMetrics.invested")]
    invested -> f64 = s.inner.invested;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "EquityMetrics.irr")]
    irr -> Option<f64> = s.inner.irr;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "EquityMetrics.moic")]
    moic -> f64 = s.inner.moic;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "EquityMetrics.nav_pct")]
    nav_pct -> f64 = s.inner.nav_pct;
});
