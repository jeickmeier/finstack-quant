//! Prepayment and default rate conversion Python entry points.

use crate::bindings::pydoc::pydoc;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.cashflows.builder._rates", "abs_to_smm")]
#[pyfunction]
#[pyo3(text_signature = "(speed, month)")]
pub(crate) fn abs_to_smm(speed: f64, month: u32) -> PyResult<f64> {
    finstack_quant_cashflows::builder::abs_to_smm(speed, month).map_err(crate::errors::core_to_py)
}

#[doc = pydoc!("finstack_quant.cashflows.builder._rates", "annual_to_monthly")]
#[pyfunction]
#[pyo3(text_signature = "(annual)")]
pub(crate) fn annual_to_monthly(annual: f64) -> PyResult<f64> {
    finstack_quant_cashflows::builder::annual_to_monthly(annual).map_err(crate::errors::core_to_py)
}

#[doc = pydoc!("finstack_quant.cashflows.builder._rates", "monthly_to_annual")]
#[pyfunction]
#[pyo3(text_signature = "(monthly)")]
pub(crate) fn monthly_to_annual(monthly: f64) -> PyResult<f64> {
    finstack_quant_cashflows::builder::monthly_to_annual(monthly).map_err(crate::errors::core_to_py)
}
