//! Typed structured-credit deal-modeling surface: `RepLine`, `PoolAsset`,
//! `AssetPool`, `Tranche`, `TrancheStructure`, the `StructuredCredit`
//! instrument and its typed companions (`CallAssumption`, `CoverageRules`,
//! `HedgeSwap`, `Waterfall`) and results (`TrancheCashflows`,
//! `EquityMetrics`, `SimulationDiagnostics`, `StochasticPricingResult`).
//!
//! Mirrors the `PyBond` pattern in `instruments.rs` for `StructuredCredit`
//! (the `Instrument`) and the `PyFixedLegSpec` pattern in `typed_legs.rs` for
//! the flat sub-models. Deep sub-configs (`WaterfallRules`,
//! `CreditModelConfig`'s stochastic specs, `DealFees`,
//! `MarketConditions`/`CreditFactors`, `DelinquencyModel`,
//! `CardPortfolioSpec`, the CMBS `PoolAsset` sub-specs, floating
//! `TrancheCoupon`) stay dict / JSON sub-fields per the nested-spec rule;
//! every builder setter accepts a dict or JSON string for them.
//!
//! `DealType` and `TrancheSeniority` have no `#[serde(rename_all)]` in Rust —
//! their wire representation is PascalCase/acronym (`"abs"`, `"clo"`,
//! `"Senior"`, ...). This binding accepts exactly that wire casing at the
//! Python surface, routed through the generic `enum_from_str` helper like
//! every other typed instrument on this branch, so `to_json()` output round-
//! trips directly back into these constructors without any translation.

use pyo3::prelude::*;

/// `to_json` / `from_json` / `to_dict` / pickle for a serde-backed wrapper
/// whose Rust type carries strict field names.
macro_rules! sc_wire_methods {
    ($py_type:ident, $rust_type:ty, $name:literal) => {
        #[pymethods]
        impl $py_type {
            /// Deserialize from the JSON produced by ``to_json``.
            ///
            /// Parameters
            /// ----------
            /// json : str
            ///     JSON-encoded value (the shape ``to_json`` writes).
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If ``json`` is malformed or carries unknown fields.
            #[staticmethod]
            #[pyo3(text_signature = "(json)")]
            fn from_json(json: &str) -> PyResult<Self> {
                serde_json::from_str::<$rust_type>(json)
                    .map(|inner| Self { inner })
                    .map_err(|e| {
                        crate::errors::serde_json_to_py(e, concat!("invalid ", $name, " JSON"))
                    })
            }

            /// Serialize to the JSON shape ``from_json`` accepts.
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the value cannot be serialized.
            #[pyo3(text_signature = "($self)")]
            fn to_json(&self) -> PyResult<String> {
                serde_json::to_string(&self.inner).map_err(crate::errors::display_to_py)
            }

            /// Return every field as a plain ``dict`` (canonical serde shape).
            ///
            /// Raises
            /// ------
            /// ValueError
            ///     If the value cannot be serialized.
            #[pyo3(text_signature = "($self)")]
            fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
                crate::bindings::pandas_utils::serde_to_py(py, &self.inner)
            }

            /// Support ``pickle`` through the ``to_json`` / ``from_json`` round-trip.
            fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
                let from_json = py.get_type::<Self>().getattr("from_json")?;
                crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
            }
        }
    };
}

mod asset_pool;
mod call_assumption;
mod coverage_rules;
mod diagnostics;
mod equity_metrics;
mod hedge_swap;
mod pool_asset;
mod rep_line;
mod stochastic_result;
mod structured_credit;
mod tranche;
mod tranche_cashflows;
mod tranche_structure;
mod waterfall;

pub(crate) use asset_pool::PyAssetPool;
pub(crate) use call_assumption::PyCallAssumption;
pub(crate) use coverage_rules::PyCoverageRules;
pub(crate) use diagnostics::PySimulationDiagnostics;
pub(crate) use equity_metrics::PyEquityMetrics;
pub(crate) use hedge_swap::PyHedgeSwap;
pub(crate) use pool_asset::PyPoolAsset;
pub(crate) use rep_line::PyRepLine;
pub(crate) use stochastic_result::PyStochasticPricingResult;
pub(crate) use structured_credit::{PyStructuredCredit, PyStructuredCreditBuilder};
pub(crate) use tranche::{PyTranche, PyTrancheBuilder};
pub(crate) use tranche_cashflows::PyTrancheCashflows;
pub(crate) use tranche_structure::PyTrancheStructure;
pub(crate) use waterfall::PyWaterfall;

/// Register the typed structured-credit deal-modeling classes on the
/// instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRepLine>()?;
    m.add_class::<PyPoolAsset>()?;
    m.add_class::<PyAssetPool>()?;
    m.add_class::<PyTranche>()?;
    m.add_class::<PyTrancheBuilder>()?;
    m.add_class::<PyTrancheStructure>()?;
    m.add_class::<PyCallAssumption>()?;
    m.add_class::<PyCoverageRules>()?;
    m.add_class::<PyHedgeSwap>()?;
    m.add_class::<PyWaterfall>()?;
    m.add_class::<PyStructuredCredit>()?;
    m.add_class::<PyStructuredCreditBuilder>()?;
    m.add_class::<PyStochasticPricingResult>()?;
    m.add_class::<PySimulationDiagnostics>()?;
    m.add_class::<PyTrancheCashflows>()?;
    m.add_class::<PyEquityMetrics>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &[
    "CallAssumption",
    "CoverageRules",
    "EquityMetrics",
    "HedgeSwap",
    "PoolAsset",
    "SimulationDiagnostics",
    "StochasticPricingResult",
    "TrancheCashflows",
    "Waterfall",
];
