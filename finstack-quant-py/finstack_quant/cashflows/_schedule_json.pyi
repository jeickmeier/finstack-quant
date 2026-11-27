"""Stub declarations mirroring ``finstack-quant-py/src/bindings/cashflows/schedule_json.rs``."""

from __future__ import annotations

from typing import Any
import datetime
from finstack_quant.cashflows.builder import CashFlowMeta, CashFlowSchedule
from finstack_quant.core.cashflow import CashFlow, CFKind
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money

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

def build_cashflow_schedule(
    spec: dict[str, Any],
    market: MarketContext | None = None,
    as_of: datetime.date | str | None = None,
) -> CashFlowSchedule:
    """
    Build a typed ``CashFlowSchedule`` from a build spec.

    A holder of JSON text loads the spec with ``json.loads`` and the market
    with ``MarketContext.from_json``; ``to_json()`` on the result gives the
    canonical schedule text.

    Parameters
    ----------
    spec : dict
        ``CashflowScheduleBuildSpec`` as a dict
        (``notional``, ``issue_date``, ``maturity``, ``coupon_program``,
        ``payment_program``, ``fees``, ``principal_events``,
        ``principal_exchange``). Each principal event requires economic ``date``
        and cash ``payment_date``; these dates may differ after payment adjustment.
    market : MarketContext, optional
        Market context for floating-rate projection. Every
        floating coupon needs its forward curve, even when supplied
        observations cover it, unless an explicit fallback applies. Explicit
        fallback covers any failed coupon projection, never supplied
        historical gaps before a resolved curve's base date.
    as_of : datetime.date or str, optional
        Valuation cutoff date overriding ``spec["as_of"]``; resets before
        the effective cutoff require historical fixings.

    Returns
    -------
    CashFlowSchedule
        Canonical typed schedule supporting ``to_arrow()``, ``to_pandas()``,
        ``to_polars()``, and ``to_pyarrow()``.

    Raises
    ------
    ValueError
        If the spec or ``as_of`` is malformed or the schedule fails validation,
        including unsupported roll-grid/ICMA anchors, negative lags, supplied
        historical fixing gaps, or curve/date/day-count/arithmetic failures.
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
    ...                 "business_day_convention": "modified_following",
    ...             },
    ...         }
    ...     ],
    ... }
    >>> build_cashflow_schedule(spec).get_flows()[0].kind.name
    'notional'
    """
    ...

def dated_flows(schedule: CashFlowSchedule) -> list[tuple[datetime.date, Money]]:
    """
    Settlement cash entries of a schedule.

    Parameters
    ----------
    schedule : CashFlowSchedule
        Canonical cashflow schedule.

    Returns
    -------
    list[tuple[datetime.date, Money]]
        Cash-settling rows in schedule order; PIK capitalizations and
        default write-downs are omitted, as are zero-cash principal markers.
        Native currencies are retained without conversion or netting.

    Raises
    ------
    ValueError
        If ``schedule`` amounts, accrual metadata, row currencies, or dates are
        invalid. Single-currency principal paths must reconcile with the
        opening notional. Composite principal paths in multiple currencies
        receive structural validation without scalar balance reconciliation.

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
        Canonical schedule holding the sorted rows. Rows are valid by
        construction, so this does not raise.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.cashflows import schedule_from_classified_flows
    >>> from finstack_quant.core.cashflow import CashFlow, CFKind
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.money import Money
    >>> flow = CashFlow(
    ...     datetime.date(2025, 6, 15),
    ...     Money(0.0, "USD"),
    ...     CFKind.PIK,
    ...     0.0,
    ...     principal=(Money(100.0, "USD"), "2025-06-15"),
    ... )
    >>> schedule_from_classified_flows([flow], DayCount.ACT_360).get_flows()[0].kind.name
    'pik'
    """
    ...

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
