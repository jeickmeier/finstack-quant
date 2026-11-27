//! Python bindings for FRTB SBA and SA-CCR regulatory capital frameworks.
//!
//! Callers build sensitivity or trade containers with the ``add_*`` methods
//! (or bulk-load a long-format frame), then run an engine to get a typed
//! result with a per-component breakdown.

mod frtb;
mod saccr;

pub(crate) use frtb::PyDrcPosition;
pub use frtb::{PyFrtbSbaEngine, PyFrtbSbaResult, PyFrtbSensitivities};
use pyo3::prelude::*;
pub use saccr::{PyEadResult, PySaCcrEngine, PySaCcrNettingSetConfig, PySaCcrTrade};

/// Register FRTB / SA-CCR classes on the margin module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFrtbSensitivities>()?;
    m.add_class::<PyDrcPosition>()?;
    m.add_class::<PyFrtbSbaEngine>()?;
    m.add_class::<PyFrtbSbaResult>()?;
    m.add_class::<PyEadResult>()?;
    m.add_class::<PySaCcrTrade>()?;
    m.add_class::<PySaCcrNettingSetConfig>()?;
    m.add_class::<PySaCcrEngine>()?;
    Ok(())
}
