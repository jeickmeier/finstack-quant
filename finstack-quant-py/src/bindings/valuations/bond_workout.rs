//! Typed bond workout ladder (`BondWorkoutLadder`, `BondWorkoutRow`).

use pyo3::prelude::*;

use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{
    self, BondWorkoutLadder, BondWorkoutRow,
};

use super::convert::{enum_to_py_string, money_to_py};
use super::instruments::PyBond;
use crate::bindings::core::money::PyMoney;
use crate::bindings::core::table::envelope_to_pandas;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::errors::core_to_py;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutRow")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "BondWorkoutRow",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBondWorkoutRow {
    /// Inner canonical Rust ladder row.
    pub(crate) inner: BondWorkoutRow,
}

crate::bindings::macros::wire_methods!(
    PyBondWorkoutRow,
    BondWorkoutRow,
    "BondWorkoutRow",
    module = "finstack_quant.valuations.instruments._bond_workout",
    to_dict
);

#[pymethods]
impl PyBondWorkoutRow {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutRow.date")]
    #[getter]
    fn date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutRow.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.kind)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "BondWorkoutRow(date={}, kind={:?}, yield_rate={:?}, is_worst={})",
            self.inner.date, self.inner.kind, self.inner.yield_rate, self.inner.is_worst
        )
    }
}

crate::bindings::macros::py_getters!(PyBondWorkoutRow |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutRow.redemption_price_pct")]
    redemption_price_pct -> f64 = s.inner.redemption_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutRow.redemption_amount")]
    redemption_amount -> PyMoney = money_to_py(s.inner.redemption_amount);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutRow.make_whole")]
    make_whole -> bool = s.inner.make_whole;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutRow.yield_rate")]
    yield_rate -> Option<f64> = s.inner.yield_rate;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutRow.i_spread")]
    i_spread -> Option<f64> = s.inner.i_spread;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutRow.is_worst")]
    is_worst -> bool = s.inner.is_worst;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutLadder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "BondWorkoutLadder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBondWorkoutLadder {
    /// Inner canonical Rust ladder.
    pub(crate) inner: BondWorkoutLadder,
}

crate::bindings::macros::wire_methods!(
    PyBondWorkoutLadder,
    BondWorkoutLadder,
    "BondWorkoutLadder",
    module = "finstack_quant.valuations.instruments._bond_workout",
    to_dict
);

#[pymethods]
impl PyBondWorkoutLadder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutLadder.as_of")]
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutLadder.quote_date")]
    #[getter]
    fn quote_date<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.quote_date)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutLadder.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        envelope_to_pandas(py, &self.inner.to_table())
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "BondWorkoutLadder(instrument_id={:?}, clean_price_pct={}, rows={})",
            self.inner.instrument_id,
            self.inner.clean_price_pct,
            self.inner.rows.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyBondWorkoutLadder |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutLadder.instrument_id")]
    instrument_id -> String = s.inner.instrument_id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutLadder.clean_price_pct")]
    clean_price_pct -> f64 = s.inner.clean_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutLadder.dirty_price")]
    dirty_price -> PyMoney = money_to_py(s.inner.dirty_price);

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "BondWorkoutLadder.rows")]
    rows -> Vec<PyBondWorkoutRow> = s
        .inner
        .rows
        .iter()
        .map(|inner| PyBondWorkoutRow { inner: inner.clone() })
        .collect();
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_workout", "bond_workout_ladder")]
#[pyfunction]
#[pyo3(text_signature = "(bond, market, as_of)")]
fn bond_workout_ladder(
    py: Python<'_>,
    bond: PyRef<'_, PyBond>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
) -> PyResult<PyBondWorkoutLadder> {
    let market = extract_market(py, market)?;
    let as_of = py_to_date(as_of)?;
    let bond = bond.inner.clone();
    let inner = py
        .detach(move || quote_conversions::bond_workout_ladder(&bond, &market, as_of))
        .map_err(core_to_py)?;
    Ok(PyBondWorkoutLadder { inner })
}

pub(crate) const EXPORTS: &[&str] = &["BondWorkoutLadder", "BondWorkoutRow", "bond_workout_ladder"];

/// Register the workout ladder on the instruments submodule.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBondWorkoutRow>()?;
    m.add_class::<PyBondWorkoutLadder>()?;
    m.add_function(pyo3::wrap_pyfunction!(bond_workout_ladder, m)?)?;
    Ok(())
}
