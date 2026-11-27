"""
Typed covenant definitions, engine evaluation, templates and forecasting.

Bindings for ``finstack-quant-covenants``. Define covenants with
:class:`CovenantType` / :class:`Covenant` / :class:`CovenantSpec` (or take a
standard package with :func:`covenant_template` from a
:class:`CovenantTemplate`), collect them in a
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

The ``validate_covenant_*_json`` validators take a document as JSON text. To
evaluate an engine held as text, load it with ``CovenantEngine.from_json``.

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
    CovenantTemplate as CovenantTemplate,
    covenant_template as covenant_template,
    reports_table as reports_table,
)
from finstack_quant.covenants._report import CovenantReport as CovenantReport
from finstack_quant.covenants._forecast import (
    CovenantForecast as CovenantForecast,
    CovenantForecastConfig as CovenantForecastConfig,
    FutureBreach as FutureBreach,
    breaches_table as breaches_table,
    forecast_breaches as forecast_breaches,
    forecast_covenant as forecast_covenant,
)

from finstack_quant.covenants._operations import (
    validate_covenant_spec_json as validate_covenant_spec_json,
    validate_covenant_report_json as validate_covenant_report_json,
    validate_covenant_engine_json as validate_covenant_engine_json,
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
    "CovenantTemplate",
    "CovenantType",
    "CovenantWaiver",
    "FutureBreach",
    "SpringingCondition",
    "ThresholdSchedule",
    "breaches_table",
    "covenant_template",
    "forecast_breaches",
    "forecast_covenant",
    "reports_table",
    "validate_covenant_engine_json",
    "validate_covenant_report_json",
    "validate_covenant_spec_json",
]

DateLike = datetime.date | str
