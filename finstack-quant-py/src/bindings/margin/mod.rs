//! Python bindings for the `finstack-quant-margin` crate.
//!
//! Exposes variation/initial margin calculators, CSA specifications,
//! collateral types, XVA configuration/results, and margin metrics.

mod calculators;
pub(crate) use calculators::PyImResult;
mod frame;
pub(crate) mod im;
mod im_curvature;
mod metrics;
mod regulatory;
pub(crate) use regulatory::PyFrtbSensitivities;
mod schema;
pub(crate) mod types;
mod xva;
pub(crate) use xva::PyExposureProfile;

use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the `margin` submodule on the parent module.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "margin")?;
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

    schema::register(py, &m)?;

    let all = PyList::new(
        py,
        [
            "CONSTANTS",
            "ClearingStatus",
            "CollateralAssetClass",
            "CsaSpec",
            "CsaSpecBuilder",
            "DrcPosition",
            "OtcMarginSpec",
            "SimmCreditClassification",
            "EadResult",
            "EligibleCollateralSchedule",
            "ExcessCollateral",
            "ExposureDiagnostics",
            "ExposureProfile",
            "FrtbSbaEngine",
            "FrtbSbaResult",
            "FrtbSensitivities",
            "FundingConfig",
            "Haircut01",
            "HaircutImCalculator",
            "ImCollateralResult",
            "ImDecayProfile",
            "ImMethodology",
            "ImProfile",
            "ImProjectionSpec",
            "MarginFundingSpread",
            "ImResult",
            "MarginCallType",
            "MarginFundingCost",
            "MarginTenor",
            "MarginUtilization",
            "MvaResult",
            "NettingSetId",
            "SaCcrEngine",
            "SaCcrNettingSetConfig",
            "SaCcrTrade",
            "ScheduleImCalculator",
            "SimmCalculator",
            "SimmCurvatureSensitivity",
            "SimmSensitivities",
            "VmCalculator",
            "VmResult",
            "XvaResult",
            "compute_bilateral_xva",
            "compute_mva",
            "im_profile_from_simm",
            "schema",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;

    Ok(())
}
