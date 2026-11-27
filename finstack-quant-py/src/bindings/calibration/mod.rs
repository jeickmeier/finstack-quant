//! Python bindings for the calibration engine.
//!
//! Wraps `finstack_quant_calibration::api::engine::calibrate` behind a typed
//! envelope-in / rich-result-out API. Envelopes can be authored as typed
//! objects (`CalibrationEnvelope`, `CalibrationPlan`, `CalibrationStep`,
//! quote classes) or as dicts; JSON text is loaded with
//! `CalibrationEnvelope.from_json`.

mod config;
mod envelope;
mod hull_white;
mod quotes;
mod report;
mod result;
mod schema;
mod step_params;
mod validation;

use config::{PyCalibrationConfig, PyRateBounds, PySolverConfig, PyValidationConfig};
use envelope::{PyCalibrationEnvelope, PyCalibrationPlan, PyCalibrationStep, PyStepWithQuotes};
use quotes::{
    PyCdsQuote, PyCdsTrancheQuote, PyFxSpotDatum, PyInflationQuote, PyPriceDatum, PyRateQuote,
    PyVolQuote, PyXccyQuote,
};
use report::{
    PyCalibrationDiagnostics, PyCalibrationReport, PyCalibrationValidationReport, PyQuoteQuality,
};
use result::PyCalibrationResult;
mod execute;

pub(crate) use execute::{CalibrationEnvelopeError, parse_envelope};
use execute::{calibrate, calibrate_bermudan_lmm_base_vol, dry_run, validate_calibration};
use pyo3::prelude::*;
use pyo3::types::PyList;

/// Register the root-level calibration submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "calibration")?;
    m.add_class::<PyCalibrationConfig>()?;
    m.add_class::<PyCalibrationDiagnostics>()?;
    m.add_class::<PyCalibrationEnvelope>()?;
    m.add_class::<PyCalibrationPlan>()?;
    m.add_class::<PyCalibrationReport>()?;
    m.add_class::<PyCalibrationResult>()?;
    m.add_class::<PyCalibrationStep>()?;
    step_params::register(&m)?;
    m.add_class::<PyCalibrationValidationReport>()?;
    m.add_class::<PyCdsQuote>()?;
    m.add_class::<PyCdsTrancheQuote>()?;
    m.add_class::<PyFxSpotDatum>()?;
    m.add_class::<PyInflationQuote>()?;
    m.add_class::<PyPriceDatum>()?;
    m.add_class::<PyQuoteQuality>()?;
    m.add_class::<PyRateBounds>()?;
    m.add_class::<PyRateQuote>()?;
    m.add_class::<PySolverConfig>()?;
    m.add_class::<PyStepWithQuotes>()?;
    m.add_class::<PyValidationConfig>()?;
    m.add_class::<PyVolQuote>()?;
    m.add_class::<PyXccyQuote>()?;
    m.add(
        "CalibrationEnvelopeError",
        py.get_type::<CalibrationEnvelopeError>(),
    )?;
    m.add_function(pyo3::wrap_pyfunction!(calibrate, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(calibrate_bermudan_lmm_base_vol, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(dry_run, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validate_calibration, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(
        validation::validate_butterfly_spread,
        &m
    )?)?;
    m.add_function(pyo3::wrap_pyfunction!(
        validation::validate_calendar_spread,
        &m
    )?)?;
    m.add_function(pyo3::wrap_pyfunction!(validation::validate_surface, &m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validation::validate_vol_bounds, &m)?)?;
    hull_white::register(py, &m)?;
    schema::register(py, &m)?;
    m.setattr(
        "__all__",
        PyList::new(
            py,
            [
                "CalibrationConfig",
                "CalibrationDiagnostics",
                "CalibrationEnvelope",
                "CalibrationEnvelopeError",
                "CalibrationPlan",
                "CalibrationReport",
                "CalibrationResultEnvelope",
                "CalibrationStep",
                "DiscountCurveParams",
                "ForwardCurveParams",
                "HazardCurveParams",
                "InflationCurveParams",
                "VolSurfaceParams",
                "SwaptionVolParams",
                "BaseCorrelationParams",
                "StudentTParams",
                "HullWhiteStepParams",
                "CapFloorHullWhiteStepParams",
                "SviSurfaceParams",
                "XccyBasisParams",
                "ParametricCurveParams",
                "CalibrationValidationReport",
                "CdsQuote",
                "CdsTrancheQuote",
                "FxSpotDatum",
                "InflationQuote",
                "PriceDatum",
                "QuoteQuality",
                "RateBounds",
                "RateQuote",
                "SolverConfig",
                "StepWithQuotes",
                "ValidationConfig",
                "VolQuote",
                "XccyQuote",
                "calibrate",
                "calibrate_bermudan_lmm_base_vol",
                "dry_run",
                "hull_white",
                "schema",
                "validate_butterfly_spread",
                "validate_calendar_spread",
                "validate_calibration",
                "validate_surface",
                "validate_vol_bounds",
            ],
        )?,
    )?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Python,
    )?;
    Ok(())
}
