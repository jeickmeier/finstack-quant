//! Input extraction shared by the scenario entry points.
//!
//! Instruments arrive as typed instrument wrappers or envelope dicts. The Rust
//! scenario engine attaches its own recalibration provider, so the bindings
//! only thread the configuration.

use finstack_quant_valuations::instruments::Instrument;
use pyo3::prelude::*;

use crate::bindings::extract::extract_instrument;
use crate::errors::core_to_py;

/// Extract an optional instrument collection from typed instrument wrappers
/// or canonical envelope dicts.
pub(crate) fn extract_instruments(
    objs: Option<Vec<Bound<'_, PyAny>>>,
) -> PyResult<Option<Vec<Box<dyn Instrument>>>> {
    let Some(objs) = objs else {
        return Ok(None);
    };
    objs.iter()
        .map(|obj| {
            extract_instrument(obj)?
                .into_parsed(None, None)
                .map(|parsed| parsed.into_boxed())
                .map_err(core_to_py)
        })
        .collect::<PyResult<Vec<_>>>()
        .map(Some)
}
