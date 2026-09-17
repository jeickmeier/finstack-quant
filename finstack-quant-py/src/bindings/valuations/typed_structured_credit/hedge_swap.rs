//! Typed interest-rate hedge settled through the waterfall (`HedgeSwap`).

use pyo3::prelude::*;

use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::valuations::convert::enum_to_py_string;
use crate::bindings::valuations::typed_rates::PyInterestRateSwap;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    HedgeSwap, SwapNotional, SwapPriority,
};

use super::super::instruments::enum_from_str;

/// An interest-rate swap settled through the deal waterfall: net receipts
/// join interest collections (and the IC test), net payments rank as a
/// senior or junior fee.
///
/// Examples
/// --------
/// >>> from finstack_quant.valuations.instruments import HedgeSwap, InterestRateSwap
/// >>> swap = InterestRateSwap.example_standard()
/// >>> hedge = HedgeSwap(swap, notional="pool_par", priority="senior_fee")
/// >>> hedge.priority, hedge.notional
/// ('senior_fee', 'pool_par')
/// >>> HedgeSwap(swap, notional={"tranche_par": "A"}).notional
/// {'tranche_par': 'A'}
#[pyclass(
    module = "finstack_quant.valuations.instruments",
    name = "HedgeSwap",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyHedgeSwap {
    /// Inner canonical Rust hedge.
    pub(crate) inner: HedgeSwap,
}

sc_wire_methods!(PyHedgeSwap, HedgeSwap, "HedgeSwap");

#[pymethods]
impl PyHedgeSwap {
    /// Construct a hedge from a typed swap.
    ///
    /// Parameters
    /// ----------
    /// swap : InterestRateSwap
    ///     The swap; its side, legs and market dependencies drive the
    ///     projected net settlements.
    /// notional : str | dict, optional
    ///     ``SwapNotional`` serde value: ``"contractual"`` (the swap's own
    ///     notional, the default), ``"pool_par"`` (balance-tracking on the
    ///     pool) or ``{"tranche_par": "<tranche id>"}`` (balance-tracking on
    ///     one class).
    /// priority : str, optional
    ///     ``"senior_fee"`` (default: net payments rank with the senior fees)
    ///     or ``"junior_fee"`` (after every note coupon).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``notional`` or ``priority`` is not a recognized value.
    #[new]
    #[pyo3(signature = (swap, notional=None, priority=None))]
    #[pyo3(text_signature = "(swap, notional=None, priority=None)")]
    fn new(
        py: Python<'_>,
        swap: PyRef<'_, PyInterestRateSwap>,
        notional: Option<&Bound<'_, PyAny>>,
        priority: Option<&str>,
    ) -> PyResult<Self> {
        let notional: SwapNotional = match notional {
            Some(value) => crate::bindings::module_utils::py_to_serde(py, value, "notional")?,
            None => SwapNotional::Contractual,
        };
        let priority: SwapPriority = match priority {
            Some(value) => enum_from_str(value, "priority")?,
            None => SwapPriority::SeniorFee,
        };
        Ok(Self {
            inner: HedgeSwap {
                swap: swap.inner.clone(),
                notional,
                priority,
            },
        })
    }

    /// The hedged swap as a typed ``InterestRateSwap``.
    #[getter]
    fn swap(&self) -> PyInterestRateSwap {
        PyInterestRateSwap {
            inner: self.inner.swap.clone(),
        }
    }

    /// ``SwapNotional`` serde value (``"contractual"``, ``"pool_par"`` or
    /// ``{"tranche_par": id}``).
    #[getter]
    fn notional<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.notional)
    }

    /// ``"senior_fee"`` or ``"junior_fee"``.
    #[getter]
    fn priority(&self) -> PyResult<String> {
        enum_to_py_string(&self.inner.priority)
    }

    /// Return ``repr(self)``.
    fn __repr__(&self) -> String {
        format!(
            "HedgeSwap(swap={:?}, notional={}, priority={})",
            self.inner.swap.id.as_str(),
            serde_json::to_string(&self.inner.notional).unwrap_or_default(),
            enum_to_py_string(&self.inner.priority).unwrap_or_default()
        )
    }
}

/// Coerce ``list[HedgeSwap | dict] | str`` into Rust hedges.
pub(crate) fn hedge_swaps_from_py(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<Vec<HedgeSwap>> {
    if let Ok(items) = value.cast::<pyo3::types::PyList>() {
        let mut hedges = Vec::with_capacity(items.len());
        for item in items.iter() {
            if let Ok(hedge) = item.cast::<PyHedgeSwap>() {
                hedges.push(hedge.borrow().inner.clone());
            } else {
                hedges.push(crate::bindings::module_utils::py_to_serde(
                    py,
                    &item,
                    "hedge_swaps",
                )?);
            }
        }
        return Ok(hedges);
    }
    crate::bindings::module_utils::py_to_serde(py, value, "hedge_swaps")
}
