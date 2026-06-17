//! Python bindings for the `finstack-quant-margin` crate.
//!
//! Exposes variation/initial margin calculators, CSA specifications,
//! collateral types, XVA configuration/results, and margin metrics.

mod calculators;
mod im;
mod metrics;
mod regulatory;
mod types;
mod xva;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `margin` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "margin")?;
    m.setattr(
        "__doc__",
        "Margin and collateral: VM/IM calculators, CSA specifications, XVA, metrics.",
    )?;

    types::register(py, &m)?;
    calculators::register(py, &m)?;
    im::register(py, &m)?;
    xva::register(py, &m)?;
    metrics::register(py, &m)?;
    regulatory::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            // Types & enums
            "ImMethodology",
            "MarginTenor",
            "MarginCallType",
            "ClearingStatus",
            "CollateralAssetClass",
            "NettingSetId",
            "CsaSpec",
            "EligibleCollateralSchedule",
            "CONSTANTS",
            // Calculators
            "VmResult",
            "VmCalculator",
            "ImResult",
            "SimmSensitivities",
            "SimmCalculator",
            "ScheduleImCalculator",
            "HaircutImCalculator",
            // XVA
            "FundingConfig",
            "XvaConfig",
            "ExposureDiagnostics",
            "ExposureProfile",
            "XvaResult",
            "CsaTerms",
            "XvaNettingSet",
            // Metrics
            "MarginUtilization",
            "ExcessCollateral",
            "MarginFundingCost",
            "Haircut01",
            // Regulatory (FRTB SBA + SA-CCR)
            "FrtbSensitivities",
            "FrtbSbaEngine",
            "SaCcrTrade",
            "SaCcrNettingSetConfig",
            "SaCcrEngine",
            "frtb_sba_charge",
            "saccr_ead",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "margin",
        crate::bindings::module_utils::ROOT_PACKAGE,
        crate::bindings::module_utils::ParentNameSource::Name,
    )?;

    Ok(())
}
