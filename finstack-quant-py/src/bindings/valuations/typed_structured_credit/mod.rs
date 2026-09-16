//! Typed structured-credit deal-modeling surface: `RepLine`, `AssetPool`,
//! `Tranche`, `TrancheStructure`, and the `StructuredCredit` instrument.
//!
//! Mirrors the `PyBond` pattern in `instruments.rs` for `StructuredCredit`
//! (the `Instrument`) and the `PyFixedLegSpec` pattern in `typed_legs.rs` for
//! the flat sub-models. Deep/optional sub-configs (`WaterfallRules`,
//! `CreditModelConfig`, `DealFees`, `MarketConditions`/`CreditFactors`,
//! loan-level `PoolAsset`, floating `TrancheCoupon`) stay JSON sub-fields per
//! the nested-spec rule; only the flat deal-modeling types above are typed.
//!
//! `DealType` and `TrancheSeniority` have no `#[serde(rename_all)]` in Rust —
//! their wire representation is PascalCase/acronym (`"abs"`, `"clo"`,
//! `"Senior"`, ...). This binding accepts exactly that wire casing at the
//! Python surface, routed through the generic `enum_from_str` helper like
//! every other typed instrument on this branch, so `to_json()` output round-
//! trips directly back into these constructors without any translation.

mod asset_pool;
mod diagnostics;
mod rep_line;
mod stochastic_result;
mod structured_credit;
mod tranche;
mod tranche_structure;

use pyo3::prelude::*;

pub(crate) use asset_pool::PyAssetPool;
pub(crate) use diagnostics::PySimulationDiagnostics;
pub(crate) use rep_line::PyRepLine;
pub(crate) use stochastic_result::PyStochasticPricingResult;
pub(crate) use structured_credit::{PyStructuredCredit, PyStructuredCreditBuilder};
pub(crate) use tranche::{PyTranche, PyTrancheBuilder};
pub(crate) use tranche_structure::PyTrancheStructure;

/// Register the typed structured-credit deal-modeling classes on the
/// instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRepLine>()?;
    m.add_class::<PyAssetPool>()?;
    m.add_class::<PyTranche>()?;
    m.add_class::<PyTrancheBuilder>()?;
    m.add_class::<PyTrancheStructure>()?;
    m.add_class::<PyStructuredCredit>()?;
    m.add_class::<PyStructuredCreditBuilder>()?;
    m.add_class::<PyStochasticPricingResult>()?;
    m.add_class::<PySimulationDiagnostics>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &["SimulationDiagnostics", "StochasticPricingResult"];
