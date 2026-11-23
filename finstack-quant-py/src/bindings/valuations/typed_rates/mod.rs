//! Typed rates instruments: `InterestRateSwap`, `Swaption` and `CapFloor`.
//!
//! Mirrors the `PyBond` pattern in `instruments.rs`: frozen wrappers with one
//! getter per public Rust field, the serde surface (`to_json` / `from_json` /
//! pickle / `to_dict`), `price` / `metric` through the canonical pricer, and
//! consuming builders that wrap the Rust `FinancialBuilder` output one setter
//! for one setter.

mod cap_floor;
mod swap;
mod swaption;

pub use cap_floor::{PyCapFloor, PyCapFloorBuilder};
use pyo3::prelude::*;
pub use swap::{PyInterestRateSwap, PyInterestRateSwapBuilder};
pub use swaption::{PySwaption, PySwaptionBuilder};

/// Register the typed rates instruments on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyInterestRateSwap>()?;
    m.add_class::<PyInterestRateSwapBuilder>()?;
    m.add_class::<PySwaption>()?;
    m.add_class::<PySwaptionBuilder>()?;
    m.add_class::<PyCapFloor>()?;
    m.add_class::<PyCapFloorBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &[
    "CapFloor",
    "CapFloorBuilder",
    "InterestRateSwap",
    "InterestRateSwapBuilder",
    "Swaption",
    "SwaptionBuilder",
];
