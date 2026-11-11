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

class GridMode:
    """
    Regular-grid selection for schedule generation.

    Exactly one grid is chosen; there is no default. Every regular roll is
    computed from the original anchor (``start`` for no-stub and back-stub
    rules, ``end`` for front-stub rules) and snapped to the grid. Only the
    declared stub side may be off the grid; with ``StubKind.NONE`` both
    endpoints must be on it. Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import GridMode
    >>> str(GridMode.END_OF_MONTH)
    'end_of_month'

    """

    NATURAL: GridMode
    """Tenor-arithmetic dates with no snapping."""
    END_OF_MONTH: GridMode
    """Last calendar day of each month; month/year frequency and a month-end anchor."""
    QUARTERLY_IMM: GridMode
    """Third Wednesday of Mar/Jun/Sep/Dec; whole quarters and an IMM-date anchor."""

    @classmethod
    def from_name(cls, name: str) -> GridMode:
        """
        Parse this variant from its canonical name string.

        Parameters
        ----------
        name : str
            Grid identifier: ``"natural"``, ``"end_of_month"`` or
            ``"quarterly_imm"``.

        Returns
        -------
        GridMode
            The grid with that canonical lowercase name.

        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.dates import GridMode
        >>> GridMode.from_name("quarterly_imm") == GridMode.QUARTERLY_IMM
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
    >>> schedule = (
    ...     Schedule
    ...     .builder(datetime.date(2025, 1, 15), datetime.date(2025, 7, 15))
    ...     .stub_rule("none")
    ...     .grid("natural")
    ...     .unadjusted()
    ...     .frequency("3M")
    ...     .build()
    ... )
    >>> schedule.dates
    [datetime.date(2025, 1, 15), datetime.date(2025, 4, 15), datetime.date(2025, 7, 15)]

    """

    @staticmethod
    def builder(start: datetime.date | str, end: datetime.date | str) -> ScheduleBuilder:
        """
        Start a schedule build between two dates.

        The canonical entry point, mirroring the ``Type.builder()`` form every
        other builder-backed type uses. There are no financial defaults: the
        frequency, stub rule, grid and payment convention must all be chosen
        before :meth:`ScheduleBuilder.build`.

        Parameters
        ----------
        start : datetime.date | str
            First accrual date.
        end : datetime.date | str
            Final accrual date; must be after *start*.

        Returns
        -------
        ScheduleBuilder
            An empty builder for the range.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.dates import Schedule
        >>> schedule = (
        ...     Schedule
        ...     .builder(datetime.date(2025, 1, 15), datetime.date(2025, 7, 15))
        ...     .stub_rule("none")
        ...     .grid("natural")
        ...     .unadjusted()
        ...     .frequency("3M")
        ...     .build()
        ... )
        >>> len(schedule.dates)
        3

        Raises
        ------
        ValueError
            If *start* is not before *end* or either date is invalid.
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
            ``end`` (ISO dates), ``frequency``, ``stub``, ``grid``,
            ``cds_imm_mode``, ``business_day_convention``, ``calendar_id``,
            and the optional ``payment_lag_days`` and
            ``fixing_lag_business_days`` (as produced by
            :meth:`ScheduleBuilder.to_spec`).

        Returns
        -------
        Schedule
            The generated schedule.

        Raises
        ------
        ValueError
            If the spec is malformed, misses a required field, is
            inconsistent (CDS IMM on a non-natural grid, a convention other
            than unadjusted without a calendar) or cannot be built.

        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> spec = (
        ...     Schedule
        ...     .builder("2025-01-15", "2025-07-15")
        ...     .stub_rule("none")
        ...     .grid("natural")
        ...     .unadjusted()
        ...     .frequency("3M")
        ...     .to_spec()
        ... )
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

        Length is always ``len(dates) - 1``. Duplicate payment dates are
        retained so the series stays 1:1 with period ends.

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
        >>> frame = (
        ...     Schedule
        ...     .builder("2025-01-15", "2026-01-15")
        ...     .stub_rule("none")
        ...     .grid("natural")
        ...     .unadjusted()
        ...     .frequency("6M")
        ...     .build()
        ...     .to_dataframe()
        ... )
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
            JSON object with ``dates``, ``payment_dates`` and ``fixing_dates``
            (ISO strings).

        Raises
        ------
        ValueError
            If the schedule cannot be serialized.

        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> schedule = (
        ...     Schedule
        ...     .builder("2025-01-15", "2025-07-15")
        ...     .stub_rule("none")
        ...     .grid("natural")
        ...     .unadjusted()
        ...     .frequency("3M")
        ...     .build()
        ... )
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
            JSON produced by :meth:`to_json`: ``dates``, ``payment_dates`` and
            ``fixing_dates`` arrays, all required (``fixing_dates`` may be
            empty).

        Returns
        -------
        Schedule
            The reconstructed schedule. Only its shape is validated; it is not
            rebuilt against a calendar or specification.

        Raises
        ------
        ValueError
            If *json* is malformed, a field is missing or unknown, or the shape
            is invalid: fewer than 2 or more than 100,000 grid dates, grid
            dates not strictly increasing, ``len(dates) - 1`` payment dates not
            given, or a non-empty fixing series of the wrong length.
        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> schedule = (
        ...     Schedule
        ...     .builder("2025-01-15", "2025-07-15")
        ...     .stub_rule("none")
        ...     .grid("natural")
        ...     .unadjusted()
        ...     .frequency("3M")
        ...     .build()
        ... )
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

    Obtain one with :meth:`Schedule.builder`. Every financial choice is
    explicit: :meth:`frequency`, :meth:`stub_rule`, :meth:`grid` and a payment
    convention (:meth:`adjust_with` or :meth:`unadjusted`). An incomplete
    builder raises at :meth:`build` and :meth:`to_spec`. The payment lag
    defaults to zero and the fixing series to none.

    Examples
    --------
    >>> from datetime import date
    >>> from finstack_quant.core.dates import (
    ...     BusinessDayConvention,
    ...     Schedule,
    ...     StubKind,
    ... )
    >>> schedule = (
    ...     Schedule
    ...     .builder(date(2025, 1, 15), date(2030, 1, 15))
    ...     .frequency("3M")
    ...     .stub_rule(StubKind.SHORT_FRONT)
    ...     .adjust_with(BusinessDayConvention.MODIFIED_FOLLOWING, "usny")
    ...     .grid("natural")
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

    def grid(self, grid: Union[GridMode, str]) -> ScheduleBuilder:
        """
        Select the regular grid.

        Selecting a grid never changes the frequency or stub rule.

        Parameters
        ----------
        grid : GridMode | str
            ``GridMode`` or its name: ``"natural"``, ``"end_of_month"``
            (month/year frequency, month-end regular anchor) or
            ``"quarterly_imm"`` (whole quarters, IMM-date regular anchor).
            ``build`` raises ``ValueError`` when the frequency or the regular
            anchor (``start`` for no-stub and back-stub rules, ``end`` for
            front-stub rules) does not suit the grid.

        Returns
        -------
        ScheduleBuilder
            This builder.

        Raises
        ------
        ValueError
            If *grid* names no grid.

        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> dates = (
        ...     Schedule
        ...     .builder("2025-03-19", "2025-12-31")
        ...     .frequency("3M")
        ...     .stub_rule("short_back")
        ...     .grid("quarterly_imm")
        ...     .unadjusted()
        ...     .build()
        ...     .dates
        ... )
        >>> [d.isoformat() for d in dates]
        ['2025-03-19', '2025-06-18', '2025-09-17', '2025-12-17', '2025-12-31']
        """
        ...

    def unadjusted(self) -> ScheduleBuilder:
        """
        Keep payment dates on the raw period ends, with no calendar.

        Selects the ``unadjusted`` convention and clears any calendar; a
        positive payment or fixing lag then makes ``build`` raise, since lags
        need a calendar (use ``adjust_with("unadjusted", calendar)``).

        Returns
        -------
        ScheduleBuilder
            This builder.

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def cds_imm(self) -> ScheduleBuilder:
        """
        Use CDS IMM dates (the 20th of March, June, September and December).

        Same semantics as the Rust ``ScheduleBuilder::cds_imm``: sets the
        frequency to quarterly and the stub rule to ``short_back`` and turns
        CDS IMM mode on; it requires the natural grid. A later ``frequency`` or
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
        >>> spec = Schedule.builder("2025-01-15", "2026-01-15").grid("natural").unadjusted().cds_imm().to_spec()
        >>> spec["frequency"], spec["stub"]
        ({'count': 3, 'unit': 'months'}, 'short_back')
        """
        ...

    def to_spec(self) -> dict[str, object]:
        """
        The complete ``ScheduleSpec`` mapping these choices describe.

        Returns
        -------
        dict[str, object]
            The canonical spec fields (``start``, ``end``, ``frequency``,
            ``stub``, ``grid``, ``cds_imm_mode``, ``business_day_convention``,
            ``calendar_id``, ``payment_lag_days``,
            ``fixing_lag_business_days``), accepted by
            :meth:`Schedule.from_spec`.

        Raises
        ------
        ValueError
            If a choice is missing (the message names every missing one:
            frequency, stub, grid, payment convention) or the combination is
            inconsistent.

        Examples
        --------
        >>> from finstack_quant.core.dates import Schedule
        >>> Schedule.builder("2025-01-15", "2025-07-15").stub_rule("none").grid("natural").unadjusted().frequency(
        ...     "3M"
        ... ).to_spec()["frequency"]
        {'count': 3, 'unit': 'months'}
        """
        ...

    def build(self) -> Schedule:
        """
        Materialize the date schedule from the builder settings.

        Delegates entirely to the canonical Rust ``ScheduleBuilder::build``,
        which returns a complete schedule or an error.

        Returns
        -------
        Schedule
            The constructed schedule.

        Raises
        ------
        ValueError
            If a choice is missing, the specification is inconsistent, the
            regular anchor is off the selected grid, or the schedule cannot be
            generated.
        KeyError
            If a calendar id is unknown.
        """
        ...

    def __repr__(self) -> str: ...
