"""Stub declarations mirroring ``finstack-quant-py/src/bindings/covenants/operations.rs``."""

from __future__ import annotations

import datetime
from finstack_quant.covenants._spec import (
    Covenant,
    CovenantConsequence,
    CovenantSpec,
    CovenantType,
    CovenantWaiver,
    SpringingCondition,
    ThresholdSchedule,
)
from finstack_quant.covenants._engine import (
    CovenantBreach,
    CovenantEngine,
    CovenantTemplate,
    covenant_template,
    reports_table,
)
from finstack_quant.covenants._report import CovenantReport
from finstack_quant.covenants._forecast import (
    CovenantForecast,
    CovenantForecastConfig,
    FutureBreach,
    breaches_table,
    forecast_breaches,
    forecast_covenant,
)
from finstack_quant.covenants import DateLike

def validate_covenant_spec_json(spec_json: str) -> str:
    """
    Validate and canonicalize a covenant specification JSON string.

    Parameters
    ----------
    spec_json : str
        JSON-encoded ``CovenantSpec`` (for example ``CovenantSpec.to_json()``).

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
    >>> from finstack_quant.covenants import CovenantTemplate, covenant_template, validate_covenant_spec_json
    >>> spec = covenant_template(CovenantTemplate.lbo_standard(5.0, 1.5, 1.2, 10_000_000.0))[0]
    >>> canonical = json.loads(validate_covenant_spec_json(spec.to_json()))
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
    >>> from finstack_quant.covenants import CovenantEngine, CovenantTemplate, covenant_template
    >>> from finstack_quant.covenants import validate_covenant_engine_json
    >>> engine = CovenantEngine.from_specs(covenant_template(CovenantTemplate.cov_lite(7.0, 4.5)))
    >>> list(json.loads(validate_covenant_engine_json(engine.to_json())))
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
    >>> from finstack_quant.covenants import CovenantEngine, CovenantTemplate, covenant_template, evaluate_engine
    >>> specs = covenant_template(CovenantTemplate.lbo_standard(5.0, 1.5, 1.2, 10_000_000.0))
    >>> engine = CovenantEngine.from_specs(specs).to_json()
    >>> metrics = {"debt_to_ebitda": 4.0, "interest_coverage": 3.0, "fixed_charge_coverage": 1.5, "capex": 5_000_000.0}
    >>> reports = evaluate_engine(engine, metrics, "2026-03-31")
    >>> reports["max_debt_ebitda"].passed, list(reports)[0]
    (True, 'max_debt_ebitda')
    """
