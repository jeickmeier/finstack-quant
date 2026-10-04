"""
Cashflow schedule construction (typed and JSON), validation, and dated-flow extraction.

Root bindings for ``finstack-quant-cashflows``. Build schedules from a
``CashflowScheduleBuildSpec``, validate canonical payloads, extract dated flows,
and compute accrued interest.

Examples
--------
>>> import datetime
>>> from finstack_quant.cashflows.primitives import CFKind, CashFlow
>>> from finstack_quant.core.money import Money
>>> CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.FIXED, 0.0).amount.amount
100.0

"""

from __future__ import annotations

from typing import Any

import datetime

from finstack_quant.cashflows import accrual as accrual
from finstack_quant.cashflows.builder import CashFlowMeta, CashFlowSchedule
from finstack_quant.cashflows.primitives import CashFlow, CFKind
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.cashflows import aggregation as aggregation
from finstack_quant.cashflows import builder as builder
from finstack_quant.cashflows import fixings as fixings
from finstack_quant.cashflows import primitives as primitives
from finstack_quant.cashflows import schema as schema

__all__ = [
    "ScheduleBuildOpts",
    "accrual",
    "aggregation",
    "build_cashflow_schedule",
    "build_cashflow_schedule_json",
    "builder",
    "fixings",
    "dated_flows",
    "dated_flows_json",
    "primitives",
    "schedule_from_classified_flows",
    "schedule_from_dated_flows",
    "schema",
    "validate_cashflow_schedule_json",
]

class ScheduleBuildOpts:
    """
    Schedule-level inputs shared by :func:`schedule_from_dated_flows` and
    :func:`schedule_from_classified_flows`.

    Examples
    --------
    >>> from finstack_quant.cashflows import ScheduleBuildOpts
    >>> from finstack_quant.core.money import Money
    >>> ScheduleBuildOpts(notional_hint=Money(100.0, "USD")).notional_hint.amount
    100.0
    """

    def __init__(self, notional_hint: Money | None = None, meta: CashFlowMeta | None = None) -> None:
        """
        Construct build options.

        Parameters
        ----------
        notional_hint : Money, optional
            Notional stamped on the resulting schedule; when omitted a zero
            notional in the first flow's currency (USD if none) is used.
        meta : CashFlowMeta, optional
            Schedule-level metadata (default contractual, no calendars).

        Notes
        -----
        The constructor does not raise.

        Examples
        --------
        >>> from finstack_quant.cashflows import ScheduleBuildOpts
        >>> ScheduleBuildOpts().notional_hint is None
        True
        """
        ...

    @property
    def notional_hint(self) -> Money | None:
        """
        Notional stamped on the resulting schedule, if provided.

        Returns
        -------
        Money or None
            The hint, or ``None`` for currency-inferred zero notional.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    @property
    def meta(self) -> CashFlowMeta:
        """
        Schedule-level metadata.

        Returns
        -------
        CashFlowMeta
            Metadata stamped on built schedules.

        Notes
        -----
        This accessor does not raise.
        """
        ...

    def __repr__(self) -> str: ...

def build_cashflow_schedule(spec: dict[str, Any] | str, market: MarketContext | str | None = None) -> CashFlowSchedule:
    """
    Build a typed ``CashFlowSchedule`` from a build spec (typed twin of
    :func:`build_cashflow_schedule_json`).

    Parameters
    ----------
    spec : dict or str
        ``CashflowScheduleBuildSpec`` as a JSON string or an equivalent dict
        (``notional``, ``issue_date``, ``maturity``, ``coupon_program``,
        ``payment_program``, ``fees``, ``principal_events``,
        ``principal_exchange``). Each principal event requires economic ``date``
        and cash ``payment_date``; these dates may differ after payment adjustment.
    market : MarketContext or str, optional
        Market context (or its JSON) for floating-rate projection. Complete
        supplied observations need no forward curve. Explicit fallback covers
        missing curves or absent fixing series, never supplied historical gaps
        before a resolved curve's base date or genuine projection errors.

    Returns
    -------
    CashFlowSchedule
        Canonical typed schedule with ``to_dataframe()``.

    Raises
    ------
    ValueError
        If the spec is malformed or the schedule fails validation, including
        unsupported roll-grid/ICMA anchors, negative lags, supplied historical
        fixing gaps, or curve/date/day-count/arithmetic failures.
    KeyError
        If required market data is unavailable and no eligible fallback resolves it.

    Examples
    --------
    >>> from finstack_quant.cashflows import build_cashflow_schedule
    >>> spec = {
    ...     "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
    ...     "issue_date": "2025-01-15",
    ...     "maturity": "2026-01-15",
    ...     "coupon_program": [
    ...         {
    ...             "kind": "fixed",
    ...             "spec": {
    ...                 "rate": "0.05",
    ...                 "frequency": {"count": 6, "unit": "months"},
    ...                 "day_count": "30_360",
    ...                 "calendar_id": "weekends_only",
    ...             },
    ...         }
    ...     ],
    ... }
    >>> build_cashflow_schedule(spec).get_flows()[0].kind.name
    'notional'
    """
    ...

def dated_flows(schedule: CashFlowSchedule | str) -> list[tuple[datetime.date, Money]]:
    """
    Settlement cash entries of a schedule (typed twin of :func:`dated_flows_json`).

    Parameters
    ----------
    schedule : CashFlowSchedule or str
        Typed schedule or its canonical JSON.

    Returns
    -------
    list[tuple[datetime.date, Money]]
        Cash-settling rows in schedule order; PIK capitalizations and
        default write-downs are omitted, as are zero-cash principal markers.
        Native currencies are retained without conversion or netting.

    Raises
    ------
    ValueError
        If ``schedule`` is malformed or its amounts, accrual metadata, row
        currencies, or dates are invalid. Single-currency principal paths
        must reconcile with the opening notional. Composite principal paths
        in multiple currencies receive structural validation without scalar
        balance reconciliation.
    TypeError
        If ``schedule`` is neither a ``CashFlowSchedule`` nor a string.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.cashflows import dated_flows, schedule_from_dated_flows
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.money import Money
    >>> schedule = schedule_from_dated_flows(
    ...     [(datetime.date(2025, 6, 15), Money(100.0, "USD"))], "fixed", DayCount.ACT_360
    ... )
    >>> dated_flows(schedule)[0][1].amount
    100.0
    """
    ...

def schedule_from_dated_flows(
    flows: list[tuple[datetime.date, Money]],
    kind: CFKind | str,
    day_count: DayCount,
    opts: ScheduleBuildOpts | None = None,
) -> CashFlowSchedule:
    """
    Build a ``CashFlowSchedule`` from dated flows sharing one classification.

    Parameters
    ----------
    flows : list[tuple[datetime.date, Money]]
        Dated amounts in any order.
    kind : CFKind or str
        Classification stamped on every row (e.g. ``"fixed"``).
    day_count : DayCount
        Representative day-count convention.
    opts : ScheduleBuildOpts, optional
        Notional hint and metadata.

    Returns
    -------
    CashFlowSchedule
        Canonical schedule with zero accrual factors and no rates.

    Raises
    ------
    ValueError
        If ``kind`` is not a known label or a date cannot be parsed.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.cashflows import schedule_from_dated_flows
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.money import Money
    >>> schedule_from_dated_flows(
    ...     [(datetime.date(2025, 6, 15), Money(100.0, "USD"))], "fixed", DayCount.THIRTY_360
    ... ).get_flows()[0].amount.amount
    100.0
    """
    ...

def schedule_from_classified_flows(
    flows: list[CashFlow],
    day_count: DayCount,
    opts: ScheduleBuildOpts | None = None,
) -> CashFlowSchedule:
    """
    Build a ``CashFlowSchedule`` from pre-classified ``CashFlow`` rows.

    Parameters
    ----------
    flows : list[CashFlow]
        Classified rows in any order; kinds preserved.
    day_count : DayCount
        Representative day-count convention.
    opts : ScheduleBuildOpts, optional
        Notional hint and metadata.

    Returns
    -------
    CashFlowSchedule
        Canonical schedule holding the sorted rows.

    Notes
    -----
    This function does not raise.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.cashflows import schedule_from_classified_flows
    >>> from finstack_quant.cashflows.primitives import CashFlow, CFKind
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.money import Money
    >>> flow = CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.PIK, 0.0)
    >>> schedule_from_classified_flows([flow], DayCount.ACT_360).get_flows()[0].kind.name
    'pik'
    """
    ...

def build_cashflow_schedule_json(spec_json: str, market_json: str | None = None) -> str:
    """
    Build a cashflow schedule from a JSON spec and return canonical schedule JSON.

    Parameters
    ----------
    spec_json : str
        JSON-encoded ``CashflowScheduleBuildSpec`` with canonical
        ``coupon_program`` and ``payment_program`` instructions, principal,
        fees, and schedule rules.
    market_json : str, optional
        JSON-encoded ``MarketContext`` for floating-rate index lookups. Omit
        when the schedule uses fixed coupons only. Complete supplied observations
        need no forward curve. Explicit fallback covers missing curves or absent
        fixing series, never supplied historical gaps before a resolved curve's
        base date or genuine projection errors.

    Returns
    -------
    str
        Canonical JSON-encoded ``CashFlowSchedule``.

    Raises
    ------
    ValueError
        If ``spec_json`` (or ``market_json`` when supplied) fails schema or
        semantic validation, including unsupported roll-grid/ICMA anchors,
        negative lags, supplied historical fixing gaps, or
        curve/date/day-count/arithmetic failures.
    KeyError
        If required market data is unavailable and no eligible fallback resolves it.

    Examples
    --------
    >>> import json
    >>> spec = {
    ...     "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
    ...     "issue_date": "2024-08-31",
    ...     "maturity": "2025-08-31",
    ...     "coupon_program": [
    ...         {
    ...             "kind": "fixed",
    ...             "spec": {
    ...                 "coupon_type": "cash",
    ...                 "rate": "0.06",
    ...                 "frequency": {"count": 12, "unit": "months"},
    ...                 "day_count": "30_360",
    ...                 "business_day_convention": "following",
    ...                 "calendar_id": "weekends_only",
    ...                 "stub": "none",
    ...                 "end_of_month": False,
    ...                 "payment_lag_days": 0,
    ...             },
    ...         }
    ...     ],
    ... }
    >>> from finstack_quant.cashflows import build_cashflow_schedule_json
    >>> schedule_json = build_cashflow_schedule_json(json.dumps(spec))
    >>> json.loads(schedule_json)["meta"]["issue_date"]
    '2024-08-31'

    """

def validate_cashflow_schedule_json(schedule_json: str) -> str:
    """
    Validate and canonicalize a ``CashFlowSchedule`` JSON payload.

    Parameters
    ----------
    schedule_json : str
        JSON-encoded ``CashFlowSchedule``.

    Returns
    -------
    str
        Canonical re-serialized schedule JSON.

    Raises
    ------
    ValueError
        If ``schedule_json`` is malformed or fails validation.

    Examples
    --------
    >>> import datetime
    >>> from decimal import Decimal
    >>> from finstack_quant.cashflows.builder import CashFlowSchedule, FixedCouponSpec, ScheduleParams
    >>> from finstack_quant.core.money import Money
    >>> schedule = (
    ...     CashFlowSchedule
    ...     .builder()
    ...     .principal(Money(1_000_000.0, "USD"), datetime.date(2025, 1, 15), datetime.date(2026, 1, 15))
    ...     .fixed_cf(FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.semiannual_30360()))
    ...     .build()
    ... )
    >>> import json
    >>> from finstack_quant.cashflows import validate_cashflow_schedule_json
    >>> json.loads(validate_cashflow_schedule_json(schedule.to_json()))["meta"]["issue_date"]
    '2025-01-15'

    """

def dated_flows_json(schedule_json: str) -> str:
    """
    Extract settlement-dated cashflows from a schedule as a compact JSON array.

    Parameters
    ----------
    schedule_json : str
        JSON-encoded ``CashFlowSchedule``.

    Returns
    -------
    str
        JSON array of settlement cash entries. ``PIK`` and
        ``DefaultedNotional`` state rows and zero-cash principal markers are
        omitted. Native currencies are retained without conversion or netting;
        parse the full schedule JSON when flow classification is required.

    Raises
    ------
    ValueError
        If the schedule JSON, amounts, accrual metadata, row currencies, or
        dates are invalid, or a single-currency principal path fails balance
        reconciliation. Composite principal paths in multiple currencies
        receive structural validation without scalar balance reconciliation.

    Examples
    --------
    >>> import datetime
    >>> from decimal import Decimal
    >>> from finstack_quant.cashflows.builder import CashFlowSchedule, FixedCouponSpec, ScheduleParams
    >>> from finstack_quant.core.money import Money
    >>> schedule = (
    ...     CashFlowSchedule
    ...     .builder()
    ...     .principal(Money(1_000_000.0, "USD"), datetime.date(2025, 1, 15), datetime.date(2026, 1, 15))
    ...     .fixed_cf(FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.semiannual_30360()))
    ...     .build()
    ... )
    >>> import json
    >>> from finstack_quant.cashflows import dated_flows_json
    >>> len(json.loads(dated_flows_json(schedule.to_json()))) == len(schedule.get_flows())
    True

    """
