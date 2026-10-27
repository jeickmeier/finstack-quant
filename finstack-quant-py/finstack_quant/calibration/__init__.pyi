"""Quote ingestion, market construction, and explicit model calibration.

Rust owns the calibration plan schema, its validation, and every solver. Python
exposes typed authoring classes over the same serde types, so an envelope can be
built in code (``RateQuote`` / ``CdsQuote`` / ``VolQuote`` / ``InflationQuote`` /
``XccyQuote`` / ``CdsTrancheQuote`` and the market datums -> ``CalibrationStep``
-> ``CalibrationPlan`` -> ``CalibrationEnvelope``) or handed to the entry points
as a ``dict`` or JSON string. The ``validate_*`` functions run the vol-surface
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
from finstack_quant.calibration._quotes import (
    CdsQuote as CdsQuote,
    CdsTrancheQuote as CdsTrancheQuote,
    FxSpotDatum as FxSpotDatum,
    InflationQuote as InflationQuote,
    PriceDatum as PriceDatum,
    RateQuote as RateQuote,
    VolQuote as VolQuote,
    XccyQuote as XccyQuote,
)
from finstack_quant.calibration._envelope import (
    CalibrationEnvelope as CalibrationEnvelope,
    CalibrationPlan as CalibrationPlan,
    CalibrationStep as CalibrationStep,
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

__all__ = [
    "CalibrationConfig",
    "CalibrationDiagnostics",
    "CalibrationEnvelope",
    "CalibrationEnvelopeError",
    "CalibrationPlan",
    "CalibrationReport",
    "CalibrationResultEnvelope",
    "CalibrationStep",
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

class CalibrationEnvelopeError(RuntimeError):
    """Raised when calibration ingestion, validation, or solving fails.

    Carries ``kind``, ``stage``, ``step_id``, ``solver_diagnostics`` (dict of
    fit-acceptance diagnostics for solver failures, else ``None``), ``details``
    (JSON string) and ``diagnostics`` (list of dicts with ``pointer``,
    ``message``, ``code`` and ``expected_version`` for strict-load failures).

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationEnvelopeError, dry_run
    >>> try:
    ...     dry_run("{}")
    ... except CalibrationEnvelopeError as exc:
    ...     isinstance(exc.diagnostics, list)
    True

    """

    kind: str
    stage: str
    step_id: str | None
    solver_diagnostics: dict[str, Any] | None
    details: str
    diagnostics: list[dict[str, Any]]

def validate_calibration(
    envelope: CalibrationEnvelope | dict[str, Any] | str,
) -> CalibrationEnvelope:
    """Validate a calibration envelope and return it in canonical typed form.

    Parameters
    ----------
    envelope : CalibrationEnvelope | dict | str
        Typed envelope, plan, dict, or JSON string using the schema marker
        ``finstack_quant.calibration/1``.

    Returns
    -------
    CalibrationEnvelope
        The validated envelope.

    Raises
    ------
    CalibrationEnvelopeError
        If strict loading or static validation rejects the envelope. Static
        validation is fail-fast (first error only); use :func:`dry_run` to list
        every static error. Strict-load failures carry ``diagnostics``.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, validate_calibration
    >>> validate_calibration(CalibrationEnvelope(CalibrationPlan([], id="smoke"))).plan.id
    'smoke'

    """

def dry_run(
    envelope: CalibrationEnvelope | dict[str, Any] | str,
) -> CalibrationValidationReport:
    """Validate an envelope statically without invoking the solver.

    Unlike :func:`validate_calibration`, semantic findings are returned in the
    report rather than raised, and every static error is collected in one pass.

    Parameters
    ----------
    envelope : CalibrationEnvelope | dict | str
        Typed envelope, plan, dict, or JSON string.

    Returns
    -------
    CalibrationValidationReport
        Every static error plus the step dependency graph.

    Raises
    ------
    CalibrationEnvelopeError
        Only when the input cannot be strictly loaded as an envelope (malformed
        JSON, wrong schema marker, unknown fields, resource limits). Those
        failures carry ``diagnostics`` with JSON pointers.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, dry_run
    >>> dry_run(CalibrationEnvelope(CalibrationPlan([], id="smoke"))).errors
    []

    """

def calibrate(
    envelope: CalibrationEnvelope | dict[str, Any] | str,
) -> CalibrationResultEnvelope:
    """Execute a calibration plan and return the calibrated market and reports.

    Parameters
    ----------
    envelope : CalibrationEnvelope | dict | str
        Typed envelope, dict,
        or JSON string using the schema marker ``finstack_quant.calibration/1``.

    Returns
    -------
    CalibrationResultEnvelope
        Calibrated market, plan-level report, per-step reports and residuals.

    Raises
    ------
    CalibrationEnvelopeError
        If ingestion, validation, context construction, target construction,
        solving, or final fit acceptance fails. Static validation is fail-fast;
        use :func:`dry_run` to list every static error.

    Examples:
    --------
    >>> from finstack_quant.calibration import CalibrationEnvelope, CalibrationPlan, calibrate
    >>> calibrate(CalibrationEnvelope(CalibrationPlan([], id="smoke"))).success
    True

    """

def calibrate_bermudan_lmm_base_vol(
    instrument: Any,
    market: MarketContext | str,
    as_of: Any,
) -> float:
    """Calibrate the explicit Bermudan LMM loading scale from the market surface.

    Parameters
    ----------
    instrument : Swaption | str
        Typed Bermudan swaption instrument or its canonical instrument JSON
        envelope.
    market : MarketContext | str
        Market carrying the required discount and swaption-volatility inputs.
    as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
        Valuation date used for tenor and expiry construction.

    Returns
    -------
    float
        Positive finite LMM base volatility, annualized decimal, to place in
        ``model_config.lmm_base_vol``.

    Raises
    ------
    ValueError
        If the instrument is not a Bermudan swaption or inputs are invalid.
    KeyError
        If a referenced curve or surface is missing from ``market``.
    RuntimeError
        If the Rebonato calibration fails.

    Examples:
    --------
    >>> from finstack_quant.calibration import calibrate_bermudan_lmm_base_vol
    >>> calibrate_bermudan_lmm_base_vol("{}", "{}", "2025-01-01")
    Traceback (most recent call last):
    finstack_quant.portfolio.ContractValidationError: ...

    """
