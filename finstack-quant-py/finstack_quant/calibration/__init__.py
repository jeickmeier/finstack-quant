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
CalibrationResultEnvelope = _calibration.CalibrationResultEnvelope
CalibrationStep = _calibration.CalibrationStep
DiscountCurveParams = _calibration.DiscountCurveParams
ForwardCurveParams = _calibration.ForwardCurveParams
HazardCurveParams = _calibration.HazardCurveParams
InflationCurveParams = _calibration.InflationCurveParams
VolSurfaceParams = _calibration.VolSurfaceParams
SwaptionVolParams = _calibration.SwaptionVolParams
BaseCorrelationParams = _calibration.BaseCorrelationParams
StudentTParams = _calibration.StudentTParams
HullWhiteStepParams = _calibration.HullWhiteStepParams
CapFloorHullWhiteStepParams = _calibration.CapFloorHullWhiteStepParams
SviSurfaceParams = _calibration.SviSurfaceParams
XccyBasisParams = _calibration.XccyBasisParams
ParametricCurveParams = _calibration.ParametricCurveParams
CalibrationValidationReport = _calibration.CalibrationValidationReport
CdsQuote = _calibration.CdsQuote
CdsTrancheQuote = _calibration.CdsTrancheQuote
FxSpotDatum = _calibration.FxSpotDatum
InflationQuote = _calibration.InflationQuote
PriceDatum = _calibration.PriceDatum
QuoteQuality = _calibration.QuoteQuality
RateBounds = _calibration.RateBounds
RateQuote = _calibration.RateQuote
SolverConfig = _calibration.SolverConfig
StepWithQuotes = _calibration.StepWithQuotes
ValidationConfig = _calibration.ValidationConfig
VolQuote = _calibration.VolQuote
XccyQuote = _calibration.XccyQuote
calibrate = _calibration.calibrate
calibrate_bermudan_lmm_base_vol = _calibration.calibrate_bermudan_lmm_base_vol
dry_run = _calibration.dry_run
validate_butterfly_spread = _calibration.validate_butterfly_spread
validate_calendar_spread = _calibration.validate_calendar_spread
validate_calibration = _calibration.validate_calibration
validate_surface = _calibration.validate_surface
validate_vol_bounds = _calibration.validate_vol_bounds
hull_white = _calibration.hull_white
schema = _calibration.schema

__all__ = [
    "BaseCorrelationParams",
    "CalibrationConfig",
    "CalibrationDiagnostics",
    "CalibrationEnvelope",
    "CalibrationEnvelopeError",
    "CalibrationPlan",
    "CalibrationReport",
    "CalibrationResultEnvelope",
    "CalibrationStep",
    "CalibrationValidationReport",
    "CapFloorHullWhiteStepParams",
    "CdsQuote",
    "CdsTrancheQuote",
    "DiscountCurveParams",
    "ForwardCurveParams",
    "FxSpotDatum",
    "HazardCurveParams",
    "HullWhiteStepParams",
    "InflationCurveParams",
    "InflationQuote",
    "ParametricCurveParams",
    "PriceDatum",
    "QuoteQuality",
    "RateBounds",
    "RateQuote",
    "SolverConfig",
    "StepWithQuotes",
    "StudentTParams",
    "SviSurfaceParams",
    "SwaptionVolParams",
    "ValidationConfig",
    "VolQuote",
    "VolSurfaceParams",
    "XccyBasisParams",
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
]
