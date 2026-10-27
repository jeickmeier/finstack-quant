"""Stub declarations mirroring ``finstack-quant-py/src/bindings/cashflows/builder/schedule.rs``."""

from __future__ import annotations

import datetime
from typing import Any
import pandas as pd
from finstack_quant.cashflows.aggregation import PeriodAggregation
from finstack_quant.cashflows.fixings import ProjectedFixing
from finstack_quant.core.cashflow import CashFlow
from finstack_quant.core.dates import DayCount, Period
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.cashflows.builder._orchestrator import CashFlowBuilder
from finstack_quant.cashflows.builder._specs.amortization import Notional

class CashFlowMeta:
    """
    Schedule-level metadata: representation, calendars, facility limit, horizon.

    Examples
    --------
    >>> from finstack_quant.cashflows.builder import CashFlowMeta
    >>> CashFlowMeta(calendar_ids=["usny"]).calendar_ids
    ['usny']
    """

    def __init__(
        self,
        representation: str = "contractual",
        calendar_ids: list[str] | None = None,
        commitment: Money | None = None,
        issue_date: datetime.date | str | None = None,
        maturity: datetime.date | str | None = None,
        projected_fixings: list[ProjectedFixing] | None = None,
    ) -> None:
        """
        Construct schedule metadata.

        Parameters
        ----------
        representation : str
            One of ``"contractual"``, ``"projected"``, ``"placeholder"``,
            ``"no_residual"``.
        calendar_ids : list[str], optional
            Holiday calendar identifiers used by the schedule. Informational:
            no Rust engine reads it.
        commitment : Money, optional
            Facility limit / commitment for revolving structures.
            Informational: no Rust engine reads it.
        issue_date : datetime.date or str, optional
            Instrument issue date. Accrual and the ``outstanding_*`` methods
            raise ``ValueError`` without it; validation falls back to the
            first flow date.
        maturity : datetime.date or str, optional
            Contractual maturity date.
        projected_fixings : list[ProjectedFixing], optional
            Raw rate or FX observations from canonical coupon projection, before
            spread/gearing/caps/floors; defaults to an empty list. Time rolls use
            them to materialize crossed resets while preserving existing fixings.

        Raises
        ------
        ValueError
            If ``representation`` is not an accepted label or a date cannot
            be parsed.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import CashFlowMeta
        >>> CashFlowMeta("projected").representation
        'projected'
        """
        ...

    @property
    def representation(self) -> str:
        """
        Schedule representation label.

        Returns
        -------
        str
            One of ``"contractual"``, ``"projected"``, ``"placeholder"``,
            or ``"no_residual"``, describing the schedule's meaning
            relative to waterfall policy.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def calendar_ids(self) -> list[str]:
        """
        Holiday calendar identifiers used by the schedule.

        Returns
        -------
        list[str]
            Calendar ids referenced while generating schedule dates.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def commitment(self) -> Money | None:
        """
        Optional facility limit / commitment.

        Returns
        -------
        Money, optional
            The facility limit for instruments like RCFs, or ``None`` when
            not applicable.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def issue_date(self) -> datetime.date | None:
        """
        Instrument issue date, when known.

        Returns
        -------
        datetime.date, optional
            The issue date, which accrual and the ``outstanding_*`` methods
            require as the first coupon period start, or ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def maturity(self) -> datetime.date | None:
        """
        Contractual maturity date, when known.

        Returns
        -------
        datetime.date, optional
            The final maturity date, or ``None`` when unset.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def projected_fixings(self) -> list[ProjectedFixing]:
        """Return the raw observations retained for future market time rolls.

        Returns
        -------
        list[ProjectedFixing]
            Copy of required raw index/FX observations, including unavailable
            projections recorded as ``None``. Existing market fixings take priority.

        Notes
        -----
        This accessor does not raise and does not mutate the schedule.
        """
        ...

    def __repr__(self) -> str: ...
    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            Strict-serde JSON document; round-trips through :meth:`from_json`.

        Raises
        ------
        ValueError
            If a field cannot be represented in JSON (non-finite float).

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import CashFlowMeta
        >>> isinstance(CashFlowMeta().to_json(), str)
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> CashFlowMeta:
        """
        Deserialize from the canonical JSON wire form (strict field names).

        Parameters
        ----------
        json : str
            JSON document produced by :meth:`to_json`.

        Returns
        -------
        CashFlowMeta
            Reconstructed value.

        Raises
        ------
        ValueError
            If the JSON is malformed or carries unknown fields.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import CashFlowMeta
        >>> value = CashFlowMeta()
        >>> CashFlowMeta.from_json(value.to_json()).to_json() == value.to_json()
        True
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Pickle support via the JSON wire form (``from_json``, ``(to_json(),)``).

        Returns
        -------
        tuple
            ``(from_json, (json,))`` reconstructor pair.

        Raises
        ------
        ValueError
            If the value holds a non-finite float that JSON cannot carry.
        """
        ...

class CashFlowSchedule:
    """
    Canonical, deterministically ordered cashflow schedule.

    The output of :meth:`builder`, which is the only construction path.

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
    ...     .fixed_cf(FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.quarterly_act360()))
    ...     .build()
    ... )
    >>> len(schedule.get_flows())
    6
    """

    @staticmethod
    def builder() -> CashFlowBuilder:
        """
        Create a new fluent cashflow builder (the only builder entry point).

        Returns
        -------
        CashFlowBuilder
            A fresh, empty builder ready for chained configuration.

        Notes
        -----
        This factory does not raise; it returns a new instance with the documented defaults.

        Examples
        --------
        >>> from finstack_quant.cashflows.builder import CashFlowSchedule
        >>> CashFlowSchedule.builder() is not None
        True
        """
        ...

    def get_flows(self) -> list[CashFlow]:
        """
        Return the canonical, deterministically ordered cashflows.

        Returns
        -------
        list[CashFlow]
            All cashflows in the schedule (coupons, principal payments,
            fees) sorted into deterministic schedule order.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def coupons(self) -> list[CashFlow]:
        """
        Interest-like coupon cashflows.

        Returns
        -------
        list[CashFlow]
            Flows whose kind is Fixed, FloatReset, InflationCoupon, or
            Stub, in schedule order.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def dates(self) -> list[datetime.date]:
        """
        Return the list of dates for all flows in schedule order.

        Returns
        -------
        list[datetime.date]
            One date per flow, matching the order of :meth:`get_flows`.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def get_notional(self) -> Notional:
        """
        Return the schedule notional.

        Returns
        -------
        Notional
            The representative notional amount and amortization rule.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def get_day_count(self) -> DayCount:
        """
        Return the representative day-count convention.

        Returns
        -------
        DayCount
            The day-count convention recorded on the schedule.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def get_meta(self) -> CashFlowMeta:
        """
        Return schedule-level metadata.

        Returns
        -------
        CashFlowMeta
            The representation label, calendar ids, facility limit, and
            issue/maturity dates recorded on the schedule.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def validate(self) -> None:
        """
        Validate all schedule-level and per-flow invariants.

        Checks the representative notional, each flow, nondecreasing flow
        dates, and cross-flow economic invariants reconciling funding,
        outstanding balances, currencies, and recorded metadata.

        Raises
        ------
        ValueError
            If the notional, any flow, the date ordering, or a cross-flow
            economic invariant is violated.
        """
        ...

    def scale_amounts(self, scale: float) -> CashFlowSchedule:
        """
        Return a new schedule with every amount scaled by ``scale``.

        Parameters
        ----------
        scale : float
            Multiplier applied to every flow amount; a negative value
            reverses the cashflow direction.

        Returns
        -------
        CashFlowSchedule
            A new schedule with scaled amounts; the representative
            notional and flow classification/date metadata are unchanged.

        Raises
        ------
        ValueError
            If *scale* is non-finite or a scaled cash amount or principal
            movement exceeds the Decimal range.
        """
        ...

    def wal(self, as_of: datetime.date | str) -> float:
        """
        Weighted Average Life (WAL) in years from ``as_of``.

        Computed on an Act/365F basis regardless of the schedule's accrual
        day count, matching conventional desk reporting. Each repayment is
        timed at its balance-effective date (the principal date, else the
        payment date) and weighted by its par principal (the principal
        delta, else the cash amount), so a payment lag or a premium/OID cash
        amount does not move WAL.

        Parameters
        ----------
        as_of : datetime.date
            Valuation date from which the WAL is measured; only repayments
            whose balance-effective date is after it contribute.

        Returns
        -------
        float
            The weighted average life in years; ``0.0`` if there are no
            future principal repayments. Funding draws and non-cash PIK or
            default write-downs do not contribute; scheduled amortization,
            prepayments, final redemption, and revolving repayments do.

        Raises
        ------
        ValueError
            If future principal repayments mix currencies or the day-count
            year-fraction calculation fails.
        """
        ...

    def outstanding_by_date(self) -> list[tuple[datetime.date, Money]]:
        """
        Outstanding principal balance path over the schedule's life.

        Returns
        -------
        list[tuple[datetime.date, Money]]
            ``(date, outstanding_balance)`` pairs in economic principal-date
            order, one per distinct balance-effective date.

        Raises
        ------
        ValueError
            If ``meta.issue_date`` is unset, or a currency mismatch is
            found between flows and the notional.
        """
        ...

    def pv_by_period(
        self,
        periods: list[Period],
        market: MarketContext,
        disc_curve_id: str,
        base: datetime.date,
        credit_curve_id: str | None = None,
    ) -> PeriodAggregation:
        """
        Periodized present values resolved from a market context.

        Parameters
        ----------
        periods : list[Period]
            Reporting periods that define the output buckets (half-open
            boundaries).
        market : MarketContext
            Market context containing the required discount (and optional
            hazard) curves.
        disc_curve_id : str
            Discount curve identifier.
        base : datetime.date
            Valuation date: discount factors and survival are relative to it,
            and flows on or before it have zero present value. Each curve uses
            its own day count.
        credit_curve_id : str, optional
            Hazard curve identifier for credit-adjusted present value.

        Returns
        -------
        PeriodAggregation
            Read-only mapping from period id label (e.g. ``"2025Q1"``) to
            per-currency present-value totals, with ``to_dataframe()``;
            periods with no flows are omitted.

        Raises
        ------
        KeyError
            If the discount or hazard curve id cannot be found in *market*.
        ValueError
            If a discount factor or survival probability is invalid, or
            credit-adjusted inputs are internally inconsistent.

        Notes
        -----
        Binds Rust ``CashFlowSchedule::pv_by_period`` (market-resolved).
        """
        ...

    def to_json(self) -> str:
        """
        Serialize the canonical schedule to JSON.

        Returns
        -------
        str
            Canonical JSON encoding of the schedule, compatible with
            :func:`finstack_quant.cashflows.validate_cashflow_schedule_json`
            and other JSON-bridge functions on
            ``finstack_quant.cashflows``.

        Raises
        ------
        ValueError
            If the value cannot be serialized to JSON, including a non-finite
            coupon rate or projected index rate on any cashflow. Invalid rates
            are rejected rather than serialized as absent metadata.
        """
        ...

    @staticmethod
    def from_json(json: str) -> CashFlowSchedule:
        """
        Deserialize a schedule from canonical JSON.

        Parameters
        ----------
        json : str
            JSON-encoded ``CashFlowSchedule`` with strict field names.

        Returns
        -------
        CashFlowSchedule
            The deserialized schedule.

        Raises
        ------
        ValueError
            If *json* is not valid JSON or does not match the canonical
            ``CashFlowSchedule`` schema (unknown fields are rejected).

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
        ...     .fixed_cf(FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.quarterly_act360()))
        ...     .build()
        ... )
        >>> round_tripped = CashFlowSchedule.from_json(schedule.to_json())
        >>> len(round_tripped.get_flows())
        6
        """
        ...

    def __reduce__(self) -> tuple[Any, tuple[str]]:
        """
        Pickle support via the JSON wire form (``from_json``, ``(to_json(),)``).

        Returns
        -------
        tuple
            ``(from_json, (json,))`` reconstructor pair.

        Raises
        ------
        ValueError
            If the schedule holds a non-finite float that JSON cannot carry.
        """
        ...

    def to_dataframe(self, outstanding: bool = False) -> pd.DataFrame:
        """
        Flows as a pandas DataFrame.

        Parameters
        ----------
        outstanding : bool, default False
            Append an ``outstanding`` column with the principal balance (float,
            notional currency) after all economic principal events on or before
            each payment date. Before the first event, use the initial notional.

        Returns
        -------
        pandas.DataFrame
            One row per flow, in schedule order, with columns ``date``
            (``datetime64``), ``reset_date`` (``datetime64``, ``NaT`` when
            absent), ``kind``, ``amount`` (float), ``currency``,
            ``accrual_factor``, ``rate`` (``NaN`` when absent), ``accrual``
            (canonical metadata dict or None), ``principal_date`` (optional economic date), ``principal_delta`` (canonical
            money dict or None), and optionally
            ``outstanding``.

        Raises
        ------
        ValueError
            If ``outstanding=True`` and ``meta.issue_date`` is missing or
            principal flows mix currencies with the notional.
        """
        ...

    def _repr_html_(self) -> str | None:
        """
        HTML table for Jupyter, delegating to ``to_dataframe()``.

        Returns
        -------
        str or None
            pandas HTML rendering, or ``None`` to fall back to ``__repr__``.
        """
        ...

    def calendar_year_ladder(self, pvs: list[float]) -> pd.DataFrame:
        """
        Calendar-year non-principal / principal / PV ladder of this schedule.

        Returns a ``pandas.DataFrame`` view alongside :meth:`to_dataframe`; use
        :func:`finstack_quant.cashflows.aggregation.calendar_year_ladder` for raw
        ``(year, non_principal, principal, pv)`` tuples over parallel arrays.

        Parameters
        ----------
        pvs : list[float]
            Present value of each flow in ``get_flows()`` order, in the flow's
            currency units; must be finite.

        Returns
        -------
        pandas.DataFrame
            Columns ``year``, ``non_principal``, ``principal``, ``pv`` sorted
            by year.

        Raises
        ------
        ValueError
            If ``pvs`` does not have one entry per flow or contains a
            non-finite value, or flow amounts mix currencies.
        """
        ...

    def with_representation(self, representation: str) -> CashFlowSchedule:
        """
        Copy with the metadata representation label replaced.

        Parameters
        ----------
        representation : str
            One of ``"contractual"``, ``"projected"``, ``"placeholder"``,
            ``"no_residual"``.

        Returns
        -------
        CashFlowSchedule
            New schedule sharing the flows.

        Raises
        ------
        ValueError
            If the label is not an accepted representation.
        """
        ...

    def with_notional(self, notional: Notional) -> CashFlowSchedule:
        """
        Copy with the representative notional replaced (flows unchanged).

        Parameters
        ----------
        notional : Notional
            New representative notional.

        Returns
        -------
        CashFlowSchedule
            New schedule with the replaced notional.

        Notes
        -----
        This method does not raise.
        """
        ...

    @staticmethod
    def from_flows(
        flows: list[CashFlow] | pd.DataFrame,
        notional: Notional,
        day_count: DayCount,
        meta: CashFlowMeta | None = None,
    ) -> CashFlowSchedule:
        """
        Build a schedule from ``CashFlow`` rows or a pandas DataFrame,
        validating every row.

        Both input shapes go through Rust ``CashFlowSchedule::from_flows``,
        which checks each row (``CashFlow.validate``) before sorting.

        Parameters
        ----------
        flows : list[CashFlow] or pandas.DataFrame
            Typed rows, or a frame with columns ``date``, ``amount`` (float,
            native currency units), ``currency`` (ISO code), ``kind``
            (``CFKind`` label), ``accrual_factor`` (float year fraction,
            required; ``0.0`` for non-coupon rows) and optional
            ``reset_date``, ``rate`` (``NaN`` / ``None`` = absent),
            ``accrual`` and ``principal_date`` (optional economic date), ``principal_delta`` (canonical JSON-compatible dicts
            or None). Exported metadata is preserved when rebuilding.
        notional : Notional
            Representative notional stamped on the schedule.
        day_count : DayCount
            Representative day-count convention.
        meta : CashFlowMeta, optional
            Schedule metadata (default contractual, no calendars).

        Returns
        -------
        CashFlowSchedule
            Canonical schedule holding the sorted rows.

        Raises
        ------
        ValueError
            If a required frame column is missing, a currency code, kind
            label or date cannot be parsed, or a row fails
            ``CashFlow.validate`` (e.g. a negative or non-finite
            ``accrual_factor``, or a reset date after the payment date).
        TypeError
            If ``flows`` is neither a list of ``CashFlow`` nor a DataFrame.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.cashflows.builder import CashFlowSchedule, Notional
        >>> from finstack_quant.core.cashflow import CashFlow, CFKind
        >>> from finstack_quant.core.dates import DayCount
        >>> from finstack_quant.core.money import Money
        >>> flow = CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.FIXED, 0.0)
        >>> CashFlowSchedule.from_flows([flow], Notional.par(1_000.0, "USD"), DayCount.ACT_360).get_flows()[0].kind.name
        'fixed'
        """
        ...

    def __repr__(self) -> str: ...

def merge_cashflow_schedules(
    schedules: list[CashFlowSchedule],
    notional: Notional,
    day_count: DayCount,
) -> CashFlowSchedule:
    """
    Merge multiple schedules into one deterministic composite schedule.

    Parameters
    ----------
    schedules : list[CashFlowSchedule]
        Schedules to combine.
    notional : Notional
        Representative notional stamped on the merged schedule.
    day_count : DayCount
        Day-count convention attached to the merged schedule.

    Returns
    -------
    CashFlowSchedule
        The merged schedule. Flows from all inputs are combined and
        re-sorted into canonical order. Metadata is merged: ``representation``
        takes the most conservative value across inputs (``Projected``
        dominates, then ``Placeholder``, ``Contractual``, ``NoResidual``);
        ``calendar_ids`` is the sorted, deduplicated union; ``commitment``
        and ``issue_date`` are kept only when every input agrees, otherwise
        ``None``. Empty input yields an empty schedule with default metadata.

    Raises
    ------
    TypeError
        If an element of *schedules* is not a ``CashFlowSchedule``, or
        *notional* / *day_count* are not the expected types.

    Examples
    --------
    >>> import datetime
    >>> from decimal import Decimal
    >>> from finstack_quant.cashflows.builder import (
    ...     CashFlowSchedule,
    ...     FixedCouponSpec,
    ...     Notional,
    ...     ScheduleParams,
    ...     merge_cashflow_schedules,
    ... )
    >>> from finstack_quant.core.dates import DayCount
    >>> from finstack_quant.core.money import Money
    >>> schedule = (
    ...     CashFlowSchedule
    ...     .builder()
    ...     .principal(Money(1_000_000.0, "USD"), datetime.date(2025, 1, 15), datetime.date(2026, 1, 15))
    ...     .fixed_cf(FixedCouponSpec(rate=Decimal("0.05"), schedule=ScheduleParams.quarterly_act360()))
    ...     .build()
    ... )
    >>> merged = merge_cashflow_schedules([schedule], Notional.par(1_000_000.0, "USD"), DayCount.ACT_360)
    >>> len(merged.get_flows())
    6
    """
    ...
