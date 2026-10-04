//! Typed credit instruments: `CreditDefaultSwap`, `CdsIndex`, `CdsTranche`,
//! `CdsOption` and `ConvertibleBond`, plus their small typed helper classes
//! (`CdsIndexParams`, `CdsIndexConstituent`, `CdsTrancheParams`,
//! `CallPutSchedule`, `ConversionSpec`).
//! Mirrors the `PyBond` pattern in `instruments.rs`.

mod cds;
mod cds_index;
mod cds_option;
mod cds_tranche;
mod convertible;

use pyo3::prelude::*;

pub(crate) use cds::PyCreditDefaultSwap;
use cds::PyCreditDefaultSwapBuilder;
pub(crate) use cds_index::PyCdsIndex;
use cds_index::PyCdsIndexBuilder;
pub(crate) use cds_option::PyCdsOption;
use cds_option::PyCdsOptionBuilder;
pub(crate) use cds_tranche::PyCdsTranche;
use cds_tranche::PyCdsTrancheBuilder;
pub(crate) use convertible::PyConvertibleBond;
use convertible::PyConvertibleBondBuilder;

/// Register the typed credit-derivative instruments on the instruments
/// submodule.
pub fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCreditDefaultSwap>()?;
    m.add_class::<PyCreditDefaultSwapBuilder>()?;
    m.add_class::<PyCdsIndex>()?;
    m.add_class::<PyCdsIndexBuilder>()?;
    m.add_class::<PyCdsOption>()?;
    m.add_class::<PyCdsOptionBuilder>()?;
    m.add_class::<PyCdsTranche>()?;
    m.add_class::<PyCdsTrancheBuilder>()?;
    m.add_class::<PyConvertibleBond>()?;
    m.add_class::<PyConvertibleBondBuilder>()?;
    cds_index::register(py, m)?;
    cds_tranche::register(py, m)?;
    convertible::register(py, m)?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
/// The four older instrument classes and their builders are listed by the
/// parent; `CdsOption` and the helper classes are listed here.
pub(crate) const EXPORTS: &[&str] = &[
    "CallPutSchedule",
    "CdsIndex",
    "CdsIndexBuilder",
    "CdsIndexConstituent",
    "CdsIndexParams",
    "CdsOption",
    "CdsOptionBuilder",
    "CdsTranche",
    "CdsTrancheBuilder",
    "CdsTrancheParams",
    "ConversionSpec",
    "ConvertibleBond",
    "ConvertibleBondBuilder",
    "CreditDefaultSwap",
    "CreditDefaultSwapBuilder",
];
