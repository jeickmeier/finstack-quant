use pyo3::prelude::*;

use crate::bindings::core::money::PyMoney;
use crate::bindings::valuations::convert::money_to_py;
use crate::errors::core_to_py;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    Tranche, TrancheStructure,
};

use super::PyTranche;

/// Typed wrapper for the Rust `TrancheStructure` (capital structure).
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "TrancheStructure",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyTrancheStructure {
    /// Inner canonical Rust tranche structure.
    pub(crate) inner: TrancheStructure,
}

#[pymethods]
impl PyTrancheStructure {
    /// Capital structure formed from a list of tranches.
    ///
    /// Validates that attachment/detachment points tile ``[0, 100]`` without
    /// gaps or overlaps, that every tranche shares one currency, and assigns
    /// each tranche a distinct, strictly-increasing ``payment_priority``
    /// ranked by seniority (see Rust ``TrancheStructure::new``).
    ///
    /// Parameters
    /// ----------
    /// tranches : list[Tranche]
    ///     Tranches forming the capital structure.
    ///
    /// Returns
    /// -------
    /// TrancheStructure
    ///     The validated tranche structure.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``tranches`` is empty, has non-finite attachment/detachment
    ///     points, leaves a gap/overlap, doesn't tile to 100%, or mixes
    ///     currencies.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import Tranche, TrancheStructure
    /// >>> senior = (
    /// ...     Tranche.builder()
    /// ...     .id("A")
    /// ...     .attachment_point(10.0)
    /// ...     .detachment_point(100.0)
    /// ...     .seniority("senior")
    /// ...     .original_balance(Money(72_000_000.0, Currency("USD")))
    /// ...     .coupon_fixed(0.05)
    /// ...     .maturity(datetime.date(2031, 1, 15))
    /// ...     .build()
    /// ... )
    /// >>> equity = (
    /// ...     Tranche.builder()
    /// ...     .id("E")
    /// ...     .attachment_point(0.0)
    /// ...     .detachment_point(10.0)
    /// ...     .seniority("equity")
    /// ...     .original_balance(Money(8_000_000.0, Currency("USD")))
    /// ...     .coupon_fixed(0.0)
    /// ...     .maturity(datetime.date(2031, 1, 15))
    /// ...     .build()
    /// ... )
    /// >>> structure = TrancheStructure([senior, equity])
    /// >>> "tranches=2" in repr(structure)
    /// True
    #[new]
    #[pyo3(text_signature = "(tranches)")]
    fn new(tranches: Vec<PyRef<'_, PyTranche>>) -> PyResult<Self> {
        let tranches: Vec<Tranche> = tranches.iter().map(|t| t.inner.clone()).collect();
        let inner = TrancheStructure::new(tranches).map_err(core_to_py)?;
        Ok(Self { inner })
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     Strict JSON object with exactly the fields ``to_json`` writes.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or has the wrong shape.
    /// Build a structure whose attachment and detachment points come from
    /// the balance shares alone (mirrors Rust ``TrancheStructure::from_balances``).
    ///
    /// Any points declared on the tranches are discarded: the first-loss
    /// class attaches at 0, each senior class stacks on top in
    /// payment-priority order and the most senior class detaches at 100.
    ///
    /// Parameters
    /// ----------
    /// tranches : list[Tranche]
    ///     Notes of the capital structure; ``seniority`` and
    ///     ``original_balance`` decide the boundaries.
    ///
    /// Returns
    /// -------
    /// TrancheStructure
    ///     The validated structure with derived points.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the list is empty or the notes mix currencies.
    ///
    /// Examples
    /// --------
    /// >>> import datetime
    /// >>> from finstack_quant.core.currency import Currency
    /// >>> from finstack_quant.core.money import Money
    /// >>> from finstack_quant.valuations.instruments import Tranche, TrancheStructure
    /// >>> def note(id_, seniority, balance):
    /// ...     return (Tranche.builder().id(id_).seniority(seniority).original_balance(Money(balance, Currency("USD")))
    /// ...             .coupon_fixed(0.05).maturity(datetime.date(2031, 1, 15)).build())
    /// >>> structure = TrancheStructure.from_balances([note("A", "senior", 60.0), note("B", "mezzanine", 30.0), note("E", "equity", 10.0)])
    /// >>> [(t.id, t.attachment_point, t.detachment_point) for t in structure.tranches]
    /// [('A', 40.0, 100.0), ('B', 10.0, 40.0), ('E', 0.0, 10.0)]
    #[staticmethod]
    #[pyo3(text_signature = "(tranches)")]
    fn from_balances(tranches: Vec<PyRef<'_, PyTranche>>) -> PyResult<Self> {
        let inner = TrancheStructure::from_balances(
            tranches
                .iter()
                .map(|t| t.inner.clone())
                .collect::<Vec<Tranche>>(),
        )
        .map_err(core_to_py)?;
        Ok(Self { inner })
    }

    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|err| crate::errors::serde_json_to_py(err, "invalid TrancheStructure JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to the canonical JSON wire form.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(crate::errors::display_to_py)
    }

    /// Return every field as a plain ``dict`` (canonical serde shape).
    #[pyo3(text_signature = "($self)")]
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
    }

    /// Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Tranches in payment-priority order.
    #[getter]
    fn tranches(&self) -> Vec<PyTranche> {
        self.inner
            .tranches
            .iter()
            .map(|t| PyTranche { inner: t.clone() })
            .collect()
    }

    /// Total original size of the capital structure.
    #[getter]
    fn total_size(&self) -> PyResult<PyMoney> {
        self.inner
            .total_size()
            .map(money_to_py)
            .map_err(crate::errors::core_to_py)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "TrancheStructure(tranches={}, total_size={})",
            self.inner.tranches.len(),
            self.total_size()?.inner.amount()
        ))
    }
}
