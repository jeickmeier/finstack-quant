"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/dates/tenor.rs``."""

from __future__ import annotations

import datetime
from typing import Optional, Union
from finstack_quant.core.dates._calendar import BusinessDayConvention, HolidayCalendar
from finstack_quant.core.dates._daycount import DayCount, DayCountContext

# Tenor

class TenorUnit:
    """
    Frequency/tenor unit enumeration.

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import TenorUnit
    >>> (str(TenorUnit.MONTHS), TenorUnit.from_name("M") == TenorUnit.MONTHS)
    ('M', True)

    """

    DAYS: TenorUnit
    """Day unit."""
    WEEKS: TenorUnit
    """Week unit."""
    MONTHS: TenorUnit
    """Month unit."""
    YEARS: TenorUnit
    """Year unit."""

    @classmethod
    def from_name(cls, name: str) -> TenorUnit:
        """
        Parse a single-character tenor unit designator.

        Parameters
        ----------
        name : str
            One of ``'D'``, ``'W'``, ``'M'``, ``'Y'`` (case-insensitive).

        Returns
        -------
        TenorUnit

            Day, week, month, or year unit selected by the case-insensitive
            ``D``, ``W``, ``M``, or ``Y`` designator.

        Raises
        ------
        ValueError
            If *name* is not a valid unit designator.

        Examples
        --------
        >>> from finstack_quant.core.dates import TenorUnit
        >>> TenorUnit.from_name("W") == TenorUnit.WEEKS
        True

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class Tenor:
    """
    A tenor such as ``3M``, ``1Y``, or ``2W``.

    Immutable, hashable value type combining a count and unit.

    Parameters
    ----------
    count : int
        Numeric count (e.g. ``3``).
    unit : TenorUnit
        Unit (e.g. ``TenorUnit.MONTHS``).

    Examples
    --------
    >>> from finstack_quant.core.dates import Tenor
    >>> (Tenor.parse("3M").months, Tenor.biweekly().days)
    (3, 14)

    """

    def __init__(self, count: int, unit: Union[TenorUnit, str]) -> None:
        """
        Construct a tenor from a count and a unit; ``Tenor.parse`` reads strings.

        Parameters
        ----------
        count : int
            Positive integer count.
        unit : TenorUnit | str
            Calendar unit: a ``TenorUnit`` or the one-letter designator
            ``"D"``/``"W"``/``"M"``/``"Y"``.

        Raises
        ------
        ValueError
            If *count* is zero or exceeds the representation limit for *unit*
            (days ``2**32 - 1``, weeks ``(2**32 - 1) // 7``, months
            ``2**31 - 1``, years ``(2**31 - 1) // 12``); these are not
            maturity caps.
        TypeError
            If *count* is not an ``int`` or *unit* is neither a ``TenorUnit``
            nor a unit letter.
        OverflowError
            If *count* is negative or exceeds the 32-bit range.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor, TenorUnit
        >>> Tenor.parse("3M") == Tenor(3, "M") == Tenor(3, TenorUnit.MONTHS)
        True

        """
        ...

    @classmethod
    def parse(cls, s: str) -> Tenor:
        """
        Parse a tenor string such as ``3M`` or ``10Y`` into a ``Tenor``.

        Parameters
        ----------
        s : str
            Tenor string (e.g. ``"3M"``, ``"1Y"``, ``"2W"``).

        Returns
        -------
        Tenor

            Validated tenor preserving the positive count and unit parsed from ``s``.

        Raises
        ------
        ValueError
            If *s* cannot be parsed.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.parse("3M").months
        3

        """
        ...

    @classmethod
    def daily(cls) -> Tenor:
        """
        One-calendar-day tenor.

        Returns
        -------
        Tenor
            One-calendar-day tenor.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.daily().days
        1
        """
        ...

    @classmethod
    def weekly(cls) -> Tenor:
        """
        One-week tenor, equivalent to seven calendar days.

        Returns
        -------
        Tenor
            One-week tenor, equivalent to seven calendar days.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.weekly().days
        7
        """
        ...

    @classmethod
    def biweekly(cls) -> Tenor:
        """
        Two-week tenor, equivalent to fourteen calendar days.

        Returns
        -------
        Tenor
            Two-week tenor, equivalent to fourteen calendar days.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.biweekly().days
        14
        """
        ...

    @classmethod
    def monthly(cls) -> Tenor:
        """
        One-month tenor.

        Returns
        -------
        Tenor
            One-month tenor.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.monthly().months
        1
        """
        ...

    @classmethod
    def bimonthly(cls) -> Tenor:
        """
        Two-month tenor.

        Returns
        -------
        Tenor
            Two-month tenor.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.bimonthly().months
        2
        """
        ...

    @classmethod
    def quarterly(cls) -> Tenor:
        """
        3-month (quarterly) tenor.

        Returns
        -------
        Tenor
            Three-month tenor.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.quarterly().months
        3
        """
        ...

    @classmethod
    def semi_annual(cls) -> Tenor:
        """
        6-month (semi-annual) tenor.

        Returns
        -------
        Tenor
            Six-month tenor.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.semi_annual().months
        6
        """
        ...

    @classmethod
    def annual(cls) -> Tenor:
        """
        12-month (annual) tenor.

        Returns
        -------
        Tenor
            One-year ``1Y`` tenor.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.annual().months
        12
        """
        ...

    @classmethod
    def from_payments_per_year(cls, payments: int) -> Tenor:
        """
        Construct from the number of coupon payments per year.

        Parameters
        ----------
        payments : int
            Payments per year (e.g. ``4`` for quarterly).

        Returns
        -------
        Tenor

            Month tenor with ``12 / payments`` months per coupon period.

        Raises
        ------
        ValueError
            If *payments* does not map to a standard tenor.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.from_payments_per_year(4).months
        3

        """
        ...

    @classmethod
    def from_years(cls, years: float, day_count: Union[DayCount, str]) -> Tenor:
        """
        Construct from a year fraction using a day-count convention.

        A year fraction that is (within a small epsilon) a whole number of
        months gives a month-based tenor; anything else is converted to days
        under *day_count*.

        Parameters
        ----------
        years : float
            Positive, finite length in years (``0.5`` gives ``6M``).
        day_count : DayCount | str
            Convention for the day conversion (``DayCount`` or a canonical
            name such as ``"act_365f"``).

        Returns
        -------
        Tenor
            Month tenor for whole-month fractions, otherwise a day tenor.

        Raises
        ------
        ValueError
            If *years* is non-positive, non-finite or exceeds 200 years, or
            *day_count* is not a recognised convention.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> str(Tenor.from_years(0.5, DayCount.ACT_365F))
        '6M'

        """
        ...

    def payments_per_year(self) -> float:
        """
        Nominal payments per year of a month or year tenor.

        Returns
        -------
        float
            ``12 / months`` for month tenors or ``1 / years`` for year tenors
            (``3M`` gives ``4.0``, ``5M`` gives ``2.4``, ``2Y`` gives
            ``0.5``); a nominal ratio, not a count of payments in a year.

        Raises
        ------
        ValueError
            For a day or week tenor: no 365, 252 or 52 observation
            convention is inferred.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.parse("3M").payments_per_year()
        4.0
        """
        ...

    def add_to_date(
        self,
        date: datetime.date | str,
        calendar: Union[HolidayCalendar, str, None] = None,
        *,
        business_day_convention: Union[BusinessDayConvention, str],
    ) -> datetime.date:
        """
        Add this tenor to a date with optional business-day adjustment.

        Month and year tenors clamp to the last valid day of the target month
        (Jan 31 + ``1M`` gives Feb 28/29).

        Parameters
        ----------
        date : datetime.date | str
            Anchor date (``datetime.date``, ``pandas.Timestamp`` or ISO string).
        calendar : HolidayCalendar | str | None
            Holiday calendar (object or id such as ``"usny"``; ``"nyse+gblo"``
            joins calendars). ``None`` skips adjustment.
        business_day_convention : BusinessDayConvention | str
            Roll rule applied when *calendar* is given (required keyword; there
            is no default convention); canonical names ignore surrounding
            whitespace and ASCII case.

        Returns
        -------
        datetime.date
            The (optionally adjusted) end date.

        Raises
        ------
        KeyError
            If *calendar* names an unknown calendar.
        ValueError
            If the convention string is unknown, *date* is invalid, or no
            business day exists before the end of the supported date range.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor.parse("1M").add_to_date("2025-01-31", business_day_convention="unadjusted")
        datetime.date(2025, 2, 28)

        """
        ...

    def to_years_with_context(
        self,
        as_of: datetime.date | str,
        *,
        day_count: Union[DayCount, str],
        calendar: Union[HolidayCalendar, str, None] = None,
        business_day_convention: Union[BusinessDayConvention, str],
        ctx: Optional[DayCountContext] = None,
    ) -> float:
        """
        Exact year fraction of this tenor from *as_of* under a day count.

        Adds the tenor to *as_of* (see :meth:`add_to_date`) and measures the
        span with *day_count* under the caller's *ctx*, so calendars and roll
        conventions are honoured, unlike the fixed approximation in
        :meth:`to_years_approx`. No coupon frequency or period is invented
        from the tenor.

        Parameters
        ----------
        as_of : datetime.date | str
            Start date of the measurement.
        day_count : DayCount | str
            Convention used to measure the span.
        calendar : HolidayCalendar | str | None
            Holiday calendar for the end-date roll; ``None`` skips adjustment.
        business_day_convention : BusinessDayConvention | str
            Roll rule for the end date.
        ctx : DayCountContext | None
            Day-count context (frequency, coupon period, calendar, termination
            status) for conventions that need one; ``None`` is the empty
            context.

        Returns
        -------
        float
            Year fraction between *as_of* and the rolled end date.

        Raises
        ------
        KeyError
            If *calendar* names an unknown calendar.
        ValueError
            If *day_count* or the convention is unrecognised, the convention
            needs context that *ctx* lacks, or the rolled end precedes
            *as_of*.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> Tenor.parse("1Y").to_years_with_context(
        ...     "2025-01-15", day_count=DayCount.ACT_ACT, business_day_convention="unadjusted"
        ... )
        1.0

        """
        ...

    @property
    def count(self) -> int:
        """
        Positive integer multiplying this tenor's calendar unit.

        Returns
        -------
        int
            Count of days, weeks, months, or years represented by this tenor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def unit(self) -> TenorUnit:
        """
        Calendar unit of this tenor (days, weeks, months, or years).

        Returns
        -------
        TenorUnit
            Calendar unit of this tenor (days, weeks, months, or years).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def months(self) -> Optional[int]:
        """
        Equivalent whole months (``None`` for day/week tenors).

        Returns
        -------
        int | None
            Equivalent whole months (``None`` for day/week tenors).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def days(self) -> Optional[int]:
        """
        Equivalent whole days (``None`` for month/year tenors).

        Returns
        -------
        int | None
            Equivalent whole days (``None`` for month/year tenors).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_years_approx(self) -> float:
        """
        Approximate tenor length in years (simple estimate, no calendar).

        Returns
        -------
        float
            The declared approximation: days divided by 365, weeks times seven
            divided by 365, months divided by 12, or the stored year count. It
            is not an actual dated accrual.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def to_days_approx(self) -> int:
        """
        Approximate tenor length in calendar days.

        Returns
        -------
        int
            Exact integer approximation: ``n`` days, ``7n`` for weeks,
            ``(365n + 6) // 12`` for months (half-day ties round up, so
            ``6M`` gives 183) and ``365n`` for years.

        Notes
        -----
        This method does not raise; it returns the stored or derived value.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
