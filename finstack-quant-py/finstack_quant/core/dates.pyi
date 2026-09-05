"""
Date, calendar, and schedule utilities from ``finstack-quant-core``.

Provides day-count conventions, tenor types, period generation, schedule
building, holiday calendars, and business-day adjustment functions.

Example::

    >>> import datetime
    >>> from finstack_quant.core.dates import DayCount, Schedule, Tenor
    >>> day_count = DayCount.ACT_365F
    >>> day_count.year_fraction(datetime.date(2024, 1, 1), datetime.date(2025, 1, 1))
    1.0027397260273974

Examples
--------
>>> from finstack_quant.core.dates import Tenor
>>> Tenor.parse("3M").months
3

"""

from __future__ import annotations

import datetime
from typing import Iterator, Optional, Sequence, Union

import pandas as pd

__all__ = [
    # day-count
    "DayCount",
    "DayCountContext",
    "Thirty360Convention",
    "days_30_360",
    "days_30e_360_isda",
    # tenor
    "TenorUnit",
    "Tenor",
    # periods
    "PeriodKind",
    "PeriodId",
    "Period",
    "PeriodPlan",
    "FiscalConfig",
    "build_periods",
    "build_fiscal_periods",
    # calendar
    "BusinessDayConvention",
    "CalendarMetadata",
    "HolidayCalendar",
    "adjust",
    "available_calendars",
    # schedule
    "StubKind",
    "ScheduleErrorPolicy",
    "Schedule",
    "ScheduleBuilder",
    # SIFMA settlements
    "SifmaSettlementClass",
    "sifma_settlement_date",
    "sifma_settlement_date_for_class",
    "estimated_sifma_settlement_date_for_class",
    "next_sifma_settlement",
    # IMM, CDS rolls, and listed-option expiries
    "third_wednesday",
    "third_friday",
    "next_imm",
    "is_imm_date",
    "is_cds_date",
    "next_cds_date",
    "prev_cds_date",
    "prev_cds_semiannual_roll",
    "next_semiannual_cds_maturity",
    "imm_option_expiry",
    "next_imm_option_expiry",
    "next_equity_option_expiry",
    # free functions
    "create_date",
    "days_since_epoch",
    "date_from_epoch_days",
    # date extensions
    "add_business_days",
    "add_months",
    "add_weekdays",
    "end_of_month",
    "fiscal_year",
    "is_weekend",
    "months_until",
    "quarter",
]

class SifmaSettlementClass:
    """
    SIFMA good-delivery settlement class.

    Examples
    --------
    >>> from finstack_quant.core.dates import SifmaSettlementClass
    >>> SifmaSettlementClass.from_agency_term("FNMA", 30) == SifmaSettlementClass.A
    True

    """

    A: SifmaSettlementClass
    """Class A: 30-year conventional (FNMA/FHLMC) and UMBS pools."""
    B: SifmaSettlementClass
    """Class B: 15-year fixed-rate agency pools."""
    C: SifmaSettlementClass
    """Class C: 30-year GNMA single-family pools."""
    D: SifmaSettlementClass
    """Class D: balloons, ARMs, multifamily and other non-standard programs."""

    @classmethod
    def from_agency_term(cls, agency: str, term_years: int) -> SifmaSettlementClass:
        """
        Infer the SIFMA good-delivery class from an agency and term.

        Parameters
        ----------
        agency : str
            Agency or program label, such as ``"FNMA"``, ``"FHLMC"``, or
            ``"GNMA"``, interpreted using the library's settlement table.
        term_years : int
            Original mortgage term in whole years, normally ``15`` or ``30``.

        Returns
        -------
        SifmaSettlementClass
            Settlement class used to select the monthly SIFMA delivery date.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import SifmaSettlementClass
        >>> SifmaSettlementClass.from_agency_term("FNMA", 30) == SifmaSettlementClass.A
        True
        """
        ...

def sifma_settlement_date(month: int, year: int) -> datetime.date | None:
    """
    Return the published SIFMA settlement date for a month when available.

    Parameters
    ----------
    month : int
        Delivery month number from ``1`` through ``12``.
    year : int
        Four-digit delivery calendar year.

    Returns
    -------
    datetime.date or None
        Published settlement date, or ``None`` when the month is not listed.

    Raises
    ------
    ValueError
        If *month* is outside ``1`` through ``12``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import sifma_settlement_date
    >>> sifma_settlement_date(1, 2026)
    datetime.date(2026, 1, 14)

    """
    ...

def sifma_settlement_date_for_class(
    month: int, year: int, settlement_class: SifmaSettlementClass
) -> datetime.date | None:
    """
    Return the SIFMA settlement date for a specified delivery class.

    Parameters
    ----------
    month : int
        Delivery month number from ``1`` through ``12``.
    year : int
        Four-digit delivery calendar year.
    settlement_class : SifmaSettlementClass
        Good-delivery class inferred from the agency/program and mortgage term.

    Returns
    -------
    datetime.date | None
        The exact published settlement date for the requested month, year,
        and settlement class, or ``None`` when the embedded SIFMA calendar
        has no date for that combination.

    Raises
    ------
    ValueError
        If *month* is outside ``1`` through ``12``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import sifma_settlement_date_for_class, SifmaSettlementClass
    >>> sifma_settlement_date_for_class(1, 2026, SifmaSettlementClass.B)
    datetime.date(2026, 1, 20)

    """
    ...

def estimated_sifma_settlement_date_for_class(
    month: int, year: int, settlement_class: SifmaSettlementClass
) -> datetime.date:
    """
    Estimate a class-specific SIFMA settlement date when no calendar is published.

    Parameters
    ----------
    month : int
        Delivery month number from ``1`` through ``12``.
    year : int
        Four-digit delivery calendar year.
    settlement_class : SifmaSettlementClass
        Good-delivery class whose conventional estimated date is required.

    Returns
    -------
    datetime.date
        Deterministic estimated settlement date for the requested class.

    Raises
    ------
    ValueError
        If *month* is outside ``1`` through ``12``.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import estimated_sifma_settlement_date_for_class, SifmaSettlementClass
    >>> estimated_sifma_settlement_date_for_class(1, 2030, SifmaSettlementClass.C)
    datetime.date(2030, 1, 22)

    """
    ...

def next_sifma_settlement(date: datetime.date | str) -> datetime.date | None:
    """
    Return the next published SIFMA settlement date on or after a date.

    Parameters
    ----------
    date : datetime.date | str
        Calendar date from which to search the published settlement calendar.

    Returns
    -------
    datetime.date or None
        Earliest available settlement date not before ``date``, or ``None``.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_sifma_settlement
    >>> next_sifma_settlement(datetime.date(2026, 1, 15))
    datetime.date(2026, 2, 12)

    """
    ...

# Day-count conventions

class DayCount:
    """
    Day-count convention for year-fraction calculations.

    Immutable, hashable enum-style type with class attributes for each
    supported convention.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import DayCount
    >>> day_count = DayCount.ACT_360
    >>> day_count.year_fraction(datetime.date(2024, 1, 1), datetime.date(2024, 7, 1))
    0.5055555555555555
    """

    ACT_360: DayCount
    """Actual/360 (money market)."""
    ACT_365F: DayCount
    """Actual/365 Fixed."""
    ACT_365L: DayCount
    """Actual/365L (ICMA Rule 251).

    Annual periods (or periods without a supplied frequency) use denominator
    366 exactly when February 29 falls in ``(start, end]``; otherwise 365.
    Non-annual periods use 366 exactly when the end date's year is a leap year;
    otherwise 365. This is explicitly not ACT/ACT AFB, which uses a different
    sub-period-splitting algorithm.
    """
    NL_365: DayCount
    """NL/365 (Actual/365 No Leap): actual days excluding every February 29
    in ``(start, end]``, divided by 365."""
    THIRTY_360: DayCount
    """30/360 US (Bond Basis)."""
    THIRTY_E_360: DayCount
    """30E/360 (Eurobond Basis)."""
    THIRTY_E_360_ISDA: DayCount
    """30E/360 ISDA."""
    ACT_ACT: DayCount
    """Actual/Actual (ISDA)."""
    ACT_ACT_ISMA: DayCount
    """Actual/Actual (ICMA/ISMA)."""
    ACT_ACT_AFB: DayCount
    """Actual/Actual AFB (Actual/Actual Euro).

    Walks whole years backwards from the end date (QuantLib
    ``ActualActual::AFB``). A year-step landing on 28 February of a leap
    year is bumped to 29 February. The residual uses denominator 366 if
    29 February lies in ``[start, residual_end)``, else 365. No context
    is required.
    """
    THIRTY_360_IT: DayCount
    """30/360 Italian.

    Day 31 becomes 30, and any February day after the 27th becomes 30
    (QuantLib ``Thirty360::Italian``). Distinct from US SIA and 30E/360.
    """
    BUS_252: DayCount
    """Business/252 (Brazilian market convention)."""

    @classmethod
    def from_name(cls, name: str) -> DayCount:
        """
        Parse a day-count convention from its string name.

        Parameters
        ----------
        name : str
            Convention identifier (e.g. ``"act_360"``, ``"act_365f"``,
            ``"30_360"``, ``"act_act_afb"``, ``"30_360_it"``, ``"bus_252"``).

        Returns
        -------
        DayCount

            Day-count convention corresponding to the exact canonical lowercase name.

        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount
        >>> DayCount.from_name("act_360") == DayCount.ACT_360
        True
        >>> DayCount.from_name("act_act_afb") == DayCount.ACT_ACT_AFB
        True
        >>> DayCount.from_name("30_360_it") == DayCount.THIRTY_360_IT
        True

        """
        ...

    @classmethod
    def parse(cls, s: str) -> DayCount:
        """
        Leniently parse a day-count label as written on term sheets.

        Case-insensitive; ``/``, ``-`` and spaces are treated as ``_``, and
        the market spellings ``"ACT/ACT ICMA"``, ``"ACT/ACT ISDA"``,
        ``"ACT/365"``, ``"30/360"`` and ``"30E/360 ISDA"`` are recognised.
        Use :meth:`from_name` for strict canonical names.

        Parameters
        ----------
        s : str
            Day-count label such as ``"ACT/360"``, ``"Act/Act ICMA"`` or
            any canonical snake_case name.

        Returns
        -------
        DayCount
            The matching convention.

        Raises
        ------
        ValueError
            If no spelling matches; the message lists the canonical names.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount
        >>> DayCount.parse("ACT/ACT ICMA") == DayCount.ACT_ACT_ISMA
        True

        """
        ...

    def year_fraction(
        self,
        start: datetime.date | str,
        end: datetime.date | str,
        ctx: Optional[DayCountContext] = None,
        *,
        frequency: Union[Tenor, str, None] = None,
        calendar: Union[HolidayCalendar, str, None] = None,
    ) -> float:
        """
        Compute the year fraction between two dates.

        Parameters
        ----------
        start : datetime.date | str
            Accrual start (inclusive); ``datetime.date``, ``pandas.Timestamp``
            or ISO ``YYYY-MM-DD`` string.
        end : datetime.date | str
            Accrual end (exclusive); must not precede *start*.
        ctx : DayCountContext | None
            Full context object (calendar, frequency, coupon period, ...).
            Mutually exclusive with *frequency* / *calendar*.
        frequency : Tenor | str | None
            Coupon frequency for ``ACT_ACT_ISMA`` / ``ACT_365L`` (e.g. ``"6M"``).
        calendar : HolidayCalendar | str | None
            Holiday calendar (object or id) required by ``BUS_252``.

        Returns
        -------
        float
            Non-negative year fraction (``0.0`` when ``start == end``).

        Raises
        ------
        ValueError
            If *start* > *end*, both *ctx* and keywords are given, or the
            convention needs context that was not supplied.
        KeyError
            If *calendar* names an unknown calendar.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount
        >>> DayCount.ACT_ACT_ISMA.year_fraction("2025-01-15", "2025-07-15", frequency="6M")
        0.5

        """
        ...

    def signed_year_fraction(
        self,
        start: datetime.date | str,
        end: datetime.date | str,
        ctx: Optional[DayCountContext] = None,
        *,
        frequency: Union[Tenor, str, None] = None,
        calendar: Union[HolidayCalendar, str, None] = None,
    ) -> float:
        """
        Compute the signed year fraction (negative when start > end).

        Parameters
        ----------
        start : datetime.date | str
            Start date.
        end : datetime.date | str
            End date; may precede *start*.
        ctx : DayCountContext | None
            Full context object; mutually exclusive with the keywords.
        frequency : Tenor | str | None
            Coupon frequency for frequency-dependent conventions.
        calendar : HolidayCalendar | str | None
            Holiday calendar required by ``BUS_252``.

        Returns
        -------
        float
            Signed year fraction.

        Raises
        ------
        ValueError
            If required context is missing or both *ctx* and keywords are given.
        KeyError
            If *calendar* names an unknown calendar.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount
        >>> DayCount.ACT_360.signed_year_fraction("2024-07-01", "2024-01-01")
        -0.5055555555555555

        """
        ...

    @staticmethod
    def calendar_days(start: datetime.date | str, end: datetime.date | str) -> int:
        """
        Count the calendar days between two dates.

        Parameters
        ----------
        start : datetime.date | str
            Start date.
        end : datetime.date | str
            End date.

        Returns
        -------
        int
            Signed number of calendar days (end - start).

        Raises
        ------
        TypeError
            If *start* or *end* is not date-like (``datetime.date``,
            ``datetime.datetime``, or ``pandas.Timestamp``).
        ValueError
            If the year/month/day attributes do not form a valid calendar date.

        Examples
        --------
        >>> import datetime
        >>> from finstack_quant.core.dates import DayCount
        >>> DayCount.calendar_days(datetime.date(2024, 1, 1), datetime.date(2024, 1, 31))
        30
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class DayCountContext:
    """
    Optional context for day-count calculations.

    Certain conventions require additional information:

    - **Bus/252** requires a holiday calendar (resolved by ``calendar_id``).
    - **Act/Act (ISMA)** requires the coupon ``frequency`` and, for
      irregular or mid-coupon accruals, the reference ``coupon_period``.
    - **30E/360 ISDA** uses ``end_is_termination_date`` for its
      end-of-February rule.

    Equality is structural, and instances round-trip through
    :meth:`to_json` / :meth:`from_json` and ``pickle``.

    Parameters
    ----------
    calendar_id : str | None
        Registered calendar id (e.g. ``"target2"``; ``"nyse+gblo"`` joins
        calendars). Resolved on each use, so an unknown id raises
        ``KeyError`` at calculation time.
    frequency : Tenor | str | None
        Coupon frequency for ISMA conventions (``Tenor`` or ``"6M"``).
    bus_basis : int | None
        Custom business-day divisor (defaults to 252 when omitted).
    coupon_period : tuple[datetime.date | str, datetime.date | str] | None
        Reference coupon period ``(start, end)``; ``start`` must precede ``end``.
    end_is_termination_date : bool
        Whether the accrual end is the instrument termination date.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCountContext
    >>> context = DayCountContext("usny", "3M", 252)
    >>> (context.calendar_id, context.frequency.months, context.bus_basis, context.calendar_id)
    ('usny', 3, 252, 'usny')

    """

    def __init__(
        self,
        calendar_id: Optional[str] = None,
        frequency: Union[Tenor, str, None] = None,
        bus_basis: Optional[int] = None,
        coupon_period: Optional[tuple[datetime.date | str, datetime.date | str]] = None,
        end_is_termination_date: bool = False,
    ) -> None:
        """
        Create a day-count context.

        Parameters
        ----------
        calendar_id : str | None
            Registered calendar id; not resolved until a calculation runs.
        frequency : Tenor | str | None
            Coupon frequency (``Tenor`` or tenor string such as ``"6M"``).
        bus_basis : int | None
            Custom business-day divisor for Bus/252.
        coupon_period : tuple[datetime.date | str, datetime.date | str] | None
            Reference coupon period ``(start, end)`` for ACT/ACT (ICMA).
        end_is_termination_date : bool
            Whether the accrual end is the instrument termination date.

        Raises
        ------
        ValueError
            If *coupon_period* is supplied and its start is not before its
            end (validated in Rust), or *frequency* does not parse.

        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form (strict field names).

        Returns
        -------
        str
            JSON object with ``calendar_id``, ``frequency``, ``bus_basis``,
            ``coupon_period`` (ISO date pair or ``null``) and
            ``end_is_termination_date``.

        Raises
        ------
        ValueError
            If the context cannot be serialized.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCountContext
        >>> DayCountContext.from_json(DayCountContext("usny").to_json()).calendar_id
        'usny'
        """
        ...

    @staticmethod
    def from_json(json: str) -> DayCountContext:
        """
        Deserialize from the canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        DayCountContext
            The reconstructed context.

        Raises
        ------
        ValueError
            If *json* is malformed, has unknown fields, or carries an
            inverted ``coupon_period``.
        Examples
        --------
        >>> from finstack_quant.core.dates import DayCountContext
        >>> DayCountContext.from_json(DayCountContext("usny").to_json()).calendar_id
        'usny'

        """
        ...

    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    @property
    def calendar_id(self) -> Optional[str]:
        """
        Optional calendar identifier.

        Returns
        -------
        str | None
            Optional calendar identifier.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def frequency(self) -> Optional[Tenor]:
        """
        Optional coupon frequency.

        Returns
        -------
        Tenor | None
            Optional coupon frequency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def bus_basis(self) -> Optional[int]:
        """
        Optional custom business-day divisor.

        Returns
        -------
        int | None
            Optional custom business-day divisor.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def coupon_period(self) -> Optional[tuple[datetime.date, datetime.date]]:
        """
        Optional reference coupon period as ``(start, end)`` dates.

        Returns
        -------
        tuple[datetime.date, datetime.date] | None

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def end_is_termination_date(self) -> bool:
        """
        Whether the accrual end is the instrument termination date.

        Returns
        -------
        bool
            Whether the accrual end is the instrument termination date.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...

class Thirty360Convention:
    """
    30/360 sub-convention (US SIA / Bond Basis, ISDA, or European).

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import Thirty360Convention
    >>> str(Thirty360Convention.US_SIA)
    'us_sia'

    """

    US_SIA: Thirty360Convention
    """US 30/360 SIA / Bond Basis convention."""
    ISDA: Thirty360Convention
    """30/360 ISDA convention."""
    EUROPEAN: Thirty360Convention
    """European 30E/360 convention."""
    ITALIAN: Thirty360Convention
    """30/360 Italian convention (31→30 and February day after 27→30)."""

    @classmethod
    def from_name(cls, name: str) -> Thirty360Convention:
        """
        Parse this variant from its snake_case name, case-insensitively.

        Parameters
        ----------
        name : str
            One of ``"us_sia"``, ``"isda"``, ``"european"``, ``"italian"``.

        Returns
        -------
        Thirty360Convention
            The matching variant.

        Raises
        ------
        ValueError
            If *name* is not one of the four variant names.

        Examples
        --------
        >>> from finstack_quant.core.dates import Thirty360Convention
        >>> Thirty360Convention.from_name("ISDA") == Thirty360Convention.ISDA
        True

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

def days_30_360(
    start: datetime.date | str,
    end: datetime.date | str,
    convention: Union[Thirty360Convention, str],
) -> int:
    """
    30/360 day count between *start* (inclusive) and *end* (exclusive).

    Parameters
    ----------
    start : datetime.date | str
        Accrual start.
    end : datetime.date | str
        Accrual end; an earlier *end* gives a negative count.
    convention : Thirty360Convention | str
        Variant governing the month-end and February rules
        (``"us_sia"``, ``"isda"``, ``"european"``, ``"italian"``).

    Returns
    -------
    int
        Signed 30/360 day count (divide by 360 for the year fraction).

    Raises
    ------
    ValueError
        If a date or the convention name is invalid.
    TypeError
        If an argument has an unsupported type.

    Examples
    --------
    >>> from finstack_quant.core.dates import days_30_360
    >>> days_30_360("2025-01-31", "2025-03-31", "isda")
    60

    """
    ...

def days_30e_360_isda(
    start: datetime.date | str,
    end: datetime.date | str,
    end_is_termination_date: bool,
) -> int:
    """
    30E/360 ISDA day count between *start* (inclusive) and *end* (exclusive).

    Parameters
    ----------
    start : datetime.date | str
        Accrual start.
    end : datetime.date | str
        Accrual end; an earlier *end* gives a negative count.
    end_is_termination_date : bool
        Whether *end* is the instrument termination date, which keeps a
        February month-end at its actual day (ISDA 2006 §4.16(h)).

    Returns
    -------
    int
        Signed 30E/360 ISDA day count.

    Raises
    ------
    ValueError
        If a date is invalid.
    TypeError
        If a date argument has an unsupported type.

    Examples
    --------
    >>> from finstack_quant.core.dates import days_30e_360_isda
    >>> days_30e_360_isda("2024-01-31", "2024-02-29", False)
    30

    """
    ...

# Tenor

class TenorUnit:
    """
    Frequency/tenor unit enumeration.

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import TenorUnit
    >>> (str(TenorUnit.MONTHS), TenorUnit.from_char("M") == TenorUnit.MONTHS)
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
    def from_char(cls, ch: str) -> TenorUnit:
        """
        Parse a single-character tenor unit designator.

        Parameters
        ----------
        ch : str
            One of ``'D'``, ``'W'``, ``'M'``, ``'Y'`` (case-sensitive).

        Returns
        -------
        TenorUnit

            Day, week, month, or year unit selected by the case-insensitive
            ``D``, ``W``, ``M``, or ``Y`` designator.

        Raises
        ------
        ValueError
            If *ch* is not a valid unit designator.

        Examples
        --------
        >>> from finstack_quant.core.dates import TenorUnit
        >>> TenorUnit.from_char("W") == TenorUnit.WEEKS
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

    def __init__(self, value: Union[str, int], unit: Union[TenorUnit, str, None] = None) -> None:
        """
        Construct a tenor from a string, or from a count and unit.

        Parameters
        ----------
        value : str | int
            Tenor string such as ``"3M"`` (money-market aliases ``"ON"``,
            ``"TN"``, ``"SN"`` give ``1D``), or the positive integer count
            when *unit* is given.
        unit : TenorUnit | str | None
            Calendar unit for an integer *value*: a ``TenorUnit`` or the
            one-letter designator ``"D"``/``"W"``/``"M"``/``"Y"``.

        Raises
        ------
        ValueError
            If the string does not parse, *count* is zero, or the count
            exceeds the supported range for *unit* (200 years).
        TypeError
            If *value* is neither ``str`` nor ``int``, *unit* is given with a
            string *value*, or *unit* is missing for an integer *value*.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor, TenorUnit
        >>> Tenor("3M") == Tenor(3, "M") == Tenor(3, TenorUnit.MONTHS)
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
        Coupon payments per year implied by this tenor.

        Returns
        -------
        float
            ``12 / months`` for month tenors, ``52 / weeks``, ``365 / days`` or
            ``1 / years`` (``3M`` gives ``4.0``, ``2Y`` gives ``0.5``).

        Notes
        -----
        This method does not raise; it returns the derived value.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor("3M").payments_per_year()
        4.0
        """
        ...

    def add_to_date(
        self,
        date: datetime.date | str,
        calendar: Union[HolidayCalendar, str, None] = None,
        business_day_convention: Union[BusinessDayConvention, str] = "modified_following",
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
            Roll rule applied when *calendar* is given; short codes
            ``MF``/``F``/``P``/``MP`` accepted.

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
            business day is found within 100 days.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> Tenor("1M").add_to_date("2025-01-31")
        datetime.date(2025, 2, 28)

        """
        ...

    def to_years_with_context(
        self,
        as_of: datetime.date | str,
        *,
        day_count: Union[DayCount, str],
        calendar: Union[HolidayCalendar, str, None] = None,
        business_day_convention: Union[BusinessDayConvention, str] = "modified_following",
    ) -> float:
        """
        Exact year fraction of this tenor from *as_of* under a day count.

        Adds the tenor to *as_of* (see :meth:`add_to_date`) and measures the
        span with *day_count*, so calendars and roll conventions are honoured,
        unlike the fixed approximation in :meth:`to_years`.

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

        Returns
        -------
        float
            Year fraction between *as_of* and the rolled end date.

        Raises
        ------
        KeyError
            If *calendar* names an unknown calendar.
        ValueError
            If *day_count* or the convention is unrecognised, or the
            convention needs context that was not supplied.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount, Tenor
        >>> Tenor("1Y").to_years_with_context("2025-01-15", day_count=DayCount.ACT_ACT)
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
        Calendar unit of this tenor (days, months, or years).

        Returns
        -------
        TenorUnit
            Calendar unit of this tenor (days, months, or years).

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

    def to_years(self) -> float:
        """
        Approximate tenor length in years (simple estimate, no calendar).

        Returns
        -------
        float
            Approximate years using days divided by 365, weeks multiplied by
            seven and divided by 365, months divided by 12, or the stored year
            count.

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
            Nearest whole-day length using seven-day weeks, ``365 / 12`` days
            per month, and 365-day years.

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

# Periods

class PeriodKind:
    """
    Frequency kind used to label schedule periods (month, quarter, year).

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import PeriodKind
    >>> (str(PeriodKind.QUARTERLY), PeriodKind.QUARTERLY.periods_per_year)
    ('quarterly', 4)

    """

    DAILY: PeriodKind
    """Daily periods (252 trading days per year)."""
    WEEKLY: PeriodKind
    """Weekly periods."""
    MONTHLY: PeriodKind
    """Monthly periods."""
    QUARTERLY: PeriodKind
    """Quarterly periods."""
    SEMI_ANNUAL: PeriodKind
    """Semi-annual periods."""
    ANNUAL: PeriodKind
    """Annual periods."""

    @classmethod
    def from_name(cls, name: str) -> PeriodKind:
        """
        Parse a period kind from a string.

        Parameters
        ----------
        name : str
            Exact canonical period-kind name, such as ``"quarterly"`` or ``"annual"``.

        Returns
        -------
        PeriodKind

            Canonical frequency represented by the exact canonical lowercase name.

        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodKind
        >>> PeriodKind.from_name("quarterly") == PeriodKind.QUARTERLY
        True

        """
        ...

    @property
    def periods_per_year(self) -> int:
        """
        Number of periods per year for this frequency.

        Returns
        -------
        int
            Number of periods per year for this frequency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def annualization_factor(self) -> float:
        """
        Annualization factor for this frequency.

        Returns
        -------
        float
            Annualization factor for this frequency.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def prior_observation_date(self, first: datetime.date | str) -> datetime.date:
        """
        Observation date one period before *first*.

        Daily and weekly step back 1 and 7 calendar days; monthly, quarterly,
        semi-annual and annual step back 1, 3, 6 and 12 months with month-end
        clamping.

        Parameters
        ----------
        first : datetime.date | str
            First observation date of the series.

        Returns
        -------
        datetime.date
            The prior observation date.

        Raises
        ------
        ValueError
            If *first* is not a valid calendar date or ISO string.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodKind
        >>> PeriodKind.QUARTERLY.prior_observation_date("2025-03-31")
        datetime.date(2024, 12, 31)

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class PeriodId:
    """
    A period identifier such as ``2025Q1`` or ``2025M03``.

    Immutable, hashable value type.

    Examples
    --------
    >>> from finstack_quant.core.dates import PeriodId
    >>> (PeriodId.parse("2025Q2").code, PeriodId.parse("2025Q2").next().code)
    ('2025Q2', '2025Q3')

    """

    @classmethod
    def parse(cls, code: str) -> PeriodId:
        """
        Parse a period code string.

        Parameters
        ----------
        code : str
            Period code such as ``"2025Q1"`` or fiscal ``"FY2025W53"``.
            Unmarked weekly identifiers remain strict ISO week-year values.

        Returns
        -------
        PeriodId

            Validated calendar or ``FY`` period encoded by ``code``.

        Raises
        ------
        ValueError
            If *code* cannot be parsed.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> PeriodId.parse("2025Q2").code
        '2025Q2'

        """
        ...

    @classmethod
    def month(cls, year: int, month: int) -> PeriodId:
        """
        Build a monthly period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        month : int
            Calendar month number from ``1`` through ``12`` for the period.

        Returns
        -------
        PeriodId
            Calendar-month identifier formatted as ``YYYYMmm``.

        Raises
        ------
        ValueError
            If *month* is outside ``1`` through ``12``.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> PeriodId.month(2025, 2).code
        '2025M02'

        """
        ...

    @classmethod
    def quarter(cls, year: int, quarter: int) -> PeriodId:
        """
        Build a quarterly period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        quarter : int
            Quarter (1-4).

        Returns
        -------
        PeriodId
            Calendar-quarter identifier formatted as ``YYYYQq``.

        Raises
        ------
        ValueError
            If *quarter* is outside ``1`` through ``4``.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> PeriodId.quarter(2025, 2).code
        '2025Q2'

        """
        ...

    @classmethod
    def annual(cls, year: int) -> PeriodId:
        """
        Build an annual period identifier.

        Parameters
        ----------
        year : int
            Calendar year.

        Returns
        -------
        PeriodId
            Calendar-year identifier formatted as ``YYYY``.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> PeriodId.annual(2025).code
        '2025'
        """
        ...

    @classmethod
    def half(cls, year: int, half: int) -> PeriodId:
        """
        Build a semi-annual period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        half : int
            Half (1 or 2).

        Returns
        -------
        PeriodId
            Calendar-half identifier formatted as ``YYYYHh``.

        Raises
        ------
        ValueError
            If *half* is not ``1`` or ``2``.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> PeriodId.half(2025, 2).code
        '2025H2'

        """
        ...

    @classmethod
    def week(cls, year: int, week: int) -> PeriodId:
        """
        Build a weekly period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        week : int
            ISO week number (1-53).

        Returns
        -------
        PeriodId
            ISO week-year identifier formatted as ``YYYYWww``.

        Raises
        ------
        ValueError
            If *week* is not a valid ISO week number for *year*.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> PeriodId.week(2025, 2).code
        '2025W02'

        """
        ...

    @classmethod
    def day(cls, year: int, ordinal: int) -> PeriodId:
        """
        Build a daily period identifier from an ordinal day.

        Parameters
        ----------
        year : int
            Calendar year.
        ordinal : int
            Ordinal day of the year (1-366).

        Returns
        -------
        PeriodId
            Calendar ordinal-day identifier formatted as ``YYYYDddd``.

        Raises
        ------
        ValueError
            If *ordinal* is not a valid day of *year*.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> PeriodId.day(2025, 2).code
        '2025D002'

        """
        ...

    @property
    def code(self) -> str:
        """
        Period code string (e.g. ``"2025Q1"``).

        Returns
        -------
        str
            Period code string (e.g. ``"2025Q1"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def year(self) -> int:
        """
        Gregorian or fiscal year label.

        Returns
        -------
        int
            Gregorian or fiscal year label.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def index(self) -> int:
        """
        Ordinal index within the year.

        Returns
        -------
        int
            Ordinal index within the year.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def kind(self) -> PeriodKind:
        """
        Kind (frequency) of this period.

        Returns
        -------
        PeriodKind
            Kind (frequency) of this period.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def is_fiscal(self) -> bool:
        """
        Whether this identifier uses fiscal-year (``FY...``) semantics.

        Returns
        -------
        bool
            Whether fiscal holds for this `PeriodId`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def periods_per_year(self) -> int:
        """
        Number of periods per year for this kind.

        Returns
        -------
        int
            Number of periods per year for this kind.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def next(self) -> PeriodId:
        """
        Next period in sequence.

        Returns
        -------
        PeriodId

            Following non-fiscal period of the same frequency, with year rollover.

        Raises
        ------
        ValueError
            If the identifier is fiscal. Use :meth:`next_fiscal` with an
            explicit :class:`FiscalConfig`.
        """
        ...

    def prev(self) -> PeriodId:
        """
        Previous period in sequence.

        Returns
        -------
        PeriodId

            Preceding non-fiscal period of the same frequency, with year rollover.

        Raises
        ------
        ValueError
            If the identifier is fiscal. Use :meth:`prev_fiscal` with an
            explicit :class:`FiscalConfig`.
        """
        ...

    def next_fiscal(self, fiscal_config: FiscalConfig) -> PeriodId:
        """
        Next period using fiscal-year week/day capacity.

        Weekly fiscal IDs can advance through a partial week 53 even when the
        same-numbered ISO Gregorian year has only 52 weeks.

        Parameters
        ----------
        fiscal_config : FiscalConfig
            Fiscal-year start month and day used to determine the next fiscal
            week, month, quarter, or year boundary.

        Returns
        -------
        PeriodId
            Following fiscal period of the same kind, with week and day
            capacity determined by ``fiscal_config``.

        Raises
        ------
        ValueError
            If *fiscal_config* does not define a valid fiscal-year start date,
            or the next fiscal boundary is outside the supported date range.

        """
        ...

    def prev_fiscal(self, fiscal_config: FiscalConfig) -> PeriodId:
        """
        Previous period using fiscal-year week/day capacity.

        Parameters
        ----------
        fiscal_config : FiscalConfig
            Fiscal-year start month and day used to determine the preceding
            fiscal week, month, quarter, or year boundary.

        Returns
        -------
        PeriodId
            Preceding fiscal period of the same kind, with week and day
            capacity determined by ``fiscal_config``.

        Raises
        ------
        ValueError
            If *fiscal_config* does not define a valid fiscal-year start date,
            or the preceding fiscal boundary is outside the supported date range.

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...
    def __lt__(self, other: PeriodId) -> bool: ...
    def __le__(self, other: PeriodId) -> bool: ...
    def __gt__(self, other: PeriodId) -> bool: ...
    def __ge__(self, other: PeriodId) -> bool: ...

class Period:
    """
    A concrete period with start/end dates and an actual/forecast flag.

    Immutable value type returned by period-building functions.

    Examples
    --------
    >>> from finstack_quant.core.dates import build_periods
    >>> period = build_periods("2024Q1..Q1").periods[0]
    >>> (period.id.code, period.start, period.end)
    ('2024Q1', datetime.date(2024, 1, 1), datetime.date(2024, 4, 1))

    """

    @property
    def id(self) -> PeriodId:
        """
        Stable string identifier for this schedule period.

        Returns
        -------
        PeriodId
            Stable string identifier for this schedule period.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def start(self) -> datetime.date:
        """
        First date included in this schedule period.

        Returns
        -------
        datetime.date
            First date included in this schedule period.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def end(self) -> datetime.date:
        """
        First date after the period; the period does not include it.

        Returns
        -------
        datetime.date
            First date after the period; the period does not include it.

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def is_actual(self) -> bool:
        """
        Whether this period is an actual (vs forecast).

        Returns
        -------
        bool
            Whether actual holds for this `Period`.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            JSON object with ``id``, ``start``, ``end`` (ISO dates) and
            ``is_actual``.

        Raises
        ------
        ValueError
            If the period cannot be serialized.

        Examples
        --------
        >>> from finstack_quant.core.dates import Period, build_periods
        >>> period = build_periods("2024Q1..Q1").periods[0]
        >>> Period.from_json(period.to_json()) == period
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> Period:
        """
        Deserialize from the canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        Period
            The reconstructed period.

        Raises
        ------
        ValueError
            If *json* is malformed.
        Examples
        --------
        >>> from finstack_quant.core.dates import Period, build_periods
        >>> period = build_periods("2024Q1..Q1").periods[0]
        >>> Period.from_json(period.to_json()) == period
        True

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class PeriodPlan:
    """
    A plan containing a contiguous sequence of periods.

    Returned by :func:`build_periods` and :func:`build_fiscal_periods`.

    Examples
    --------
    >>> from finstack_quant.core.dates import build_periods
    >>> [period.id.code for period in build_periods("2024Q1..Q2").periods]
    ['2024Q1', '2024Q2']

    """

    @property
    def periods(self) -> list[Period]:
        """
        List of periods in ascending order.

        Returns
        -------
        list[Period]
            List of periods in ascending order.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def to_dataframe(self) -> pd.DataFrame:
        """
        Periods as a pandas DataFrame.

        Returns
        -------
        pandas.DataFrame
            One row per period with columns ``id`` (str), ``start`` and
            ``end`` (``datetime64``) and ``is_actual`` (bool).

        Raises
        ------
        ImportError
            If pandas is not installed.

        Examples
        --------
        >>> from finstack_quant.core.dates import build_periods
        >>> list(build_periods("2024Q1..Q2").to_dataframe().columns)
        ['id', 'start', 'end', 'is_actual']
        """
        ...

    def to_json(self) -> str:
        """
        Serialize to the canonical JSON wire form.

        Returns
        -------
        str
            JSON object with a ``periods`` array.

        Raises
        ------
        ValueError
            If the plan cannot be serialized.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodPlan, build_periods
        >>> plan = build_periods("2024Q1..Q2")
        >>> PeriodPlan.from_json(plan.to_json()) == plan
        True
        """
        ...

    @staticmethod
    def from_json(json: str) -> PeriodPlan:
        """
        Deserialize from the canonical JSON wire form.

        Parameters
        ----------
        json : str
            JSON produced by :meth:`to_json`.

        Returns
        -------
        PeriodPlan
            The reconstructed plan.

        Raises
        ------
        ValueError
            If *json* is malformed.
        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodPlan, build_periods
        >>> plan = build_periods("2024Q1..Q2")
        >>> PeriodPlan.from_json(plan.to_json()) == plan
        True

        """
        ...

    def __iter__(self) -> Iterator[Period]: ...
    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class FiscalConfig:
    """
    Fiscal year configuration.

    Parameters
    ----------
    start_month : int
        Month when the fiscal year starts (1-12).
    start_day : int
        Day when the fiscal year starts (1-31).

    Raises
    ------
    ValueError
        If the month/day combination is invalid.

    Examples
    --------
    >>> from finstack_quant.core.dates import FiscalConfig
    >>> (FiscalConfig.us_federal().start_month, FiscalConfig.uk().start_day)
    (10, 6)

    """

    def __init__(self, start_month: int, start_day: int) -> None:
        """
        Create a fiscal configuration from a start month and day.

        Parameters
        ----------
        start_month : int
            Calendar month number from ``1`` through ``12`` at which each
            fiscal year begins.
        start_day : int
            Calendar day from ``1`` through ``31`` at which each fiscal year
            begins, subject to the selected start month's valid range.

        Raises
        ------
        ValueError
            If the combination is invalid.
        """
        ...

    @classmethod
    def calendar_year(cls) -> FiscalConfig:
        """
        Standard calendar year (January 1).

        Returns
        -------
        FiscalConfig
            Fiscal configuration beginning January 1.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> config = FiscalConfig.calendar_year()
        >>> (config.start_month, config.start_day)
        (1, 1)
        """
        ...

    @classmethod
    def us_federal(cls) -> FiscalConfig:
        """
        US Federal fiscal year (October 1).

        Returns
        -------
        FiscalConfig
            US federal fiscal configuration beginning October 1.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> config = FiscalConfig.us_federal()
        >>> (config.start_month, config.start_day)
        (10, 1)
        """
        ...

    @classmethod
    def uk(cls) -> FiscalConfig:
        """
        UK fiscal year (April 6).

        Returns
        -------
        FiscalConfig
            UK fiscal configuration beginning April 6.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> config = FiscalConfig.uk()
        >>> (config.start_month, config.start_day)
        (4, 6)
        """
        ...

    @classmethod
    def japan(cls) -> FiscalConfig:
        """
        Japanese fiscal year (April 1).

        Returns
        -------
        FiscalConfig
            Japanese fiscal configuration beginning April 1.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> config = FiscalConfig.japan()
        >>> (config.start_month, config.start_day)
        (4, 1)
        """
        ...

    @classmethod
    def australia(cls) -> FiscalConfig:
        """
        Australian fiscal year (July 1).

        Returns
        -------
        FiscalConfig
            Australian fiscal configuration beginning July 1.

        Notes
        -----
        This method does not raise; it returns a fixed instance.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> config = FiscalConfig.australia()
        >>> (config.start_month, config.start_day)
        (7, 1)
        """
        ...

    @property
    def start_month(self) -> int:
        """
        Month when the fiscal year starts (1-12).

        Returns
        -------
        int
            Month when the fiscal year starts (1-12).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def start_day(self) -> int:
        """
        Day when the fiscal year starts (1-31).

        Returns
        -------
        int
            Day when the fiscal year starts (1-31).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

def build_periods(
    spec: str,
    actuals_cutoff: Optional[str] = None,
) -> PeriodPlan:
    """
    Build periods from a range expression.

    Parameters
    ----------
    spec : str
        Range expression (e.g. ``"2025Q1..Q4"``, ``"2024M01..M12"``).
    actuals_cutoff : str | None
        Cutoff period code for actual/forecast split (e.g. ``"2025Q2"``).

    Returns
    -------
    PeriodPlan
        Plan containing the generated periods.

    Raises
    ------
    ValueError
        If *spec* cannot be parsed.

    Examples
    --------
    >>> from finstack_quant.core.dates import build_periods
    >>> [(period.id.code, period.is_actual) for period in build_periods("2024Q1..Q4", "2024Q2").periods]
    [('2024Q1', True), ('2024Q2', True), ('2024Q3', False), ('2024Q4', False)]

    """
    ...

def build_fiscal_periods(
    spec: str,
    fiscal_config: FiscalConfig,
    actuals_cutoff: Optional[str] = None,
) -> PeriodPlan:
    """
    Build fiscal periods with a custom fiscal year configuration.

    Parameters
    ----------
    spec : str
        Range expression.
    fiscal_config : FiscalConfig
        Fiscal year configuration.
    actuals_cutoff : str | None
        Cutoff period code for actual/forecast split.

    Returns
    -------
    PeriodPlan
        Plan containing the generated fiscal periods.

    Raises
    ------
    ValueError
        If *spec* cannot be parsed.

    Examples
    --------
    >>> from finstack_quant.core.dates import FiscalConfig, build_fiscal_periods
    >>> build_fiscal_periods("2024Q1..Q1", FiscalConfig.us_federal()).periods[0].start
    datetime.date(2023, 10, 1)

    """
    ...

# Calendar & business-day adjustment

class BusinessDayConvention:
    """
    Business-day adjustment convention.

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import BusinessDayConvention
    >>> str(BusinessDayConvention.MODIFIED_FOLLOWING)
    'modified_following'
    >>> BusinessDayConvention.from_name("MF") == BusinessDayConvention.MODIFIED_FOLLOWING
    True

    """

    UNADJUSTED: BusinessDayConvention
    """No adjustment -- use the date as given."""
    FOLLOWING: BusinessDayConvention
    """Roll forward to the next business day."""
    MODIFIED_FOLLOWING: BusinessDayConvention
    """Roll forward unless it crosses a month boundary, then roll backward."""
    PRECEDING: BusinessDayConvention
    """Roll backward to the previous business day."""
    MODIFIED_PRECEDING: BusinessDayConvention
    """Roll backward unless it crosses a month boundary, then roll forward."""
    NEAREST: BusinessDayConvention
    """Roll to the closer business day; a tie rolls following (FpML NEAREST)."""

    @classmethod
    def from_name(cls, name: str) -> BusinessDayConvention:
        """
        Parse this variant from its canonical name string.

        Parameters
        ----------
        name : str
            snake_case name (``"following"``, ``"modified_following"``) or
            short code (``"MF"``, ``"F"``, ``"P"``, ``"MP"``, ``"NONE"``),
            case-insensitive.

        Returns
        -------
        BusinessDayConvention
            The matching convention.

        Raises
        ------
        ValueError
            If *name* is not recognised; the message lists the accepted names.

        Examples
        --------
        >>> from finstack_quant.core.dates import BusinessDayConvention
        >>> str(BusinessDayConvention.from_name("following"))
        'following'

        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class CalendarMetadata:
    """
    Metadata for a holiday calendar.

    Immutable value type.

    Examples
    --------
    >>> from finstack_quant.core.dates import HolidayCalendar
    >>> metadata = HolidayCalendar("usny").metadata
    >>> (metadata.id, metadata.name)
    ('usny', 'United States (New York Federal) Holidays')

    """

    @property
    def id(self) -> str:
        """
        Short identifier for this holiday calendar (for example ``NYSE``).

        Returns
        -------
        str
            Short identifier for this holiday calendar (for example ``NYSE``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def name(self) -> str:
        """
        Display name of this holiday calendar.

        Returns
        -------
        str
            Display name of this holiday calendar.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def ignore_weekends(self) -> bool:
        """
        Whether weekends are ignored for this calendar.

        Returns
        -------
        bool
            Whether weekends are ignored for this calendar.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    @property
    def weekend_rule(self) -> str:
        """
        Weekend convention as a snake_case name.

        Returns
        -------
        str
            One of ``"saturday_sunday"``, ``"friday_saturday"``,
            ``"friday_only"``, or ``"none"``.

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class HolidayCalendar:
    """
    A holiday calendar resolved from the global registry.

    The calendar is resolved once at construction and cached. Ids may join
    several calendars with ``+`` (``"nyse+gblo"``): the result is a business
    day only when every member is. Equality and hashing are by canonical
    ``code``; instances pickle.

    Parameters
    ----------
    code : str
        Registered calendar id (``"usny"``, ``"target2"``, ``"nyse"``,
        ``"weekends_only"``, ...; see :func:`available_calendars`) or a
        ``+``-joined union. Matching is ASCII case-insensitive.

    Raises
    ------
    KeyError
        If *code* (or any ``+`` member) is not a registered calendar; the
        message carries "Did you mean ...?" suggestions.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import HolidayCalendar
    >>> calendar = HolidayCalendar("usny")
    >>> (calendar.is_holiday(datetime.date(2025, 1, 1)), calendar.is_business_day(datetime.date(2025, 1, 6)))
    (True, True)

    """

    def __init__(self, code: str) -> None:
        """
        Resolve a calendar by its code.

        Parameters
        ----------
        code : str
            Registered calendar id (``"target2"``, ``"nyse"``) or a
            ``+``-joined union such as ``"nyse+gblo"``.

        Raises
        ------
        KeyError
            If *code* is not a known calendar.
        """
        ...

    def count_business_days(self, start: datetime.date | str, end: datetime.date | str) -> int:
        """
        Count business days in ``[start, end)``.

        Parameters
        ----------
        start : datetime.date | str
            First date included in the count.
        end : datetime.date | str
            Exclusive boundary; ``end <= start`` gives ``0``.

        Returns
        -------
        int
            Number of business days from *start* up to but excluding *end*.

        Raises
        ------
        TypeError
            If either argument is not date-like.
        ValueError
            If either argument is not a valid calendar date or ISO string.

        Examples
        --------
        >>> from finstack_quant.core.dates import HolidayCalendar
        >>> HolidayCalendar("usny").count_business_days("2025-01-01", "2025-01-08")
        4
        """
        ...

    def is_holiday(self, date: datetime.date | str) -> bool:
        """
        Check whether a date is a holiday.

        Parameters
        ----------
        date : datetime.date | str
            The date to check.

        Returns
        -------
        bool
            Whether holiday holds for this `HolidayCalendar`.

        Raises
        ------
        TypeError
            If *date* is not date-like (``datetime.date``, ``datetime.datetime``,
            or ``pandas.Timestamp``).
        ValueError
            If the year/month/day attributes do not form a valid calendar date.
        """
        ...

    def is_business_day(self, date: datetime.date | str) -> bool:
        """
        Check whether a date is a business day.

        Parameters
        ----------
        date : datetime.date | str
            The date to check.

        Returns
        -------
        bool
            Whether business day holds for this `HolidayCalendar`.

        Raises
        ------
        TypeError
            If *date* is not date-like (``datetime.date``, ``datetime.datetime``,
            or ``pandas.Timestamp``).
        ValueError
            If the year/month/day attributes do not form a valid calendar date.
        """
        ...

    @property
    def metadata(self) -> Optional[CalendarMetadata]:
        """
        Calendar metadata (if available).

        Returns
        -------
        CalendarMetadata | None

        Notes
        -----
        This accessor does not raise; it returns the stored or derived value.
        """
        ...

    @property
    def code(self) -> str:
        """
        Canonical registry id (``"usny"``, ``"target2"``, ``"weekends_only"``),
        or the normalized ``a+b`` form for union calendars.

        Returns
        -------
        str
            Canonical calendar code (``HolidayCalendar("USNY").code`` is ``"usny"``).

        Notes
        -----
        This accessor does not raise; it returns the stored value.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

def adjust(
    date: datetime.date | str,
    convention: Union[BusinessDayConvention, str],
    calendar: Union[HolidayCalendar, str],
) -> datetime.date:
    """
    Adjust a date according to a business-day convention and calendar.

    Parameters
    ----------
    date : datetime.date | str
        Date to adjust (``datetime.date``, ``pandas.Timestamp`` or ISO
        ``YYYY-MM-DD`` string).
    convention : BusinessDayConvention | str
        Roll rule: a ``BusinessDayConvention`` or its name
        (``"modified_following"``; short codes ``MF``/``F``/``P``/``MP``/``NONE``).
    calendar : HolidayCalendar | str
        Holiday calendar object or registry id (``"usny"``; ``"nyse+gblo"``
        joins calendars).

    Returns
    -------
    datetime.date
        The adjusted date (unchanged when already a business day or under
        ``UNADJUSTED``).

    Raises
    ------
    KeyError
        If *calendar* names an unknown calendar.
    ValueError
        If *convention* is unknown, *date* is invalid, or no business day
        exists within 100 days.
    TypeError
        If an argument has an unsupported type.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import adjust
    >>> adjust(datetime.date(2025, 1, 4), "following", "usny")
    datetime.date(2025, 1, 6)

    """
    ...

def available_calendars() -> list[str]:
    """
    Return the list of available calendar codes in the global registry.

    Returns
    -------
    list[str]
        Calendar code strings.

    Notes
    -----
    This method does not raise; it returns the stored or derived value.

    Examples
    --------
    >>> from finstack_quant.core.dates import available_calendars
    >>> "usny" in available_calendars()
    True
    """
    ...

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
            ``payment_lag_business_days`` and ``fixing_lag_business_days``
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
            Roll rule (``"modified_following"``; short codes ``MF``/``F``/``P``).
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

    def payment_lag_business_days(self, lag: int) -> ScheduleBuilder:
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
            Whether to enable end-of-month rolling.

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
        Enable CDS IMM date mode and disable standard IMM mode.
        Returns
        -------
        ScheduleBuilder
            This builder.

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
        """
        ...

    def imm(self) -> ScheduleBuilder:
        """
        Enable standard IMM date mode and disable CDS IMM mode.
        Returns
        -------
        ScheduleBuilder
            This builder.

        Notes
        -----
        This method does not raise; it returns the same instance for chaining.
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
            ``error_policy``, ``payment_lag_business_days``,
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

        Delegates entirely to the canonical Rust ``ScheduleSpec::build``:
        under the default ``STRICT`` policy an invalid range (``start`` after
        ``end``) or any build warning raises ``ValueError`` (strict fails
        closed in Rust). Under ``MISSING_CALENDAR_WARNING`` or
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

# Free functions

def create_date(year: int, month: int, day: int) -> datetime.date:
    """
    Create a ``datetime.date`` from year, month (1-12), and day.

    Parameters
    ----------
    year : int
        Calendar year.
    month : int
        Calendar month number from ``1`` through ``12``.
    day : int
        Day of the month.

    Returns
    -------
    datetime.date
        The calendar date represented by *year*, *month*, and *day*.

    Raises
    ------
    ValueError
        If the date components are invalid.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import create_date
    >>> create_date(2025, 2, 28)
    datetime.date(2025, 2, 28)

    """
    ...

def days_since_epoch(date: datetime.date | str) -> int:
    """
    Return the number of days since the Unix epoch (1970-01-01).

    Parameters
    ----------
    date : datetime.date | str
        Input date.

    Returns
    -------
    int
        Signed number of days since 1970-01-01.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import days_since_epoch
    >>> days_since_epoch(datetime.date(1970, 1, 2))
    1

    """
    ...

def date_from_epoch_days(days: int) -> datetime.date:
    """
    Reconstruct a ``datetime.date`` from epoch days (days since 1970-01-01).

    Parameters
    ----------
    days : int
        Number of days since epoch.

    Returns
    -------
    datetime.date
        The calendar date exactly *days* days from 1970-01-01; negative values
        denote dates before the Unix epoch.

    Raises
    ------
    ValueError
        If *days* is out of the valid date range.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import date_from_epoch_days
    >>> date_from_epoch_days(-1)
    datetime.date(1969, 12, 31)

    """
    ...

# IMM, CDS rolls, and listed-option expiries

def third_wednesday(month: int, year: int) -> datetime.date:
    """
    Return the third Wednesday of a month — the IMM date convention.

    Parameters
    ----------
    month : int
        Month number from ``1`` through ``12``.
    year : int
        Four-digit calendar year.

    Returns
    -------
    datetime.date
        The third Wednesday of the given month.

    Raises
    ------
    ValueError
        If *month* is outside ``1..12``.

    Examples
    --------
    >>> from finstack_quant.core.dates import third_wednesday
    >>> third_wednesday(3, 2025)
    datetime.date(2025, 3, 19)

    """
    ...

def third_friday(month: int, year: int) -> datetime.date:
    """
    Return the third Friday of a month — the listed-equity-option expiry.

    Parameters
    ----------
    month : int
        Month number from ``1`` through ``12``.
    year : int
        Four-digit calendar year.

    Returns
    -------
    datetime.date
        The third Friday of the given month.

    Raises
    ------
    ValueError
        If *month* is outside ``1..12``.

    Examples
    --------
    >>> from finstack_quant.core.dates import third_friday
    >>> third_friday(3, 2025)
    datetime.date(2025, 3, 21)

    """
    ...

def next_imm(date: datetime.date | str) -> datetime.date:
    """
    Return the next quarterly IMM date strictly after *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Next March/June/September/December IMM date after *date*.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_imm
    >>> next_imm(datetime.date(2025, 5, 1))
    datetime.date(2025, 6, 18)

    """
    ...

def is_imm_date(date: datetime.date | str) -> bool:
    """
    Return whether *date* is a quarterly IMM date.

    Parameters
    ----------
    date : datetime.date | str
        Candidate date.

    Returns
    -------
    bool
        ``True`` when *date* is the third Wednesday of a quarterly month.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import is_imm_date
    >>> is_imm_date(datetime.date(2025, 3, 19))
    True

    """
    ...

def is_cds_date(date: datetime.date | str) -> bool:
    """
    Return whether *date* is a standard CDS roll date.

    Parameters
    ----------
    date : datetime.date | str
        Candidate date.

    Returns
    -------
    bool
        ``True`` when *date* is a standard CDS roll date.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import is_cds_date
    >>> is_cds_date(datetime.date(2025, 6, 20))
    True

    """
    ...

def next_cds_date(date: datetime.date | str) -> datetime.date:
    """
    Return the next standard CDS roll date on or after *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Next standard CDS roll date.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_cds_date
    >>> next_cds_date(datetime.date(2025, 5, 1))
    datetime.date(2025, 6, 20)

    """
    ...

def prev_cds_date(date: datetime.date | str) -> datetime.date:
    """
    Return the most recent standard CDS roll date on or before *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Most recent standard CDS roll date.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import prev_cds_date
    >>> prev_cds_date(datetime.date(2025, 5, 1))
    datetime.date(2025, 3, 20)

    """
    ...

def prev_cds_semiannual_roll(date: datetime.date | str) -> datetime.date:
    """
    Return the most recent semi-annual CDS roll on or before *date*.

    Semi-annual rolls are the March and September dates only.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Most recent March or September CDS roll date.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import prev_cds_semiannual_roll
    >>> prev_cds_semiannual_roll(datetime.date(2025, 5, 1))
    datetime.date(2025, 3, 20)

    """
    ...

def next_semiannual_cds_maturity(date: datetime.date | str) -> datetime.date:
    """
    Return the next semi-annual CDS maturity date after *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Next semi-annual CDS maturity date.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_semiannual_cds_maturity
    >>> next_semiannual_cds_maturity(datetime.date(2025, 5, 1))
    datetime.date(2025, 6, 20)

    """
    ...

def imm_option_expiry(month: int, year: int) -> datetime.date:
    """
    Return the expiry of the option on the IMM future for a month.

    Parameters
    ----------
    month : int
        Month number from ``1`` through ``12``.
    year : int
        Four-digit calendar year.

    Returns
    -------
    datetime.date
        Option expiry date for that IMM contract month.

    Raises
    ------
    ValueError
        If *month* is outside ``1..12``.

    Examples
    --------
    >>> from finstack_quant.core.dates import imm_option_expiry
    >>> imm_option_expiry(3, 2025)
    datetime.date(2025, 3, 14)

    """
    ...

def next_imm_option_expiry(date: datetime.date | str) -> datetime.date:
    """
    Return the next quarterly IMM option expiry strictly after *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Next quarterly IMM option expiry.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_imm_option_expiry
    >>> next_imm_option_expiry(datetime.date(2025, 5, 1))
    datetime.date(2025, 6, 13)

    """
    ...

def next_equity_option_expiry(date: datetime.date | str) -> datetime.date:
    """
    Return the next monthly listed-equity-option expiry after *date*.

    Parameters
    ----------
    date : datetime.date | str
        Reference date.

    Returns
    -------
    datetime.date
        Next third-Friday expiry strictly after *date*.

    Raises
    ------
    TypeError
        If *date* is not a date-like object with integer ``year``, ``month``,
        and ``day`` attributes.
    ValueError
        If those attributes do not form a valid calendar date.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import next_equity_option_expiry
    >>> next_equity_option_expiry(datetime.date(2025, 5, 1))
    datetime.date(2025, 5, 16)

    """
    ...

# Date extensions (finstack_quant_core::dates::DateExt)

def add_business_days(
    date: datetime.date | str,
    n: int,
    calendar: Union[HolidayCalendar, str],
) -> datetime.date:
    """
    Add (or subtract) *n* business days to *date* under *calendar*.

    Skips weekends and holidays according to the calendar. Positive *n*
    moves forward, negative *n* backward; ``0`` returns *date* unchanged even
    when it is not itself a business day.

    Parameters
    ----------
    date : datetime.date | str
        Anchor date.
    n : int
        Signed number of business days to move.
    calendar : HolidayCalendar | str
        Holiday calendar object or registry id (``"usny"``; ``"nyse+gblo"``
        joins calendars).

    Returns
    -------
    datetime.date
        The shifted business day.

    Raises
    ------
    KeyError
        If *calendar* names an unknown calendar.
    ValueError
        If *date* is invalid or no business day is found within the bounded
        (100-day) search window.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import add_business_days
    >>> add_business_days(datetime.date(2025, 6, 27), 3, "target2")
    datetime.date(2025, 7, 2)

    """
    ...

def add_weekdays(date: datetime.date | str, n: int) -> datetime.date:
    """
    Add (or subtract) *n* weekdays to *date*, skipping only Saturdays and Sundays.

    Holidays are not considered; use :func:`add_business_days` with a
    calendar for holiday-aware arithmetic.

    Parameters
    ----------
    date : datetime.date | str
        Anchor date.
    n : int
        Signed number of weekdays to move; ``0`` returns *date* unchanged.

    Returns
    -------
    datetime.date
        The shifted weekday.

    Raises
    ------
    ValueError
        If *date* is not a valid calendar date or ISO string.

    Examples
    --------
    >>> from finstack_quant.core.dates import add_weekdays
    >>> add_weekdays("2025-01-03", 1)
    datetime.date(2025, 1, 6)

    """
    ...

def add_months(date: datetime.date | str, months: int) -> datetime.date:
    """
    Add *months* to *date*, clamping to the last valid day of the target month.

    Parameters
    ----------
    date : datetime.date | str
        Anchor date.
    months : int
        Signed number of calendar months (Jan 31 + 1 gives Feb 28/29).

    Returns
    -------
    datetime.date
        The shifted date.

    Raises
    ------
    ValueError
        If *date* is not a valid calendar date or ISO string.

    Examples
    --------
    >>> from finstack_quant.core.dates import add_months
    >>> add_months("2024-01-31", 1)
    datetime.date(2024, 2, 29)

    """
    ...

def end_of_month(date: datetime.date | str) -> datetime.date:
    """
    Last day of the month containing *date*.

    Parameters
    ----------
    date : datetime.date | str
        Any date in the month.

    Returns
    -------
    datetime.date
        The month-end date.

    Raises
    ------
    ValueError
        If *date* is not a valid calendar date or ISO string.

    Examples
    --------
    >>> from finstack_quant.core.dates import end_of_month
    >>> end_of_month("2024-02-15")
    datetime.date(2024, 2, 29)

    """
    ...

def is_weekend(date: datetime.date | str) -> bool:
    """
    Whether *date* falls on a Saturday or Sunday.

    Parameters
    ----------
    date : datetime.date | str
        Date to test.

    Returns
    -------
    bool
        ``True`` for Saturday or Sunday.

    Raises
    ------
    ValueError
        If *date* is not a valid calendar date or ISO string.

    Examples
    --------
    >>> from finstack_quant.core.dates import is_weekend
    >>> is_weekend("2025-01-04")
    True

    """
    ...

def quarter(date: datetime.date | str) -> int:
    """
    Calendar quarter (1-4) containing *date*.

    Parameters
    ----------
    date : datetime.date | str
        Date to classify.

    Returns
    -------
    int
        Quarter number from ``1`` (Jan-Mar) to ``4`` (Oct-Dec).

    Raises
    ------
    ValueError
        If *date* is not a valid calendar date or ISO string.

    Examples
    --------
    >>> from finstack_quant.core.dates import quarter
    >>> quarter("2025-08-15")
    3

    """
    ...

def fiscal_year(date: datetime.date | str, config: FiscalConfig) -> int:
    """
    Fiscal year label of *date* under a fiscal-year configuration.

    Parameters
    ----------
    date : datetime.date | str
        Date to classify.
    config : FiscalConfig
        Fiscal-year start (``FiscalConfig.us_federal()`` starts October 1, so
        2024-10-15 belongs to fiscal year 2025).

    Returns
    -------
    int
        Fiscal year label (the calendar year in which the fiscal year ends).

    Raises
    ------
    ValueError
        If *date* is not a valid calendar date or ISO string.

    Examples
    --------
    >>> from finstack_quant.core.dates import FiscalConfig, fiscal_year
    >>> fiscal_year("2024-10-15", FiscalConfig.us_federal())
    2025

    """
    ...

def months_until(date: datetime.date | str, other: datetime.date | str) -> int:
    """
    Whole months from *date* to *other* (``0`` when *other* is earlier).

    Counts complete months, subtracting one when *other*'s day-of-month has
    not yet reached *date*'s (month-end to month-end counts as whole). This
    is the loan-seasoning convention used by structured-credit models.

    Parameters
    ----------
    date : datetime.date | str
        Start date.
    other : datetime.date | str
        End date.

    Returns
    -------
    int
        Non-negative month count.

    Raises
    ------
    ValueError
        If either argument is not a valid calendar date or ISO string.

    Examples
    --------
    >>> from finstack_quant.core.dates import months_until
    >>> months_until("2020-01-15", "2022-03-10")
    25

    """
    ...
