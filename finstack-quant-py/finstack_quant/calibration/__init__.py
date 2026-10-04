"""Quote ingestion, market construction, and explicit model calibration.

Envelopes can be authored with the typed classes (``RateQuote``, ``CdsQuote``,
``VolQuote``, ``InflationQuote``, ``XccyQuote``, ``CdsTrancheQuote``, the market
datums, ``CalibrationStep``, ``CalibrationPlan``, ``CalibrationEnvelope``) or
handed to :func:`calibrate` as a ``dict`` or JSON string. The ``validate_*``
functions run the vol-surface no-arbitrage checks on a standalone surface.

Examples:
--------
>>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, calibrate
>>> calibrate(CalibrationEnvelope(CalibrationPlan([], id="smoke"))).success
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
CdsTrancheQuote = _calibration.CdsTrancheQuote
CollateralEntry = _calibration.CollateralEntry
DividendScheduleDatum = _calibration.DividendScheduleDatum
FxSpotDatum = _calibration.FxSpotDatum
InflationQuote = _calibration.InflationQuote
PriceDatum = _calibration.PriceDatum
QuoteQuality = _calibration.QuoteQuality
RateBounds = _calibration.RateBounds
RateQuote = _calibration.RateQuote
SolverConfig = _calibration.SolverConfig
ValidationConfig = _calibration.ValidationConfig
VolQuote = _calibration.VolQuote
XccyQuote = _calibration.XccyQuote
calibrate = _calibration.calibrate
calibrate_bermudan_lmm_base_vol = _calibration.calibrate_bermudan_lmm_base_vol
dry_run = _calibration.dry_run
dry_run_json = _calibration.dry_run_json
validate_butterfly_call_convexity = _calibration.validate_butterfly_call_convexity
validate_butterfly_spread = _calibration.validate_butterfly_spread
validate_calendar_spread = _calibration.validate_calendar_spread
validate_calendar_spread_with_forwards = _calibration.validate_calendar_spread_with_forwards
validate_calibration = _calibration.validate_calibration
validate_calibration_json = _calibration.validate_calibration_json
validate_surface = _calibration.validate_surface
validate_surface_with_forwards = _calibration.validate_surface_with_forwards
validate_vol_bounds = _calibration.validate_vol_bounds
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
    "CdsTrancheQuote",
    "CollateralEntry",
    "DividendScheduleDatum",
    "FxSpotDatum",
    "InflationQuote",
    "PriceDatum",
    "QuoteQuality",
    "RateBounds",
    "RateQuote",
    "SolverConfig",
    "ValidationConfig",
    "VolQuote",
    "XccyQuote",
    "calibrate",
    "calibrate_bermudan_lmm_base_vol",
    "dry_run",
    "dry_run_json",
    "hull_white",
    "schema",
    "validate_butterfly_call_convexity",
    "validate_butterfly_spread",
    "validate_calendar_spread",
    "validate_calendar_spread_with_forwards",
    "validate_calibration",
    "validate_calibration_json",
    "validate_surface",
    "validate_surface_with_forwards",
    "validate_vol_bounds",
]
