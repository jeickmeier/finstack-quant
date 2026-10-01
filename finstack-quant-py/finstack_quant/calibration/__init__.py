"""Quote ingestion, market construction, and explicit model calibration.

Envelopes can be authored with the typed classes (``RateQuote``, ``CdsQuote``,
``VolQuote``, ``CalibrationStep``, ``CalibrationPlan``, ``CalibrationEnvelope``)
or handed to :func:`calibrate` as a ``dict`` or JSON string.

Examples:
--------
>>> from finstack_quant.calibration import CalibrationPlan, calibrate
>>> calibrate(CalibrationPlan([], id="smoke")).success
True
"""

from finstack_quant.finstack_quant import calibration as _calibration

CalibrationConfig = _calibration.CalibrationConfig
CalibrationDiagnostics = _calibration.CalibrationDiagnostics
CalibrationEnvelope = _calibration.CalibrationEnvelope
CalibrationEnvelopeError = _calibration.CalibrationEnvelopeError
CalibrationPlan = _calibration.CalibrationPlan
CalibrationReport = _calibration.CalibrationReport
CalibrationResult = _calibration.CalibrationResult
CalibrationStep = _calibration.CalibrationStep
CalibrationValidationReport = _calibration.CalibrationValidationReport
CdsQuote = _calibration.CdsQuote
QuoteQuality = _calibration.QuoteQuality
RateBounds = _calibration.RateBounds
RateQuote = _calibration.RateQuote
SolverConfig = _calibration.SolverConfig
ValidationConfig = _calibration.ValidationConfig
VolQuote = _calibration.VolQuote
calibrate = _calibration.calibrate
calibrate_bermudan_lmm_base_vol = _calibration.calibrate_bermudan_lmm_base_vol
dry_run = _calibration.dry_run
dry_run_json = _calibration.dry_run_json
validate_calibration = _calibration.validate_calibration
validate_calibration_json = _calibration.validate_calibration_json
hull_white = _calibration.hull_white
schema = _calibration.schema

__all__ = [
    "CalibrationConfig",
    "CalibrationDiagnostics",
    "CalibrationEnvelope",
    "CalibrationEnvelopeError",
    "CalibrationPlan",
    "CalibrationReport",
    "CalibrationResult",
    "CalibrationStep",
    "CalibrationValidationReport",
    "CdsQuote",
    "QuoteQuality",
    "RateBounds",
    "RateQuote",
    "SolverConfig",
    "ValidationConfig",
    "VolQuote",
    "calibrate",
    "calibrate_bermudan_lmm_base_vol",
    "dry_run",
    "dry_run_json",
    "hull_white",
    "schema",
    "validate_calibration",
    "validate_calibration_json",
]
