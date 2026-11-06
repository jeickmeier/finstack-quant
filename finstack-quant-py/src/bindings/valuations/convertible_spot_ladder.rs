//! Typed convertible spot ladder (`ConvertibleSpotLadder`, `ConvertibleSpotRow`).

use pyo3::prelude::*;

use finstack_quant_valuations::instruments::fixed_income::convertible::{
    self, ConvertibleSpotLadder, ConvertibleSpotRow,
};

use super::convert::enum_to_py_string;
use super::typed_convertible::PyConvertibleBond;
use crate::bindings::core::table::envelope_to_pandas;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::extract::extract_market;
use crate::errors::core_to_py;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ConvertibleSpotRow",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyConvertibleSpotRow {
    /// Inner canonical Rust ladder row.
    pub(crate) inner: ConvertibleSpotRow,
}

crate::bindings::macros::wire_methods!(
    PyConvertibleSpotRow,
    ConvertibleSpotRow,
    "ConvertibleSpotRow",
    module = "finstack_quant.valuations.instruments._convertible_spot_ladder",
    to_dict
);

#[pymethods]
impl PyConvertibleSpotRow {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow.regime")]
    #[getter]
    fn regime(&self) -> PyResult<Option<String>> {
        self.inner
            .regime
            .as_ref()
            .map(enum_to_py_string)
            .transpose()
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ConvertibleSpotRow(spot={}, price={}, delta={:?}, regime={:?})",
            self.inner.spot, self.inner.price, self.inner.delta, self.inner.regime
        )
    }
}

crate::bindings::macros::py_getters!(PyConvertibleSpotRow |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow.spot")]
    spot -> f64 = s.inner.spot;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow.is_market_spot")]
    is_market_spot -> bool = s.inner.is_market_spot;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow.price")]
    price -> f64 = s.inner.price;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow.delta")]
    delta -> Option<f64> = s.inner.delta;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow.hedge_ratio")]
    hedge_ratio -> Option<f64> = s.inner.hedge_ratio;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow.conversion_value")]
    conversion_value -> f64 = s.inner.conversion_value;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow.parity")]
    parity -> f64 = s.inner.parity;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow.conversion_premium")]
    conversion_premium -> Option<f64> = s.inner.conversion_premium;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotRow.bond_floor")]
    bond_floor -> f64 = s.inner.bond_floor;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotLadder")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "ConvertibleSpotLadder",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyConvertibleSpotLadder {
    /// Inner canonical Rust ladder.
    pub(crate) inner: ConvertibleSpotLadder,
}

crate::bindings::macros::wire_methods!(
    PyConvertibleSpotLadder,
    ConvertibleSpotLadder,
    "ConvertibleSpotLadder",
    module = "finstack_quant.valuations.instruments._convertible_spot_ladder",
    to_dict
);

#[pymethods]
impl PyConvertibleSpotLadder {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotLadder.as_of")]
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotLadder.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        envelope_to_pandas(py, &self.inner.to_table())
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "ConvertibleSpotLadder(instrument_id={:?}, market_spot={}, rows={})",
            self.inner.instrument_id,
            self.inner.market_spot,
            self.inner.rows.len()
        )
    }
}

crate::bindings::macros::py_getters!(PyConvertibleSpotLadder |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotLadder.instrument_id")]
    instrument_id -> String = s.inner.instrument_id.clone();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotLadder.currency")]
    currency -> String = s.inner.currency.to_string();

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotLadder.market_spot")]
    market_spot -> f64 = s.inner.market_spot;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "ConvertibleSpotLadder.rows")]
    rows -> Vec<PyConvertibleSpotRow> = s
        .inner
        .rows
        .iter()
        .map(|inner| PyConvertibleSpotRow { inner: inner.clone() })
        .collect();
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._convertible_spot_ladder", "convertible_spot_ladder")]
#[pyfunction]
#[pyo3(signature = (bond, market, as_of, spots=None), text_signature = "(bond, market, as_of, spots=None)")]
fn convertible_spot_ladder(
    py: Python<'_>,
    bond: PyRef<'_, PyConvertibleBond>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    spots: Option<Vec<f64>>,
) -> PyResult<PyConvertibleSpotLadder> {
    let market = extract_market(py, market)?;
    let as_of = py_to_date(as_of)?;
    let bond = bond.inner.clone();
    let inner = py
        .detach(move || {
            convertible::convertible_spot_ladder(&bond, &market, as_of, spots.as_deref())
        })
        .map_err(core_to_py)?;
    Ok(PyConvertibleSpotLadder { inner })
}

pub(crate) const EXPORTS: &[&str] = &[
    "ConvertibleSpotLadder",
    "ConvertibleSpotRow",
    "convertible_spot_ladder",
];

/// Register the spot ladder on the instruments submodule.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyConvertibleSpotRow>()?;
    m.add_class::<PyConvertibleSpotLadder>()?;
    m.add_function(pyo3::wrap_pyfunction!(convertible_spot_ladder, m)?)?;
    Ok(())
}
