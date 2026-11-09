//! Python bindings for published SIFMA settlement dates.

use finstack_quant_core::dates::{
    SifmaSettlementClass, next_sifma_settlement, sifma_settlement_date,
};
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::label_enum;
use crate::errors::core_to_py;

/// Public names registered by this module.
pub const EXPORTS: &[&str] = &[
    "SifmaSettlementClass",
    "sifma_settlement_date",
    "next_sifma_settlement",
];

/// SIFMA good-delivery settlement class for agency MBS TBA trades.
///
/// SIFMA publishes one settlement date per month for each class; the class
/// is determined by the agency program and original term. Immutable,
/// hashable enum-style type.
///
/// Examples
/// --------
/// >>> from finstack_quant.core.dates import SifmaSettlementClass
/// >>> SifmaSettlementClass.from_agency_term("FNMA", 30) == SifmaSettlementClass.A
/// True
#[pyclass(
    name = "SifmaSettlementClass",
    module = "finstack_quant.core.dates",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PySifmaSettlementClass {
    inner: SifmaSettlementClass,
}

label_enum!(PySifmaSettlementClass, SifmaSettlementClass, "SifmaSettlementClass", {
    /// Class A: 30-year conventional (FNMA/FHLMC) and UMBS pools.
    A => A,
    /// Class B: 15-year conventional pools.
    B => B,
    /// Class C: 30-year GNMA pools.
    C => C,
    /// Class D: balloon, ARM and other non-standard programs.
    D => D,
});

#[pymethods]
impl PySifmaSettlementClass {
    /// Infer the settlement class from an agency label and original term in years.
    ///
    /// Parameters
    /// ----------
    /// agency : str
    ///     Agency or program label such as ``"FNMA"``, ``"FHLMC"``, ``"UMBS"``
    ///     or ``"GNMA"`` (case-insensitive).
    /// term_years : int
    ///     Original mortgage term in whole years, normally ``15`` or ``30``.
    ///
    /// Returns
    /// -------
    /// SifmaSettlementClass
    ///     Class used to select the monthly SIFMA delivery date; unknown
    ///     combinations fall into class ``D``. This method does not raise.
    #[classmethod]
    #[pyo3(text_signature = "(cls, agency, term_years)")]
    fn from_agency_term(
        _cls: &Bound<'_, pyo3::types::PyType>,
        agency: &str,
        term_years: u32,
    ) -> Self {
        Self {
            inner: SifmaSettlementClass::from_agency_term(agency, term_years),
        }
    }
}

/// Published SIFMA settlement date for a delivery month and settlement class.
///
/// Parameters
/// ----------
/// month : int
///     Delivery month, 1-12.
/// year : int
///     Delivery year.
/// settlement_class : SifmaSettlementClass
///     Class whose published date applies (classes settle on different days
///     of the same delivery month). Required: there is no default class.
///
/// Returns
/// -------
/// datetime.date
///     The maintained published Settlement date, unchanged.
///
/// Raises
/// ------
/// KeyError
///     If the maintained SIFMA data has no published date for that month,
///     year and class.
/// ValueError
///     If ``month`` is outside 1-12, or the stored date is invalid.
#[pyfunction(name = "sifma_settlement_date")]
#[pyo3(text_signature = "(month, year, settlement_class)")]
fn py_sifma_settlement_date<'py>(
    py: Python<'py>,
    month: u8,
    year: i32,
    settlement_class: PyRef<'_, PySifmaSettlementClass>,
) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(
        py,
        sifma_settlement_date(
            crate::bindings::date_utils::month_from_u8(month)?,
            year,
            settlement_class.inner,
        )
        .map_err(core_to_py)?,
    )
}

/// Next published SIFMA settlement date for a class strictly after `date`.
///
/// Parameters
/// ----------
/// date : datetime.date | str
///     Reference date. A settlement on this date is not returned.
/// settlement_class : SifmaSettlementClass
///     Class whose published dates are searched. Required.
///
/// Returns
/// -------
/// datetime.date
///     The current month's published date when it is later than ``date``,
///     otherwise the following month's published date.
///
/// Raises
/// ------
/// KeyError
///     If a required current- or following-month date is not in the
///     maintained data (gaps are never skipped).
/// TypeError
///     If ``date`` is not date-like.
/// ValueError
///     If ``date`` is not a valid calendar date or has no later month.
#[pyfunction(name = "next_sifma_settlement")]
#[pyo3(text_signature = "(date, settlement_class)")]
fn py_next_sifma_settlement<'py>(
    py: Python<'py>,
    date: &Bound<'_, PyAny>,
    settlement_class: PyRef<'_, PySifmaSettlementClass>,
) -> PyResult<Bound<'py, PyAny>> {
    date_to_py(
        py,
        next_sifma_settlement(py_to_date(date)?, settlement_class.inner).map_err(core_to_py)?,
    )
}

pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySifmaSettlementClass>()?;
    module.add_function(wrap_pyfunction!(py_sifma_settlement_date, module)?)?;
    module.add_function(wrap_pyfunction!(py_next_sifma_settlement, module)?)?;
    Ok(())
}
