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
>>> CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.FIXED).amount.amount
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
    "accrued_interest",
    "aggregation",
    "build_cashflow_schedule",
    "build_cashflow_schedule_json",
    "builder",
    "fixings",
    "cdr_to_mdr",
    "cpr_to_smm",
    "dated_flows",
    "dated_flows_json",
    "mdr_to_cdr",
    "primitives",
    "schedule_from_classified_flows",
    "schedule_from_dated_flows",
    "schema",
    "smm_to_cpr",
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
        (``notional``, ``issue``, ``maturity``, ``coupon_program``,
        ``payment_program``, ``fees``, ``principal_events``,
        ``principal_exchange``). Each principal event requires economic ``date``
        and cash ``payment_date``; these dates may differ after payment adjustment.
    market : MarketContext or str, optional
        Market context (or its JSON) for floating-rate projection.

    Returns
    -------
    CashFlowSchedule
        Canonical typed schedule with ``to_dataframe()``.

    Raises
    ------
    ValueError
        If the spec is malformed or the schedule fails validation.
    KeyError
        If a floating leg references a curve missing from ``market``.

    Examples
    --------
    >>> from finstack_quant.cashflows import build_cashflow_schedule
    >>> spec = {
    ...     "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
    ...     "issue": "2025-01-15",
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
        default write-downs are omitted.

    Raises
    ------
    ValueError
        If ``schedule`` is a malformed JSON string.
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
    >>> flow = CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.PIK)
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
        when the schedule uses fixed coupons only.

    Returns
    -------
    str
        Canonical JSON-encoded ``CashFlowSchedule``.

    Raises
    ------
    ValueError
        If ``spec_json`` (or ``market_json`` when supplied) fails schema or
        semantic validation.
    KeyError
        If required market data or a fixing series is missing.

    Examples
    --------
    >>> import json
    >>> spec = {
    ...     "notional": {"initial": {"amount": "1000000", "currency": "USD"}, "amort": "none"},
    ...     "issue": "2024-08-31",
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
        ``DefaultedNotional`` state rows are omitted; parse the full schedule
        JSON when flow classification is required.

    Raises
    ------
    ValueError
        If ``schedule_json`` is invalid.

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

def accrued_interest(schedule_json: str, as_of: datetime.date | str, config_json: str | None = None) -> float:
    """
    Compute accrued interest for a schedule as of a valuation date.

    Parameters
    ----------
    schedule_json : str
        JSON-encoded ``CashFlowSchedule``.
    as_of : datetime.date | str
        Accrual snapshot date, either a date-like object or an ISO 8601 string.
    config_json : str, optional
        JSON-encoded ``AccrualConfig`` overriding default accrual conventions.

    Returns
    -------
    float
        Accrued interest in the schedule settlement currency. The Rust engine
        computes from the canonical schedule and crosses the binding boundary as
        ``f64``; for large notionals, compare with an absolute tolerance scaled
        to the schedule notional rather than expecting decimal-string equality.
        Returns ``0.0`` when ``as_of`` is outside all coupon periods.

    Raises
    ------
    ValueError
        If the schedule JSON or accrual configuration is invalid.
    KeyError
        If an ex-coupon calendar is unknown.

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
    >>> from finstack_quant.cashflows import accrued_interest
    >>> accrued_interest(schedule.to_json(), "2025-04-15") > 0.0
    True

    """

def cpr_to_smm(cpr: float) -> float:
    """
    Convert an annual CPR (constant prepayment rate) to a monthly SMM.

    Flat re-export of :func:`finstack_quant.cashflows.builder.cpr_to_smm`,
    mirroring the Rust crate-root re-export. Uses
    ``SMM = 1 - (1 - CPR)^(1/12)``.

    Parameters
    ----------
    cpr : float
        Annualized CPR as a decimal in ``[0, 1]`` (``0.06`` means 6%).

    Returns
    -------
    float
        Monthly SMM as a decimal fraction.

    Raises
    ------
    ValueError
        If ``cpr`` is negative, non-finite, or above ``1.0``.

    Examples
    --------
    >>> from finstack_quant.cashflows import cpr_to_smm
    >>> round(cpr_to_smm(0.06), 6)
    0.005143
    """
    ...

def smm_to_cpr(smm: float) -> float:
    """
    Convert a monthly SMM (single monthly mortality) to an annual CPR.

    Flat re-export of :func:`finstack_quant.cashflows.builder.smm_to_cpr`,
    mirroring the Rust crate-root re-export. Uses ``CPR = 1 - (1 - SMM)^12``.

    Parameters
    ----------
    smm : float
        Monthly SMM as a decimal in ``[0, 1]``.

    Returns
    -------
    float
        Annualized CPR as a decimal fraction.

    Raises
    ------
    ValueError
        If ``smm`` is negative, non-finite, or above ``1.0``.

    Examples
    --------
    >>> from finstack_quant.cashflows import cpr_to_smm, smm_to_cpr
    >>> round(smm_to_cpr(cpr_to_smm(0.06)), 10)
    0.06
    """
    ...

def cdr_to_mdr(cdr: float) -> float:
    """
    Convert an annual CDR (constant default rate) to a monthly MDR.

    Flat re-export of :func:`finstack_quant.cashflows.builder.cdr_to_mdr`,
    mirroring the Rust crate-root re-export. Default and prepayment mortality
    rates share the same kernel: ``MDR = 1 - (1 - CDR)^(1/12)``.

    Parameters
    ----------
    cdr : float
        Constant annual default rate as a decimal in ``[0, 1]``.

    Returns
    -------
    float
        Monthly MDR as a decimal fraction.

    Raises
    ------
    ValueError
        If ``cdr`` is negative, non-finite, or above ``1.0``.

    Examples
    --------
    >>> from finstack_quant.cashflows import cdr_to_mdr
    >>> round(cdr_to_mdr(0.02), 6)
    0.001682
    """
    ...

def mdr_to_cdr(mdr: float) -> float:
    """
    Convert a monthly MDR (monthly default rate) to an annual CDR.

    Flat re-export of :func:`finstack_quant.cashflows.builder.mdr_to_cdr`,
    mirroring the Rust crate-root re-export. Uses ``CDR = 1 - (1 - MDR)^12``.

    Parameters
    ----------
    mdr : float
        Monthly default rate as a decimal in ``[0, 1]``.

    Returns
    -------
    float
        Annualized CDR as a decimal fraction.

    Raises
    ------
    ValueError
        If ``mdr`` is negative, non-finite, or above ``1.0``.

    Examples
    --------
    >>> from finstack_quant.cashflows import cdr_to_mdr, mdr_to_cdr
    >>> round(mdr_to_cdr(cdr_to_mdr(0.02)), 10)
    0.02
    """
    ...
