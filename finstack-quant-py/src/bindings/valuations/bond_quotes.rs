//! Typed bond quote set (`BondQuoteSet`, `compute_quotes`).

use pyo3::prelude::*;

use finstack_quant_valuations::instruments::fixed_income::bond::pricing::quote_conversions::{
    self, BondQuoteSet,
};
use finstack_quant_valuations::instruments::BondQuoteInput;

use super::instruments::PyBond;
use crate::bindings::date_utils::py_to_date;
use crate::bindings::extract::extract_market;
use crate::bindings::pandas_utils::serde_rows_to_dataframe_with_schema;
use crate::errors::core_to_py;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet")]
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "BondQuoteSet",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBondQuoteSet {
    /// Inner canonical Rust quote set.
    pub(crate) inner: BondQuoteSet,
}

crate::bindings::macros::wire_methods!(
    PyBondQuoteSet,
    BondQuoteSet,
    "BondQuoteSet",
    module = "finstack_quant.valuations.instruments._bond_quotes",
    to_dict
);

#[pymethods]
impl PyBondQuoteSet {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.to_dataframe")]
    #[pyo3(text_signature = "($self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let row = serde_json::to_value(&self.inner)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid BondQuoteSet"))?;
        serde_rows_to_dataframe_with_schema(
            py,
            &[row],
            &[
                ("clean_price_currency", "float64"),
                ("clean_price_pct", "float64"),
                ("dirty_price_currency", "float64"),
                ("ytm", "float64"),
                ("ytw", "float64"),
                ("z_spread", "float64"),
                ("discount_margin", "float64"),
                ("oas", "float64"),
                ("asw_par", "float64"),
                ("asw_market", "float64"),
                ("i_spread", "float64"),
                ("japanese_simple_yield", "float64"),
                ("moosmuller_ytm", "float64"),
            ],
        )
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "BondQuoteSet(clean_price_pct={}, ytm={:?}, ytw={:?}, z_spread={:?})",
            self.inner.clean_price_pct, self.inner.ytm, self.inner.ytw, self.inner.z_spread
        )
    }
}

crate::bindings::macros::py_getters!(PyBondQuoteSet |s| {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.clean_price_currency")]
    clean_price_currency -> f64 = s.inner.clean_price_currency;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.clean_price_pct")]
    clean_price_pct -> f64 = s.inner.clean_price_pct;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.dirty_price_currency")]
    dirty_price_currency -> f64 = s.inner.dirty_price_currency;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.ytm")]
    ytm -> Option<f64> = s.inner.ytm;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.ytw")]
    ytw -> Option<f64> = s.inner.ytw;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.z_spread")]
    z_spread -> Option<f64> = s.inner.z_spread;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.discount_margin")]
    discount_margin -> Option<f64> = s.inner.discount_margin;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.oas")]
    oas -> Option<f64> = s.inner.oas;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.asw_par")]
    asw_par -> Option<f64> = s.inner.asw_par;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.asw_market")]
    asw_market -> Option<f64> = s.inner.asw_market;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.i_spread")]
    i_spread -> Option<f64> = s.inner.i_spread;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.japanese_simple_yield")]
    japanese_simple_yield -> Option<f64> = s.inner.japanese_simple_yield;

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "BondQuoteSet.moosmuller_ytm")]
    moosmuller_ytm -> Option<f64> = s.inner.moosmuller_ytm;
});

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.valuations.instruments._bond_quotes", "compute_quotes")]
#[pyfunction]
#[pyo3(text_signature = "(bond, market, as_of, quote_input)")]
fn compute_quotes(
    py: Python<'_>,
    bond: PyRef<'_, PyBond>,
    market: &Bound<'_, PyAny>,
    as_of: &Bound<'_, PyAny>,
    quote_input: &Bound<'_, PyAny>,
) -> PyResult<PyBondQuoteSet> {
    let market = extract_market(py, market)?;
    let as_of = py_to_date(as_of)?;
    let quote_input: BondQuoteInput = match quote_input.extract::<String>() {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid BondQuoteInput"))?,
        Err(_) => crate::bindings::module_utils::py_to_serde(py, quote_input, "BondQuoteInput")?,
    };
    let bond = bond.inner.clone();
    let inner = py
        .detach(move || {
            quote_conversions::compute_quotes(
                &bond,
                &market,
                as_of,
                quote_input,
                finstack_quant_calibration::recalibration::pricing_options(),
            )
        })
        .map_err(core_to_py)?;
    Ok(PyBondQuoteSet { inner })
}

pub(crate) const EXPORTS: &[&str] = &["BondQuoteSet", "compute_quotes"];

/// Register the quote set on the instruments submodule.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBondQuoteSet>()?;
    m.add_function(pyo3::wrap_pyfunction!(compute_quotes, m)?)?;
    Ok(())
}
