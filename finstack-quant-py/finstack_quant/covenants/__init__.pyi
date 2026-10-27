"""
Typed covenant definitions, engine evaluation, templates and forecasting.

Bindings for ``finstack-quant-covenants``. Define covenants with
:class:`CovenantType` / :class:`Covenant` / :class:`CovenantSpec` (or take a
standard package from :func:`lbo_standard`, :func:`cov_lite`,
:func:`real_estate`, :func:`project_finance`), collect them in a
:class:`CovenantEngine`, and evaluate against a ``dict`` of metric values
into typed :class:`CovenantReport` results. Step-down schedules
(:class:`ThresholdSchedule`), waivers (:class:`CovenantWaiver`) and springing
conditions (:class:`SpringingCondition`) are typed too. Forecast future
compliance from a date-indexed ``pandas.DataFrame`` of projected metrics with
:func:`forecast_covenant` / :func:`forecast_breaches`.

Conventions
-----------
Ratio metrics and thresholds are in turns (``4.5`` means 4.5x); rate-style
custom metrics such as debt yield or LTV are decimal fractions (``0.08`` is
8%); amount metrics (capex, liquidity, baskets) are bare numbers in the
caller's reporting currency. The engine tests whenever you call ``evaluate``
— ``test_frequency`` is descriptive metadata. Required metric observations must be finite (``ValueError`` otherwise);
a missing required metric raises ``KeyError``. Net-debt tests also require
positive earnings, selected by ``denominator_metric_id`` (default ``ebitda``).

The JSON surface shared with WASM is kept: the ``validate_covenant_*_json``
validators, ``evaluate_engine`` on an engine document, and the ``*_json``
template twins that return a JSON array of specs.

Examples
--------
>>> from finstack_quant.covenants import Covenant, CovenantEngine, CovenantSpec, CovenantType
>>> covenant = Covenant(CovenantType.max_debt_to_ebitda(4.5), "3M", "max_total_leverage")
>>> engine = CovenantEngine().add_spec(CovenantSpec(covenant, "debt_to_ebitda"))
>>> report = engine.evaluate({"debt_to_ebitda": 3.2}, "2025-03-31")["max_total_leverage"]
>>> report.passed, report.threshold
(True, 4.5)
"""

from __future__ import annotations

import datetime
from finstack_quant.covenants._spec import (
    Covenant as Covenant,
    CovenantConsequence as CovenantConsequence,
    CovenantSpec as CovenantSpec,
    CovenantType as CovenantType,
    CovenantWaiver as CovenantWaiver,
    SpringingCondition as SpringingCondition,
    ThresholdSchedule as ThresholdSchedule,
)
from finstack_quant.covenants._engine import (
    CovenantBreach as CovenantBreach,
    CovenantEngine as CovenantEngine,
    cov_lite as cov_lite,
    lbo_standard as lbo_standard,
    project_finance as project_finance,
    real_estate as real_estate,
    reports_to_dataframe as reports_to_dataframe,
)
from finstack_quant.covenants._report import CovenantReport as CovenantReport
from finstack_quant.covenants._forecast import (
    CovenantForecast as CovenantForecast,
    CovenantForecastConfig as CovenantForecastConfig,
    FutureBreach as FutureBreach,
    breaches_to_dataframe as breaches_to_dataframe,
    forecast_breaches as forecast_breaches,
    forecast_covenant as forecast_covenant,
)

__all__ = [
    "Covenant",
    "CovenantBreach",
    "CovenantConsequence",
    "CovenantEngine",
    "CovenantForecast",
    "CovenantForecastConfig",
    "CovenantReport",
    "CovenantSpec",
    "CovenantType",
    "CovenantWaiver",
    "FutureBreach",
    "SpringingCondition",
    "ThresholdSchedule",
    "breaches_to_dataframe",
    "cov_lite",
    "cov_lite_json",
    "evaluate_engine",
    "forecast_breaches",
    "forecast_covenant",
    "lbo_standard",
    "lbo_standard_json",
    "project_finance",
    "project_finance_json",
    "real_estate",
    "real_estate_json",
    "reports_to_dataframe",
    "validate_covenant_engine_json",
    "validate_covenant_report_json",
    "validate_covenant_spec_json",
]

DateLike = datetime.date | str

def validate_covenant_spec_json(spec_json: str) -> str:
    """
    Validate and canonicalize a covenant specification JSON string.

    Parameters
    ----------
    spec_json : str
        JSON-encoded ``CovenantSpec`` (for example ``CovenantSpec.to_json()``
        or one element of a ``*_json`` template).

    Returns
    -------
    str
        Canonical JSON after validation: object keys sorted recursively,
        arrays in semantic order, no insignificant whitespace.

    Raises
    ------
    ValueError
        If the spec fails schema or semantic validation (unknown fields,
        non-finite threshold, negative cure period, invalid schedule).

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import lbo_standard_json, validate_covenant_spec_json
    >>> spec = json.loads(lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0))[0]
    >>> canonical = json.loads(validate_covenant_spec_json(json.dumps(spec)))
    >>> list(canonical)
    ['covenant', 'metric_id']
    >>> list(canonical["covenant"])[:3]
    ['consequences', 'covenant_type', 'cure_period_days']
    """

def validate_covenant_report_json(report_json: str) -> str:
    """
    Validate and canonicalize a covenant evaluation report JSON string.

    Parameters
    ----------
    report_json : str
        JSON-encoded ``CovenantReport`` with pass/fail and headroom per covenant.

    Returns
    -------
    str
        Canonical JSON after validation, with object keys sorted recursively
        and the audit ``meta`` stamp filled in.

    Raises
    ------
    ValueError
        If the report JSON is malformed or fails validation.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import validate_covenant_report_json
    >>> report = {
    ...     "covenant_type": "Debt/EBITDA <= 5.00x",
    ...     "covenant_id": "max_debt_ebitda",
    ...     "passed": False,
    ...     "actual_value": 5.5,
    ...     "threshold": 5.0,
    ...     "details": "Exceeded",
    ...     "headroom": -0.1,
    ... }
    >>> canonical = json.loads(validate_covenant_report_json(json.dumps(report)))
    >>> canonical["passed"], list(canonical)[:2]
    (False, ['actual_value', 'covenant_id'])
    """

def validate_covenant_engine_json(engine_json: str) -> str:
    """
    Validate and canonicalize a covenant engine JSON string.

    Parameters
    ----------
    engine_json : str
        JSON-encoded engine document. Only ``specs`` is required;
        ``breach_history``, ``windows`` and ``waivers`` default to empty.
        Unknown fields are rejected.

    Returns
    -------
    str
        Canonical JSON after validation, with object keys sorted recursively
        and the defaulted arrays written out.

    Raises
    ------
    ValueError
        If the engine JSON is malformed or fails validation.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import cov_lite_json, validate_covenant_engine_json
    >>> engine = {"specs": json.loads(cov_lite_json(7.0, 4.5))}
    >>> list(json.loads(validate_covenant_engine_json(json.dumps(engine))))
    ['breach_history', 'specs', 'waivers', 'windows']
    """

def evaluate_engine(engine_json: str, metrics: dict[str, float] | str, as_of: DateLike) -> dict[str, CovenantReport]:
    """
    Evaluate a covenant engine JSON document against a metric mapping.

    JSON twin of :meth:`CovenantEngine.evaluate`; prefer the typed engine
    when you build the package in Python.

    Parameters
    ----------
    engine_json : str
        Serialized engine (``CovenantEngine.to_json()`` or a hand-written
        document; only ``specs`` is required).
    metrics : dict[str, float] | str
        Metric values keyed by metric id, or a JSON object string. Ratios in
        turns, amounts in the reporting currency. A dict is handed to Rust as
        numbers, exactly as :meth:`CovenantEngine.evaluate` does: a NaN or
        infinite value fails only when a covenant reads that metric. Every
        value of a JSON string must be a JSON number.
    as_of : datetime.date | str
        Evaluation date, a date-like object or an ISO 8601 string.

    Returns
    -------
    dict[str, CovenantReport]
        Typed report per covenant keyed by covenant label, in spec order.

    Raises
    ------
    KeyError
        If a required metric is missing from ``metrics``.
    ValueError
        If the engine document or a metric value is invalid, or the date
        string is not ISO 8601.
    TypeError
        If ``metrics`` is neither a dict nor a string, or ``as_of`` is
        neither a string nor date-like.

    Examples
    --------
    >>> from finstack_quant.covenants import evaluate_engine, lbo_standard_json
    >>> engine = '{"specs": ' + lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0) + "}"
    >>> metrics = {"debt_to_ebitda": 4.0, "interest_coverage": 3.0, "fixed_charge_coverage": 1.5, "capex": 5_000_000.0}
    >>> reports = evaluate_engine(engine, metrics, "2026-03-31")
    >>> reports["max_debt_ebitda"].passed, list(reports)[0]
    (True, 'max_debt_ebitda')
    """

def lbo_standard_json(
    initial_leverage: float,
    interest_coverage: float,
    fixed_charge_coverage: float,
    max_capex: float,
) -> str:
    """
    Standard leveraged-buyout covenant package as JSON (twin of :func:`lbo_standard`).

    Parameters
    ----------
    initial_leverage : float
        Maximum gross Debt/EBITDA in turns (``6.0`` for 6.0x).
    interest_coverage : float
        Minimum interest coverage ratio in turns.
    fixed_charge_coverage : float
        Minimum fixed-charge coverage ratio in turns.
    max_capex : float
        Maximum annual capex as a reporting-currency amount.

    Returns
    -------
    str
        JSON array of four ``CovenantSpec`` objects; wrap as
        ``{"specs": [...]}`` for an engine document.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import lbo_standard_json
    >>> len(json.loads(lbo_standard_json(5.0, 1.5, 1.2, 10_000_000.0)))
    4
    """

def cov_lite_json(max_leverage: float, max_senior_leverage: float) -> str:
    """
    Covenant-lite package as JSON (twin of :func:`cov_lite`).

    Parameters
    ----------
    max_leverage : float
        Maximum total Debt/EBITDA in turns.
    max_senior_leverage : float
        Maximum senior Debt/EBITDA in turns.

    Returns
    -------
    str
        JSON array of three incurrence ``CovenantSpec`` objects.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import cov_lite_json
    >>> len(json.loads(cov_lite_json(7.0, 4.5)))
    3
    """

def real_estate_json(min_dscr: float, min_debt_yield: float, max_ltv: float) -> str:
    """
    Real-estate covenant package as JSON (twin of :func:`real_estate`).

    Parameters
    ----------
    min_dscr : float
        Minimum debt-service coverage ratio in turns.
    min_debt_yield : float
        Minimum debt yield as a decimal fraction (``0.08`` for 8%); encoded
        as a custom minimum covenant on metric ``debt_yield``.
    max_ltv : float
        Maximum loan-to-value as a decimal fraction (``0.75`` for 75%);
        encoded as a custom maximum covenant on metric ``ltv``.

    Returns
    -------
    str
        JSON array of three ``CovenantSpec`` objects.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import real_estate_json
    >>> len(json.loads(real_estate_json(1.25, 0.08, 0.75)))
    3
    """

def project_finance_json(
    min_dscr: float,
    distribution_lockup_dscr: float,
    min_liquidity: float,
    max_net_leverage: float,
) -> str:
    """
    Project-finance covenant package as JSON (twin of :func:`project_finance`).

    Parameters
    ----------
    min_dscr : float
        Minimum DSCR in turns whose breach leads to default.
    distribution_lockup_dscr : float
        Higher DSCR in turns below which distributions are blocked.
    min_liquidity : float
        Minimum debt-service reserve as a reporting-currency amount.
    max_net_leverage : float
        Maximum net Debt/EBITDA in turns.

    Returns
    -------
    str
        JSON array of four ``CovenantSpec`` objects.

    Raises
    ------
    ValueError
        If any input is NaN, infinite or negative.

    Examples
    --------
    >>> import json
    >>> from finstack_quant.covenants import project_finance_json
    >>> len(json.loads(project_finance_json(1.30, 1.10, 5_000_000.0, 5.0)))
    4
    """
