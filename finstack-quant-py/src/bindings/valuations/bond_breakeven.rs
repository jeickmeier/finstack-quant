//! Typed bond breakeven ladders and their presentation.

use super::instruments::PyBond;
use crate::bindings::core::dates::tenor::{PyTenor, extract_tenor};
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::pydoc::pydoc;
use crate::errors::core_to_py;
use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{
    self, BondBreakevenLadder, BondBreakevenRow,
};
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenRow")]
#[pyclass(
    name = "BondBreakevenRow",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
pub struct PyBondBreakevenRow {
    inner: BondBreakevenRow,
}

crate::bindings::macros::wire_methods!(
    PyBondBreakevenRow,
    BondBreakevenRow,
    "BondBreakevenRow",
    module = "finstack_quant.valuations.instruments._bond_breakeven",
    to_dict
);

#[pymethods]
impl PyBondBreakevenRow {
    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "BondBreakevenRow(horizon={}, carry={}, sensitivity={}, breakeven_bp={})",
            self.inner.horizon, self.inner.carry, self.inner.sensitivity, self.inner.breakeven_bp
        )
    }
}

crate::bindings::macros::py_getters!(PyBondBreakevenRow |row| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenRow.horizon")]
    horizon -> PyTenor = PyTenor::from_inner(row.inner.horizon);
    #[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenRow.carry")]
    carry -> f64 = row.inner.carry;
    #[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenRow.sensitivity")]
    sensitivity -> f64 = row.inner.sensitivity;
    #[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenRow.breakeven_bp")]
    breakeven_bp -> f64 = row.inner.breakeven_bp;
});

#[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenLadder")]
#[pyclass(
    name = "BondBreakevenLadder",
    module = "finstack_quant.valuations.instruments",
    frozen,
    skip_from_py_object
)]
pub struct PyBondBreakevenLadder {
    inner: BondBreakevenLadder,
}

crate::bindings::macros::wire_methods!(
    PyBondBreakevenLadder,
    BondBreakevenLadder,
    "BondBreakevenLadder",
    module = "finstack_quant.valuations.instruments._bond_breakeven",
    to_dict
);

crate::bindings::macros::py_getters!(PyBondBreakevenLadder |ladder| {
    #[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenLadder.instrument_id")]
    instrument_id -> String = ladder.inner.instrument_id.clone();
    #[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenLadder.currency")]
    currency -> String = ladder.inner.currency.to_string();
    #[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenLadder.rows")]
    rows -> Vec<PyBondBreakevenRow> = ladder.inner.rows.iter().map(|row| PyBondBreakevenRow { inner: row.clone() }).collect();
});

#[pymethods]
impl PyBondBreakevenLadder {
    #[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenLadder.as_of")]
    #[getter]
    fn as_of<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        date_to_py(py, self.inner.as_of)
    }

    #[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "BondBreakevenLadder.config")]
    #[getter]
    fn config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.config)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "BondBreakevenLadder(instrument_id={:?}, as_of={}, currency={:?}, rows={})",
            self.inner.instrument_id,
            self.inner.as_of,
            self.inner.currency.to_string(),
            self.inner.rows.len()
        )
    }
}

impl PyBondBreakevenLadder {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        PyArrowTable::from_envelope(&self.inner.to_table())
    }
}

crate::bindings::macros::tabular_methods!(
    PyBondBreakevenLadder,
    "BondBreakevenLadder",
    module = "finstack_quant.valuations.instruments._bond_breakeven"
);

#[doc = pydoc!("finstack_quant.valuations.instruments._bond_breakeven", "bond_breakeven_ladder")]
#[pyfunction]
fn bond_breakeven_ladder(
    py: Python<'_>,
    bond: &PyBond,
    market: PyRef<'_, PyMarketContext>,
    as_of: &Bound<'_, PyAny>,
    horizons: Vec<Bound<'_, PyAny>>,
    config: &Bound<'_, PyAny>,
) -> PyResult<PyBondBreakevenLadder> {
    let market = &market.inner;
    let as_of = py_to_date(as_of)?;
    let horizons = horizons
        .iter()
        .map(extract_tenor)
        .collect::<PyResult<Vec<_>>>()?;
    let config = crate::bindings::module_utils::spec_from_py(py, config, "breakeven config")?;
    let bond = &bond.inner;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = market;
    py.detach(|| {
        quote_conversions::bond_breakeven_ladder(
            bond,
            market_ref,
            as_of,
            &horizons,
            config,
            finstack_quant_calibration::recalibration::pricing_options(),
        )
    })
    .map(|inner| PyBondBreakevenLadder { inner })
    .map_err(core_to_py)
}

pub(crate) const EXPORTS: &[&str] = &[
    "BondBreakevenLadder",
    "BondBreakevenRow",
    "bond_breakeven_ladder",
];

/// Register native breakeven ladders and the pricing entry point.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyBondBreakevenLadder>()?;
    module.add_class::<PyBondBreakevenRow>()?;
    module.add_function(wrap_pyfunction!(bond_breakeven_ladder, module)?)?;
    Ok(())
}
