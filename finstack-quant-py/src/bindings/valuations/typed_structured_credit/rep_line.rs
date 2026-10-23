use pyo3::prelude::*;

use crate::bindings::core::dates::daycount::PyDayCount;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::{extract_basis_points, extract_rate_decimal};
use crate::bindings::valuations::convert::{money_from_py, money_to_py, opt_repr};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::RepLine;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "RepLine",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRepLine {
    /// Inner canonical Rust rep line.
    pub(crate) inner: RepLine,
}

#[pymethods]
impl PyRepLine {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.__init__")]
    #[new]
    #[pyo3(signature = (id, balance, rate, maturity, seasoning_months, day_count, *, asset_type,
                        spread_bp = None, forward_curve_id = None, index_floor_bp = None, cpr = None,
                        cdr = None, recovery_rate = None, contractual_payment = None,
                        amortization_term_months = None, io_months = None))]
    #[pyo3(
        text_signature = "(id, balance, rate, maturity, seasoning_months, day_count, *, asset_type, \
spread_bp=None, forward_curve_id=None, index_floor_bp=None, cpr=None, cdr=None, recovery_rate=None, contractual_payment=None, amortization_term_months=None, io_months=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        id: &str,
        balance: &Bound<'_, PyAny>,
        rate: &Bound<'_, PyAny>,
        maturity: &Bound<'_, PyAny>,
        seasoning_months: u32,
        day_count: PyRef<'_, PyDayCount>,
        asset_type: &Bound<'_, PyAny>,
        spread_bp: Option<&Bound<'_, PyAny>>,
        forward_curve_id: Option<String>,
        index_floor_bp: Option<f64>,
        cpr: Option<f64>,
        cdr: Option<f64>,
        recovery_rate: Option<f64>,
        contractual_payment: Option<PyRef<'_, PyMoney>>,
        amortization_term_months: Option<u32>,
        io_months: Option<u32>,
    ) -> PyResult<Self> {
        let maturity = py_to_date(maturity)?;
        let balance = money_from_py(balance, None, "balance")?;
        let rate = extract_rate_decimal(rate, "rate")?;
        let spread_bp = spread_bp
            .map(|value| extract_basis_points(value, "spread_bp"))
            .transpose()?;
        let mut inner = RepLine::new(
            id,
            balance,
            rate,
            maturity,
            day_count.inner,
            crate::bindings::module_utils::py_to_serde(py, asset_type, "asset_type")?,
        );
        inner.spread_bp = spread_bp;
        inner.forward_curve_id = forward_curve_id;
        inner.index_floor_bp = index_floor_bp;
        inner.seasoning_months = seasoning_months;
        if let Some(cpr) = cpr {
            inner = inner.with_cpr(cpr);
        }
        if let Some(cdr) = cdr {
            inner = inner.with_cdr(cdr);
        }
        if let Some(recovery_rate) = recovery_rate {
            inner = inner.with_recovery_rate(recovery_rate);
        }
        inner.contractual_payment = contractual_payment.map(|money| money.inner);
        inner.amortization_term_months = amortization_term_months;
        inner.io_months = io_months;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.from_json")]
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| crate::errors::serde_json_to_py(err, "invalid RepLine JSON"))?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.to_json")]
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(crate::errors::display_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.to_dict")]
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.__reduce__")]
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.asset_type")]
    #[getter]
    fn asset_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.asset_type)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.maturity")]
    #[getter]
    fn maturity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.maturity)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "RepLine(id='{}', balance={}, currency='{}', rate={}, spread_bp={}, maturity='{}')",
            self.inner.id,
            self.inner.balance.amount(),
            self.inner.balance.currency(),
            self.inner.rate,
            opt_repr(self.inner.spread_bp),
            self.inner.maturity,
        )
    }
}

crate::bindings::macros::py_getters!(PyRepLine |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.id")]
    id -> String = s.inner.id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.balance")]
    balance -> PyMoney = money_to_py(s.inner.balance);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.rate")]
    rate -> f64 = s.inner.rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.spread_bp")]
    spread_bp -> Option<f64> = s.inner.spread_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.forward_curve_id")]
    forward_curve_id -> Option<String> = s.inner.forward_curve_id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.index_floor_bp")]
    index_floor_bp -> Option<f64> = s.inner.index_floor_bp;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.contractual_payment")]
    contractual_payment -> Option<PyMoney> = s.inner.contractual_payment.map(money_to_py);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.amortization_term_months")]
    amortization_term_months -> Option<u32> = s.inner.amortization_term_months;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.io_months")]
    io_months -> Option<u32> = s.inner.io_months;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.seasoning_months")]
    seasoning_months -> u32 = s.inner.seasoning_months;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.day_count")]
    day_count -> PyDayCount = PyDayCount::from_inner(s.inner.day_count);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.cpr")]
    cpr -> Option<f64> = s.inner.cpr;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.cdr")]
    cdr -> Option<f64> = s.inner.cdr;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments", "RepLine.recovery_rate")]
    recovery_rate -> Option<f64> = s.inner.recovery_rate;
});
