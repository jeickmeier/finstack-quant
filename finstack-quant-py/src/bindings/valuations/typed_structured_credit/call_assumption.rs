//! Typed optional-redemption assumption (`CallAssumption`).

use pyo3::prelude::*;

use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::pandas_utils::serde_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    CallAssumption, CallScope,
};

/// Assumed optional redemption for price-to-call analytics.
///
/// A deal-scope call liquidates the collateral on the first payment date at
/// or after ``date`` and redeems every note at ``price_pct`` of its balance;
/// a tranche-scope call (``tranche_id`` given) leaves the deal's cashflows
/// unchanged and only truncates that class's ``*_to_call`` metrics.
///
/// Examples
/// --------
/// >>> import datetime
/// >>> from finstack_quant.valuations.instruments import CallAssumption
/// >>> call = CallAssumption(datetime.date(2027, 1, 15), 100.0)
/// >>> call.scope, call.tranche_id
/// ('deal', None)
/// >>> CallAssumption(datetime.date(2027, 1, 15), 101.0, tranche_id="B").tranche_id
/// 'B'
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "CallAssumption",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCallAssumption {
    /// Inner canonical Rust call assumption.
    pub(crate) inner: CallAssumption,
}

sc_wire_methods!(PyCallAssumption, CallAssumption, "CallAssumption");

#[pymethods]
impl PyCallAssumption {
    /// Construct a call assumption.
    ///
    /// Parameters
    /// ----------
    /// date : datetime.date
    ///     Assumed call date; the redemption happens on the first payment
    ///     date at or after it.
    /// price_pct : float
    ///     Redemption price as a percent of the note balance (``100.0`` =
    ///     par; a premium is paid as interest, a discount is a write-down).
    /// tranche_id : str, optional
    ///     Restrict the call to one class (tranche-scope). ``None`` calls the
    ///     whole deal.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``date`` is not a valid date.
    #[new]
    #[pyo3(signature = (date, price_pct, tranche_id=None))]
    #[pyo3(text_signature = "(date, price_pct, tranche_id=None)")]
    fn new(date: &Bound<'_, PyAny>, price_pct: f64, tranche_id: Option<&str>) -> PyResult<Self> {
        let date = extract_date(date)?;
        let inner = match tranche_id {
            Some(id) => CallAssumption::for_tranche(date, price_pct, id),
            None => CallAssumption::new(date, price_pct),
        };
        Ok(Self { inner })
    }

    /// Assumed call date as ``datetime.date``.
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.date)
    }

    /// Redemption price as a percent of balance.
    #[getter]
    fn price_pct(&self) -> f64 {
        self.inner.price_pct
    }

    /// ``"deal"`` or ``"tranche"``.
    #[getter]
    fn scope(&self) -> &'static str {
        match self.inner.scope {
            CallScope::Deal => "deal",
            CallScope::Tranche(_) => "tranche",
        }
    }

    /// The called class for a tranche-scope call, else ``None``.
    #[getter]
    fn tranche_id(&self) -> Option<String> {
        match &self.inner.scope {
            CallScope::Deal => None,
            CallScope::Tranche(id) => Some(id.clone()),
        }
    }

    /// ``CallScope`` serde value (``"deal"`` or ``{"tranche": id}``).
    #[getter]
    fn scope_spec<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scope)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "CallAssumption(date={}, price_pct={}, scope={:?}, tranche_id={})",
            self.inner.date,
            self.inner.price_pct,
            self.scope(),
            crate::bindings::valuations::convert::opt_repr(self.tranche_id())
        )
    }
}
