"""Quote ingestion, market construction, and explicit model calibration.

Rust owns the calibration plan schema, its validation, and every solver. Python
exposes typed authoring classes over the same serde types, so an envelope can be
built in code (``RateQuote`` / ``CdsQuote`` / ``VolQuote`` / ``InflationQuote`` /
``XccyQuote`` / ``CdsTrancheQuote`` and the market datums -> ``CalibrationStep``
-> ``CalibrationPlan`` -> ``CalibrationEnvelope``) or handed to the entry points
as a ``dict``. The ``validate_*`` functions run the vol-surface
no-arbitrage checks on a standalone ``VolSurface``.

Examples:
--------
>>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, calibrate
>>> result = calibrate(CalibrationEnvelope(CalibrationPlan([], id="smoke")))
>>> result.success
True
"""

from typing import Any
from finstack_quant.calibration import hull_white as hull_white
from finstack_quant.calibration import schema as schema
from finstack_quant.core.market_data import MarketContext
from finstack_quant.calibration._config import (
    CalibrationConfig as CalibrationConfig,
    RateBounds as RateBounds,
    SolverConfig as SolverConfig,
    ValidationConfig as ValidationConfig,
)
from finstack_quant.calibration._quotes.market import (
    CdsTrancheQuote as CdsTrancheQuote,
    FxSpotDatum as FxSpotDatum,
    InflationQuote as InflationQuote,
    PriceDatum as PriceDatum,
    XccyQuote as XccyQuote,
)
from finstack_quant.calibration._quotes.rates_vol import (
    CdsQuote as CdsQuote,
    RateQuote as RateQuote,
    VolQuote as VolQuote,
)
from finstack_quant.calibration._step_params import (
    DiscountCurveParams as DiscountCurveParams,
    ForwardCurveParams as ForwardCurveParams,
    HazardCurveParams as HazardCurveParams,
    InflationCurveParams as InflationCurveParams,
    VolSurfaceParams as VolSurfaceParams,
    SwaptionVolParams as SwaptionVolParams,
    BaseCorrelationParams as BaseCorrelationParams,
    StudentTParams as StudentTParams,
    HullWhiteStepParams as HullWhiteStepParams,
    CapFloorHullWhiteStepParams as CapFloorHullWhiteStepParams,
    SviSurfaceParams as SviSurfaceParams,
    XccyBasisParams as XccyBasisParams,
    ParametricCurveParams as ParametricCurveParams,
)
from finstack_quant.calibration._envelope import (
    CalibrationEnvelope as CalibrationEnvelope,
    CalibrationPlan as CalibrationPlan,
    CalibrationStep as CalibrationStep,
    StepWithQuotes as StepWithQuotes,
)
from finstack_quant.calibration._report import (
    CalibrationDiagnostics as CalibrationDiagnostics,
    CalibrationReport as CalibrationReport,
    CalibrationValidationReport as CalibrationValidationReport,
    QuoteQuality as QuoteQuality,
)
from finstack_quant.calibration._result import CalibrationResultEnvelope as CalibrationResultEnvelope
from finstack_quant.calibration._validation import (
    validate_butterfly_spread as validate_butterfly_spread,
    validate_calendar_spread as validate_calendar_spread,
    validate_surface as validate_surface,
    validate_vol_bounds as validate_vol_bounds,
)

from finstack_quant.calibration._execute import (
    CalibrationEnvelopeError as CalibrationEnvelopeError,
    validate_calibration as validate_calibration,
    dry_run as dry_run,
    calibrate as calibrate,
    calibrate_bermudan_lmm_base_vol as calibrate_bermudan_lmm_base_vol,
)

__all__ = [
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
]
