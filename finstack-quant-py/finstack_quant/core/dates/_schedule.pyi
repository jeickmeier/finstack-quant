"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/dates/schedule.rs``."""

from __future__ import annotations

import datetime
from typing import Iterator, Union
import pandas as pd
from finstack_quant.core.dates._calendar import BusinessDayConvention, HolidayCalendar
from finstack_quant.core.dates._tenor import Tenor

# Schedule

class StubKind:
    """
    Stub positioning rule for schedule generation.

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import StubKind
    >>> str(StubKind.SHORT_FRONT)
    'short_front'

    """

    NONE: StubKind
    """No stub -- periods divide evenly."""
    SHORT_FRONT: StubKind
    """Short stub at the front."""
    SHORT_BACK: StubKind
    """Short stub at the back."""
    LONG_FRONT: StubKind
    """Long stub at the front."""
    LONG_BACK: StubKind
    """Long stub at the back."""

    @classmethod
    def from_name(cls, name: str) -> StubKind:
        """
        Parse this variant from its canonical name string.

        Parameters
        ----------
        name : str
            Stub kind identifier (e.g. ``"short_front"``, ``"long_back"``).

        Returns
        -------
        StubKind

            No-stub, front-stub, or back-stub rule represented by the exact
            canonical lowercase name.

        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.dates import StubKind
        >>> StubKind.from_name("short_front") == StubKind.SHORT_FRONT
        True

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class ScheduleErrorPolicy:
    """
    Error handling policy for schedule building.

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import ScheduleErrorPolicy
    >>> ScheduleErrorPolicy.STRICT != ScheduleErrorPolicy.GRACEFUL_EMPTY
    True

    """

    STRICT: ScheduleErrorPolicy
    """Strict -- errors are immediately propagated."""
    MISSING_CALENDAR_WARNING: ScheduleErrorPolicy
    """Emit a warning for missing calendars, but continue."""
    GRACEFUL_EMPTY: ScheduleErrorPolicy
    """Gracefully return an empty schedule on error."""

    @classmethod
    def from_name(cls, name: str) -> ScheduleErrorPolicy:
        """
        Parse this variant from its snake_case name, case-insensitively.

        Parameters
        ----------
        name : str
            One of ``"strict"``, ``"missing_calendar_warning"``,
            ``"graceful_empty"``.

        Returns
        -------
        ScheduleErrorPolicy
            The matching policy.

        Raises
        ------
        ValueError
            If *name* is not one of the three policy names.

        Examples
        --------
        >>> from finstack_quant.core.dates import ScheduleErrorPolicy
        >>> ScheduleErrorPolicy.from_name("graceful_empty") == ScheduleErrorPolicy.GRACEFUL_EMPTY
        True

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class Schedule:
    """
    A generated date schedule.

    Immutable value type produced by :class:`ScheduleBuilder`.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import Schedule
    >>> schedule = Schedule.builder(datetime.date(2025, 1, 15), datetime.date(2025, 7, 15)).frequency("3M").build()
    >>> schedule.dates
    [datetime.date(2025, 1, 15), datetime.date(2025, 4, 15), datetime.date(2025, 7, 15)]

    """

    @staticmethod
    def builder(start: datetime.date | str, end: datetime.date | str) -> ScheduleBuilder:
        """
        Start a schedule build between two dates.

        The canonical entry point, mirroring the ``Type.builder()`` form every
        other builder-backed type uses. Constructing :class:`ScheduleBuilder`
        directly is equivalent.

        Parameters
        ----------
        start : datetime.date | str
            First accrual date.
        end : datetime.date | str
            Final accrual date.

        Returns
        -------
        ScheduleBuilder
            A fresh builder defaulting to a monthly frequency.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.dates import Schedule
        >>> schedule = Schedule.builder(datetime.date(2025, 1, 15), datetime.date(2025, 7, 15)).frequency("3M").build()
        >>> len(schedule.dates)
        3

        Raises
        ------
        ValueError
            If *start* is after *end* or either date is invalid.
        """
        ...

    @staticmethod
    def from_spec(spec: Union[dict, str]) -> Schedule:
        """
        Build a schedule from a serialized ``ScheduleSpec``.

        Parameters
        ----------
        spec : dict | str
            Mapping or JSON string with the canonical fields ``start``,
            ``end`` (ISO dates), ``frequency`` (``"3M"``), ``stub``,
            ``business_day_convention``, ``calendar_id``, ``end_of_month``,
            ``imm_mode``, ``cds_imm_mode``, ``error_policy``,
            ``payment_lag_days`` and ``fixing_lag_business_days``
            (as produced by :meth:`ScheduleBuilder.to_spec`).

        Returns
        -------
        Schedule
            The generated schedule.

        Raises
        ------
        ValueError
            If the spec is malformed or the schedule cannot be built.

        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> spec = Schedule.builder("2025-01-15", "2025-07-15").frequency("3M").to_spec()
        >>> len(Schedule.from_spec(spec))
        3

        """
        ...

    @property
    def dates(self) -> list[datetime.date]:
        """
        Unadjusted accrual dates as a list of ``datetime.date``.

        These dates are the roll-grid anchors and are never business-day
        adjusted. Payment-date adjustment lives on ``payment_dates``.

        Returns
        -------
        list[datetime.date]
            Monotonic unadjusted accrual grid (period start plus each period end).

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def payment_dates(self) -> list[datetime.date]:
        """
        Payment date for each accrual period (one per period end).

        Length is ``len(dates) - 1`` for a non-empty schedule. Duplicate
        payment dates are retained so the series stays 1:1 with period ends.

        Returns
        -------
        list[datetime.date]
            Adjusted (and optionally lagged) payment dates.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def fixing_dates(self) -> list[datetime.date]:
        """
        Fixing dates for each accrual period.

        Empty when no fixing lag was configured; otherwise the same length
        as ``payment_dates``. Each date is the period's accrual start minus
        the configured T-minus business-day lag.

        Returns
        -------
        list[datetime.date]
            Fixing dates, or an empty list when no fixing lag is set.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    def has_warnings(self) -> bool:
        """
        Whether any warnings were generated during schedule building.

        Returns
        -------
        bool
            Whether this `Schedule` has warnings.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

    def used_graceful_fallback(self) -> bool:
        """
        Whether a graceful fallback was used during schedule building.

        Returns
        -------
        bool
            ``True`` exactly when the schedule warnings include a graceful-fallback warning.

        Notes
        -----
        This method does not raise; it returns ``True`` or ``False``.
        """
        ...

    @property
    def warnings(self) -> list[dict[str, object]]:
        """
        Warnings generated during schedule construction.

        Returns
        -------
        list[dict[str, object]]
            One dict per warning with ``kind`` (``"graceful_fallback"`` or
            ``"missing_calendar_id"``), ``message`` (human-readable text) and
            the warning's own field (``error_message`` or ``calendar_id``).
            Empty under the strict policy.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Accrual periods as a pandas DataFrame.

        Returns
        -------
        pandas.DataFrame
            One row per period with ``datetime64`` columns ``period_start``,
            ``period_end``, ``payment_date`` and ``fixing_date`` (``NaT`` when
            no fixing lag is configured).

        Raises
        ------
        ImportError
            If pandas is not installed.

        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> frame = Schedule.builder("2025-01-15", "2026-01-15").frequency("6M").build().to_dataframe()
        >>> list(frame.columns)
        ['period_start', 'period_end', 'payment_date', 'fixing_date']
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form (strict field names).

        Returns
        -------
        str
            JSON object with ``dates``, ``payment_dates``, ``fixing_dates``
            (ISO strings) and, when present, ``warnings``.

        Raises
        ------
        ValueError
            If the schedule cannot be serialized.

        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> schedule = Schedule.builder("2025-01-15", "2025-07-15").frequency("3M").build()
        >>> Schedule.from_json(schedule.to_json()) == schedule
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> Schedule:
        """
        Deserialize from the canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        Schedule
            The reconstructed schedule.

        Raises
        ------
        ValueError
            If *json* is malformed.
        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> schedule = Schedule.builder("2025-01-15", "2025-07-15").frequency("3M").build()
        >>> Schedule.from_json(schedule.to_json()) == schedule
        True

        """
        ...

    def __iter__(self) -> Iterator[datetime.date]: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class ScheduleBuilder:
    """
    Builder for constructing date schedules.

    Setters mutate the builder **in place** and return that same instance,
    matching Rust's fluent builder semantics.

    Parameters
    ----------
    start : datetime.date | str
        Schedule start date.
    end : datetime.date | str
        Schedule end date (must not be before *start*; validated by the
        canonical Rust builder at ``build()`` time).

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.dates import (
    ...     ScheduleBuilder,
    ...     StubKind,
    ...     BusinessDayConvention,
    ...     ScheduleErrorPolicy,
    ... )
    >>> schedule = (
    ...     Schedule
    ...     .builder(date(2025, 1, 15), date(2030, 1, 15))
    ...     .frequency("3M")
    ...     .stub_rule(StubKind.SHORT_FRONT)
    ...     .adjust_with(BusinessDayConvention.MODIFIED_FOLLOWING, "usny")
    ...     .end_of_month(False)
    ...     .error_policy(ScheduleErrorPolicy.STRICT)
    ...     .build()
    ... )
    >>> len(schedule) >= 20
    True
    """

    def frequency(self, frequency: Union[Tenor, str]) -> ScheduleBuilder:
        """
        Set the coupon/roll frequency.

        Parameters
        ----------
        frequency : Tenor | str
            Tenor object or string like ``"3M"``.

        Returns
        -------
        ScheduleBuilder
            Same builder instance after replacing its validated coupon or roll tenor.

        Raises
        ------
        ValueError
            If a string *frequency* is not a valid supported tenor.

        """
        ...

    def stub_rule(self, stub: Union[StubKind, str]) -> ScheduleBuilder:
        """
        Set how a short first or last stub period is generated.

        Parameters
        ----------
        stub : StubKind | str
            Stub positioning rule, or its name (``"short_front"``, ...).

        Returns
        -------
        ScheduleBuilder
            Same builder instance after replacing its stub rule with ``stub``.

        Raises
        ------
        ValueError
            If a string *stub* is not a stub-kind name.
        """
        ...

    def adjust_with(
        self,
        convention: Union[BusinessDayConvention, str],
        calendar: Union[HolidayCalendar, str],
    ) -> ScheduleBuilder:
        """
        Set the business-day convention and calendar used to adjust payment dates.

        Parameters
        ----------
        convention : BusinessDayConvention | str
            Roll rule (for example ``"modified_following"``), ignoring outer whitespace and ASCII case.
        calendar : HolidayCalendar | str
            Holiday calendar object or registry id (``"target2"``;
            ``"nyse+gblo"`` joins calendars). A string id is resolved at
            ``build()`` under the error policy (``STRICT`` raises
            ``KeyError`` for unknown ids).

        Returns
        -------
        ScheduleBuilder
            Same builder instance after storing the convention and calendar
            used to adjust dates during ``build()``.

        Raises
        ------
        ValueError
            If *convention* is unknown.
        """
        ...

    def payment_lag_days(self, lag: int) -> ScheduleBuilder:
        """
        Shift each payment date by *lag* business days after the adjusted period end.

        Parameters
        ----------
        lag : int
            Non-negative business-day delay from each period's payment
            anchor. Zero is T+0. A positive lag requires a calendar from
            ``adjust_with``. Negative values are rejected at ``build()``.

        Returns
        -------
        ScheduleBuilder
            Same builder instance after storing the payment lag.

        Notes
        -----
        This method does not raise; validation happens in ``build()``.
        """
        ...

    def fixing_lag_business_days(self, lag: int) -> ScheduleBuilder:
        """
        Set a T-minus fixing lag from each period's unadjusted accrual start.

        Parameters
        ----------
        lag : int
            Non-negative business-day lookback from each period's accrual
            start. Zero stores the accrual start itself. A positive lag
            requires a calendar from ``adjust_with``. Negative values are
            rejected at ``build()``.

        Returns
        -------
        ScheduleBuilder
            Same builder instance after storing the fixing lag.

        Notes
        -----
        This method does not raise; validation happens in ``build()``.
        """
        ...

    def end_of_month(self, eom: bool) -> ScheduleBuilder:
        """
        Enable or disable end-of-month roll logic.

        Parameters
        ----------
        eom : bool
            Whether to enable end-of-month rolling. Requires a month/year tenor
            and a regular schedule rule; ``build`` raises ``ValueError`` for
            day/week tenors or IMM/CDS modes. With ``StubKind.NONE``, maturity
            must lie on the generated month-end roll grid; otherwise an
            explicit stub rule is required.

        Returns
        -------
        ScheduleBuilder
            Same builder instance after enabling or disabling end-of-month rolling.

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def cds_imm(self) -> ScheduleBuilder:
        """
        Use CDS IMM dates (the 20th of March, June, September and December).

        Same semantics as the Rust ``ScheduleBuilder::cds_imm``: sets the
        frequency to quarterly and the stub rule to ``short_back``, turns CDS
        IMM mode on and standard IMM mode off. A later ``frequency`` or
        ``stub_rule`` call wins, and ``to_spec()`` reports exactly what
        ``build()`` uses.

        Returns
        -------
        ScheduleBuilder
            This builder.

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.

        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> spec = Schedule.builder("2025-01-15", "2026-01-15").cds_imm().to_spec()
        >>> spec["frequency"], spec["stub"]
        ({'count': 3, 'unit': 'months'}, 'short_back')
        """
        ...

    def imm(self) -> ScheduleBuilder:
        """
        Roll on standard IMM dates (the third Wednesday of Mar, Jun, Sep, Dec).

        Same semantics as the Rust ``ScheduleBuilder::imm``: sets the IMM
        defaults (quarterly frequency, ``short_back`` stub rule), turns standard
        IMM mode on and CDS IMM mode off. A later ``frequency`` or ``stub_rule``
        call wins. ``build()`` keeps ``start`` and ``end`` as the first and last
        dates and lays the regular periods on IMM dates at the frequency, which
        must be a whole number of quarters: forward from the first IMM date on
        or after ``start`` for back-stub rules, backward from the last IMM date
        on or before ``end`` for front-stub rules. With no IMM date strictly
        between ``start`` and ``end`` the schedule is the single period
        ``[start, end]``.

        Returns
        -------
        ScheduleBuilder
            This builder.

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        ``build()`` raises ``ValueError`` when the frequency is not a whole
        number of quarters, or when the stub rule is ``none`` and ``start`` or
        ``end`` is not an IMM date.

        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> dates = Schedule.builder("2025-01-15", "2025-12-31").imm().build().dates
        >>> [d.isoformat() for d in dates]
        ['2025-01-15', '2025-03-19', '2025-06-18', '2025-09-17', '2025-12-17', '2025-12-31']
        """
        ...

    def error_policy(self, policy: Union[ScheduleErrorPolicy, str]) -> ScheduleBuilder:
        """
        Set how invalid schedule dates are reported or skipped.

        Setting a policy fully replaces any previous policy; calls are
        order-independent and idempotent.

        Parameters
        ----------
        policy : ScheduleErrorPolicy | str
            Error handling policy, or its name (``"strict"``,
            ``"missing_calendar_warning"``, ``"graceful_empty"``).

        Returns
        -------
        ScheduleBuilder
            Same builder instance after fully replacing its schedule-build error policy.

        Raises
        ------
        ValueError
            If a string *policy* is not a policy name.
        """
        ...

    def to_spec(self) -> dict[str, object]:
        """
        Current builder state as a ``ScheduleSpec`` mapping.

        Returns
        -------
        dict[str, object]
            The canonical spec fields (``start``, ``end``, ``frequency``,
            ``stub``, ``business_day_convention``, ``calendar_id``,
            ``end_of_month``, ``imm_mode``, ``cds_imm_mode``,
            ``error_policy``, ``payment_lag_days``,
            ``fixing_lag_business_days``), accepted by
            :meth:`Schedule.from_spec`.

        Notes
        -----
        This method does not raise; it returns the current state.

        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> Schedule.builder("2025-01-15", "2025-07-15").frequency("3M").to_spec()["frequency"]
        {'count': 3, 'unit': 'months'}
        """
        ...

    def build(self) -> Schedule:
        """
        Materialize the date schedule from the builder settings.

        Delegates entirely to the canonical Rust ``ScheduleBuilder::build``:
        under the default ``STRICT`` policy a generation failure raises
        ``ValueError`` and an unknown calendar id ``KeyError``. Under ``MISSING_CALENDAR_WARNING`` or
        ``GRACEFUL_EMPTY`` the schedule is returned carrying its warnings
        (inspect via ``Schedule.warnings`` / ``Schedule.has_warnings()``).

        Returns
        -------
        Schedule
            The constructed schedule.

        Raises
        ------
        ValueError
            If the schedule cannot be built with the given parameters, or
            if warnings occur under the strict policy.
        """
        ...

    def __repr__(self) -> str: ...
