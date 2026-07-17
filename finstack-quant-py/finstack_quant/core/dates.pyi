"""
Date, calendar, and schedule utilities from ``finstack-quant-core``.

Provides day-count conventions, tenor types, period generation, schedule
building, holiday calendars, and business-day adjustment functions.

Example::

    >>> import datetime
    >>> from finstack_quant.core.dates import DayCount, Tenor, ScheduleBuilder
    >>> dc = DayCount.ACT_365F
    >>> dc.year_fraction(datetime.date(2024, 1, 1), datetime.date(2025, 1, 1))
    1.0

Examples
--------
>>> import finstack_quant.core.dates as dates
>>> dates.__name__
'finstack_quant.core.dates'
"""

from __future__ import annotations

import datetime
from typing import Optional, Sequence, Union

__all__ = [
    # day-count
    "DayCount",
    "DayCountContext",
    "DayCountContextState",
    "Thirty360Convention",
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
    # free functions
    "create_date",
    "days_since_epoch",
    "date_from_epoch_days",
]

class SifmaSettlementClass:
    """
    SIFMA good-delivery settlement class.

    Examples
    --------
    >>> from finstack_quant.core.dates import SifmaSettlementClass
    >>> SifmaSettlementClass.__name__
    'SifmaSettlementClass'
    """

    A: SifmaSettlementClass
    B: SifmaSettlementClass
    C: SifmaSettlementClass
    D: SifmaSettlementClass

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

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.dates import SifmaSettlementClass
        >>> callable(SifmaSettlementClass.from_agency_term)
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
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.core.dates import sifma_settlement_date
    >>> callable(sifma_settlement_date)
    True
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
        Result of sifma settlement date for class for the binding in the annotated representation.

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.core.dates import sifma_settlement_date_for_class
    >>> callable(sifma_settlement_date_for_class)
    True
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
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.core.dates import estimated_sifma_settlement_date_for_class
    >>> callable(estimated_sifma_settlement_date_for_class)
    True
    """
    ...

def next_sifma_settlement(date: datetime.date) -> datetime.date | None:
    """
    Return the next published SIFMA settlement date on or after a date.

    Parameters
    ----------
    date : datetime.date
        Calendar date from which to search the published settlement calendar.

    Returns
    -------
    datetime.date or None
        Earliest available settlement date not before ``date``, or ``None``.

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.core.dates import next_sifma_settlement
    >>> callable(next_sifma_settlement)
    True
    """
    ...

# ---------------------------------------------------------------------------
# Day-count conventions
# ---------------------------------------------------------------------------

class DayCount:
    """
    Day-count convention for year-fraction calculations.

    Immutable, hashable enum-style type with class attributes for each
    supported convention.

    Examples
    --------
    >>> import datetime
    >>> from finstack_quant.core.dates import DayCount
    >>> dc = DayCount.ACT_360
    >>> dc.year_fraction(datetime.date(2024, 1, 1), datetime.date(2024, 7, 1))
    0.5027777777777778
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
            ``"thirty_360"``, ``"bus_252"``).

        Returns
        -------
        DayCount

            Result of from name for this `DayCount` in the annotated representation.
        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount
        >>> callable(DayCount.from_name)
        True
        """
        ...

    def year_fraction(
        self,
        start: datetime.date,
        end: datetime.date,
        ctx: Optional[DayCountContext] = None,
    ) -> float:
        """
        Compute the year fraction between two dates.

        Parameters
        ----------
        start : datetime.date
            Start date (inclusive).
        end : datetime.date
            End date (exclusive).
        ctx : DayCountContext | None
            Optional context providing calendar or frequency data
            required by conventions like Bus/252 or Act/Act ISMA.

        Returns
        -------
        float
            Non-negative year fraction.

        Raises
        ------
        ValueError
            If *start* > *end* or required context is missing.
        """
        ...

    def signed_year_fraction(
        self,
        start: datetime.date,
        end: datetime.date,
        ctx: Optional[DayCountContext] = None,
    ) -> float:
        """
        Compute the signed year fraction (negative when start > end).

        Parameters
        ----------
        start : datetime.date
            Start date.
        end : datetime.date
            End date.
        ctx : DayCountContext | None
            Optional context for calendar/frequency-dependent conventions.

        Returns
        -------
        float
            Signed year fraction.

        Raises
        ------
        ValueError
            If required context is missing.
        """
        ...

    @staticmethod
    def calendar_days(start: datetime.date, end: datetime.date) -> int:
        """
        Count the calendar days between two dates.

        Parameters
        ----------
        start : datetime.date
            Start date.
        end : datetime.date
            End date.

        Returns
        -------
        int
            Signed number of calendar days (end - start).

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.dates import DayCount
        >>> callable(DayCount.calendar_days)
        True
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
    - **Act/Act (ISMA)** requires the coupon ``frequency``.

    Parameters
    ----------
    calendar_id : str | None
        Calendar identifier (e.g. ``"target2"``).
    frequency : Tenor | None
        Coupon frequency for ISMA conventions.
    bus_basis : int | None
        Custom business-day divisor (defaults to 252 when omitted).

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCountContext
    >>> DayCountContext.__name__
    'DayCountContext'
    """

    def __init__(
        self,
        calendar_id: Optional[str] = None,
        frequency: Optional[Tenor] = None,
        bus_basis: Optional[int] = None,
        coupon_period: Optional[tuple[datetime.date, datetime.date]] = None,
        end_is_termination_date: bool = False,
    ) -> None:
        """
        Create a day-count context.

        Parameters
        ----------
        calendar_id : str | None
            Calendar identifier.
        frequency : Tenor | None
            Coupon frequency.
        bus_basis : int | None
            Custom business-day divisor.
        coupon_period : tuple[datetime.date, datetime.date] | None
            Reference coupon period ``(start, end)`` for ACT/ACT (ICMA).
        end_is_termination_date : bool
            Whether the accrual end is the instrument termination date.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def calendar_id(self) -> Optional[str]:
        """
        Optional calendar identifier.

        Returns
        -------
        str | None
            The calendar id exposed by this `DayCountContext`.
        """
        ...

    @property
    def frequency(self) -> Optional[Tenor]:
        """
        Optional coupon frequency.

        Returns
        -------
        Tenor | None
            The frequency exposed by this `DayCountContext`.
        """
        ...

    @property
    def bus_basis(self) -> Optional[int]:
        """
        Optional custom business-day divisor.

        Returns
        -------
        int | None
            The bus basis exposed by this `DayCountContext`.
        """
        ...

    @property
    def coupon_period(self) -> Optional[tuple[datetime.date, datetime.date]]:
        """
        Optional reference coupon period as ``(start, end)`` dates.

        Returns
        -------
        tuple[datetime.date, datetime.date] | None
        """
        ...

    @property
    def end_is_termination_date(self) -> bool:
        """
        Whether the accrual end is the instrument termination date.

        Returns
        -------
        bool
            The end is termination date exposed by this `DayCountContext`.
        """
        ...

    def to_state(self) -> DayCountContextState:
        """
        Convert to a serializable state snapshot.

        Returns
        -------
        DayCountContextState
        """
        ...

    def __repr__(self) -> str: ...

class DayCountContextState:
    """
    Serializable snapshot of :class:`DayCountContext` for persistence.

    Parameters
    ----------
    calendar_id : str | None
        Calendar identifier.
    frequency : Tenor | None
        Coupon frequency.
    bus_basis : int | None
        Custom business-day divisor.

    Examples
    --------
    >>> from finstack_quant.core.dates import DayCountContextState
    >>> DayCountContextState.__name__
    'DayCountContextState'
    """

    def __init__(
        self,
        calendar_id: Optional[str] = None,
        frequency: Optional[Tenor] = None,
        bus_basis: Optional[int] = None,
        coupon_period: Optional[tuple[datetime.date, datetime.date]] = None,
        end_is_termination_date: bool = False,
    ) -> None:
        """
        Create a context state.

        Parameters
        ----------
        calendar_id : str | None
            Calendar identifier.
        frequency : Tenor | None
            Coupon frequency.
        bus_basis : int | None
            Custom business-day divisor.
        coupon_period : tuple[datetime.date, datetime.date] | None
            Reference coupon period ``(start, end)``.
        end_is_termination_date : bool
            Whether the accrual end is the instrument termination date.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def to_context(self) -> DayCountContext:
        """
        Reconstruct a live :class:`DayCountContext` from this state.

        Returns
        -------
        DayCountContext
            Result of to context for this `DayCountContextState` in the annotated representation.
        """
        ...

    @property
    def calendar_id(self) -> Optional[str]:
        """
        Optional calendar identifier.

        Returns
        -------
        str | None
            The calendar id exposed by this `DayCountContextState`.
        """
        ...

    @property
    def frequency(self) -> Optional[Tenor]:
        """
        Optional coupon frequency.

        Returns
        -------
        Tenor | None
            The frequency exposed by this `DayCountContextState`.
        """
        ...

    @property
    def bus_basis(self) -> Optional[int]:
        """
        Optional custom business-day divisor.

        Returns
        -------
        int | None
            The bus basis exposed by this `DayCountContextState`.
        """
        ...

    @property
    def coupon_period(self) -> Optional[tuple[datetime.date, datetime.date]]:
        """
        Optional reference coupon period as ``(start, end)`` dates.

        Returns
        -------
        tuple[datetime.date, datetime.date] | None
        """
        ...

    @property
    def end_is_termination_date(self) -> bool:
        """
        Whether the accrual end is the instrument termination date.

        Returns
        -------
        bool
            The end is termination date exposed by this `DayCountContextState`.
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
    >>> Thirty360Convention.__name__
    'Thirty360Convention'
    """

    US_SIA: Thirty360Convention
    """US 30/360 SIA / Bond Basis convention."""
    ISDA: Thirty360Convention
    """30/360 ISDA convention."""
    EUROPEAN: Thirty360Convention
    """European 30E/360 convention."""

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

# ---------------------------------------------------------------------------
# Tenor
# ---------------------------------------------------------------------------

class TenorUnit:
    """
    Frequency/tenor unit enumeration.

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import TenorUnit
    >>> TenorUnit.__name__
    'TenorUnit'
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

            Result of from char for this `TenorUnit` in the annotated representation.
        Raises
        ------
        ValueError
            If *ch* is not a valid unit designator.

        Examples
        --------
        >>> from finstack_quant.core.dates import TenorUnit
        >>> callable(TenorUnit.from_char)
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
    >>> Tenor.__name__
    'Tenor'
    """

    def __init__(self, count: int, unit: TenorUnit) -> None:
        """
        Construct a tenor from a count and unit.

        Parameters
        ----------
        count : int
            Numeric count.
        unit : TenorUnit
            Tenor unit.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @classmethod
    def parse(cls, s: str) -> Tenor:
        """
        Parse a tenor string.

        Parameters
        ----------
        s : str
            Tenor string (e.g. ``"3M"``, ``"1Y"``, ``"2W"``).

        Returns
        -------
        Tenor

            Result of parse for this `Tenor` in the annotated representation.
        Raises
        ------
        ValueError
            If *s* cannot be parsed.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> callable(Tenor.parse)
        True
        """
        ...

    @classmethod
    def daily(cls) -> Tenor:
        """
        Compute daily for `Tenor`.
        1-day tenor.

        Returns
        -------
        Tenor
            Result of daily for this `Tenor` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> callable(Tenor.daily)
        True
        """
        ...

    @classmethod
    def weekly(cls) -> Tenor:
        """
        Compute weekly for `Tenor`.
        1-week tenor.

        Returns
        -------
        Tenor
            Result of weekly for this `Tenor` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> callable(Tenor.weekly)
        True
        """
        ...

    @classmethod
    def biweekly(cls) -> Tenor:
        """
        Compute biweekly for `Tenor`.
        2-week tenor.

        Returns
        -------
        Tenor
            Result of biweekly for this `Tenor` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> callable(Tenor.biweekly)
        True
        """
        ...

    @classmethod
    def monthly(cls) -> Tenor:
        """
        Compute monthly for `Tenor`.
        1-month tenor.

        Returns
        -------
        Tenor
            Result of monthly for this `Tenor` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> callable(Tenor.monthly)
        True
        """
        ...

    @classmethod
    def bimonthly(cls) -> Tenor:
        """
        Compute bimonthly for `Tenor`.
        2-month tenor.

        Returns
        -------
        Tenor
            Result of bimonthly for this `Tenor` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> callable(Tenor.bimonthly)
        True
        """
        ...

    @classmethod
    def quarterly(cls) -> Tenor:
        """
        3-month (quarterly) tenor.

        Returns
        -------
        Tenor
            Result of quarterly for this `Tenor` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> callable(Tenor.quarterly)
        True
        """
        ...

    @classmethod
    def semi_annual(cls) -> Tenor:
        """
        6-month (semi-annual) tenor.

        Returns
        -------
        Tenor
            Result of semi annual for this `Tenor` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> callable(Tenor.semi_annual)
        True
        """
        ...

    @classmethod
    def annual(cls) -> Tenor:
        """
        12-month (annual) tenor.

        Returns
        -------
        Tenor
            Result of annual for this `Tenor` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> callable(Tenor.annual)
        True
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

            Result of from payments per year for this `Tenor` in the annotated representation.
        Raises
        ------
        ValueError
            If *payments* does not map to a standard tenor.

        Examples
        --------
        >>> from finstack_quant.core.dates import Tenor
        >>> callable(Tenor.from_payments_per_year)
        True
        """
        ...

    @property
    def count(self) -> int:
        """
        Return the count for `Tenor`.
        Numeric count.

        Returns
        -------
        int
            The count exposed by this `Tenor`.
        """
        ...

    @property
    def unit(self) -> TenorUnit:
        """
        Unit of the tenor.

        Returns
        -------
        TenorUnit
            The unit exposed by this `Tenor`.
        """
        ...

    @property
    def months(self) -> Optional[int]:
        """
        Equivalent whole months (``None`` for day/week tenors).

        Returns
        -------
        int | None
            The months exposed by this `Tenor`.
        """
        ...

    @property
    def days(self) -> Optional[int]:
        """
        Equivalent whole days (``None`` for month/year tenors).

        Returns
        -------
        int | None
            The days exposed by this `Tenor`.
        """
        ...

    def to_years_simple(self) -> float:
        """
        Approximate tenor length in years (simple estimate, no calendar).

        Returns
        -------
        float
            Result of to years simple for this `Tenor` in the annotated representation.
        """
        ...

    def to_days_approx(self) -> int:
        """
        Approximate tenor length in calendar days.

        Returns
        -------
        int
            Result of to days approx for this `Tenor` in the annotated representation.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

# ---------------------------------------------------------------------------
# Periods
# ---------------------------------------------------------------------------

class PeriodKind:
    """
    Period frequency kind.

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import PeriodKind
    >>> PeriodKind.__name__
    'PeriodKind'
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
            Period kind identifier (e.g. ``"quarterly"``, ``"m"``, ``"annual"``).

        Returns
        -------
        PeriodKind

            Result of from name for this `PeriodKind` in the annotated representation.
        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodKind
        >>> callable(PeriodKind.from_name)
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
            The periods per year exposed by this `PeriodKind`.
        """
        ...

    @property
    def annualization_factor(self) -> float:
        """
        Annualization factor for this frequency.

        Returns
        -------
        float
            The annualization factor exposed by this `PeriodKind`.
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
    >>> PeriodId.__name__
    'PeriodId'
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

            Result of parse for this `PeriodId` in the annotated representation.
        Raises
        ------
        ValueError
            If *code* cannot be parsed.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> callable(PeriodId.parse)
        True
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
            Result of month for this `PeriodId` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> callable(PeriodId.month)
        True
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
            Result of quarter for this `PeriodId` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> callable(PeriodId.quarter)
        True
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
            Result of annual for this `PeriodId` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> callable(PeriodId.annual)
        True
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
            Result of half for this `PeriodId` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> callable(PeriodId.half)
        True
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
            Result of week for this `PeriodId` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> callable(PeriodId.week)
        True
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
            Result of day for this `PeriodId` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

        Examples
        --------
        >>> from finstack_quant.core.dates import PeriodId
        >>> callable(PeriodId.day)
        True
        """
        ...

    @property
    def code(self) -> str:
        """
        Period code string (e.g. ``"2025Q1"``).

        Returns
        -------
        str
            The code exposed by this `PeriodId`.
        """
        ...

    @property
    def year(self) -> int:
        """
        Gregorian or fiscal year label.

        Returns
        -------
        int
            The year exposed by this `PeriodId`.
        """
        ...

    @property
    def index(self) -> int:
        """
        Ordinal index within the year.

        Returns
        -------
        int
            The index exposed by this `PeriodId`.
        """
        ...

    @property
    def kind(self) -> PeriodKind:
        """
        Kind (frequency) of this period.

        Returns
        -------
        PeriodKind
            The kind exposed by this `PeriodId`.
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
        """
        ...

    @property
    def periods_per_year(self) -> int:
        """
        Number of periods per year for this kind.

        Returns
        -------
        int
            The periods per year exposed by this `PeriodId`.
        """
        ...

    def next(self) -> PeriodId:
        """
        Next period in sequence.

        Returns
        -------
        PeriodId

            Result of next for this `PeriodId` in the annotated representation.
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

            Result of prev for this `PeriodId` in the annotated representation.
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
            Result of next fiscal for this `PeriodId` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
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
            Result of prev fiscal for this `PeriodId` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class Period:
    """
    A concrete period with start/end dates and an actual/forecast flag.

    Immutable value type returned by period-building functions.

    Examples
    --------
    >>> from finstack_quant.core.dates import Period
    >>> Period.__name__
    'Period'
    """

    @property
    def id(self) -> PeriodId:
        """
        Period identifier.

        Returns
        -------
        PeriodId
            The id exposed by this `Period`.
        """
        ...

    @property
    def start(self) -> datetime.date:
        """
        Inclusive start date.

        Returns
        -------
        datetime.date
            The start exposed by this `Period`.
        """
        ...

    @property
    def end(self) -> datetime.date:
        """
        Exclusive end date.

        Returns
        -------
        datetime.date
            The end exposed by this `Period`.
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
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class PeriodPlan:
    """
    A plan containing a contiguous sequence of periods.

    Returned by :func:`build_periods` and :func:`build_fiscal_periods`.

    Examples
    --------
    >>> from finstack_quant.core.dates import PeriodPlan
    >>> PeriodPlan.__name__
    'PeriodPlan'
    """

    @property
    def periods(self) -> list[Period]:
        """
        List of periods in ascending order.

        Returns
        -------
        list[Period]
            The periods exposed by this `PeriodPlan`.
        """
        ...

    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

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
    >>> FiscalConfig.__name__
    'FiscalConfig'
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
            Result of calendar year for this `FiscalConfig` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> callable(FiscalConfig.calendar_year)
        True
        """
        ...

    @classmethod
    def us_federal(cls) -> FiscalConfig:
        """
        US Federal fiscal year (October 1).

        Returns
        -------
        FiscalConfig
            Result of us federal for this `FiscalConfig` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> callable(FiscalConfig.us_federal)
        True
        """
        ...

    @classmethod
    def uk(cls) -> FiscalConfig:
        """
        UK fiscal year (April 6).

        Returns
        -------
        FiscalConfig
            Result of uk for this `FiscalConfig` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> callable(FiscalConfig.uk)
        True
        """
        ...

    @classmethod
    def japan(cls) -> FiscalConfig:
        """
        Japanese fiscal year (April 1).

        Returns
        -------
        FiscalConfig
            Result of japan for this `FiscalConfig` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> callable(FiscalConfig.japan)
        True
        """
        ...

    @classmethod
    def canada(cls) -> FiscalConfig:
        """
        Canadian fiscal year (April 1).

        Returns
        -------
        FiscalConfig
            Result of canada for this `FiscalConfig` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> callable(FiscalConfig.canada)
        True
        """
        ...

    @classmethod
    def australia(cls) -> FiscalConfig:
        """
        Australian fiscal year (July 1).

        Returns
        -------
        FiscalConfig
            Result of australia for this `FiscalConfig` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> callable(FiscalConfig.australia)
        True
        """
        ...

    @classmethod
    def germany(cls) -> FiscalConfig:
        """
        German fiscal year (January 1).

        Returns
        -------
        FiscalConfig
            Result of germany for this `FiscalConfig` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> callable(FiscalConfig.germany)
        True
        """
        ...

    @classmethod
    def france(cls) -> FiscalConfig:
        """
        French fiscal year (January 1).

        Returns
        -------
        FiscalConfig
            Result of france for this `FiscalConfig` in the annotated representation.

        Examples
        --------
        >>> from finstack_quant.core.dates import FiscalConfig
        >>> callable(FiscalConfig.france)
        True
        """
        ...

    @property
    def start_month(self) -> int:
        """
        Month when the fiscal year starts (1-12).

        Returns
        -------
        int
            The start month exposed by this `FiscalConfig`.
        """
        ...

    @property
    def start_day(self) -> int:
        """
        Day when the fiscal year starts (1-31).

        Returns
        -------
        int
            The start day exposed by this `FiscalConfig`.
        """
        ...

    def __repr__(self) -> str: ...

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
    >>> callable(build_periods)
    True
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
    >>> from finstack_quant.core.dates import build_fiscal_periods
    >>> callable(build_fiscal_periods)
    True
    """
    ...

# ---------------------------------------------------------------------------
# Calendar & business-day adjustment
# ---------------------------------------------------------------------------

class BusinessDayConvention:
    """
    Business-day adjustment convention.

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import BusinessDayConvention
    >>> BusinessDayConvention.__name__
    'BusinessDayConvention'
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

    @classmethod
    def from_name(cls, name: str) -> BusinessDayConvention:
        """
        Parse from a string.

        Parameters
        ----------
        name : str
            Convention identifier (e.g. ``"following"``,
            ``"modified_following"``).

        Returns
        -------
        BusinessDayConvention

        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.dates import BusinessDayConvention
        >>> callable(BusinessDayConvention.from_name)
        True
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
    >>> from finstack_quant.core.dates import CalendarMetadata
    >>> CalendarMetadata.__name__
    'CalendarMetadata'
    """

    @property
    def id(self) -> str:
        """
        Calendar short code.

        Returns
        -------
        str
            The id exposed by this `CalendarMetadata`.
        """
        ...

    @property
    def name(self) -> str:
        """
        Human-readable name.

        Returns
        -------
        str
            The name exposed by this `CalendarMetadata`.
        """
        ...

    @property
    def ignore_weekends(self) -> bool:
        """
        Whether weekends are ignored for this calendar.

        Returns
        -------
        bool
            The ignore weekends exposed by this `CalendarMetadata`.
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
        """
        ...

    def __repr__(self) -> str: ...

class HolidayCalendar:
    """
    A holiday calendar resolved from the global registry.

    Parameters
    ----------
    code : str
        Calendar code (e.g. ``"target2"``, ``"nyse"``).

    Raises
    ------
    ValueError
        If *code* does not match any known calendar.

    Examples
    --------
    >>> from finstack_quant.core.dates import HolidayCalendar
    >>> HolidayCalendar.__name__
    'HolidayCalendar'
    """

    def __init__(self, code: str) -> None:
        """
        Resolve a calendar by its code.

        Parameters
        ----------
        code : str
            Calendar code (e.g. ``"target2"``, ``"nyse"``).

        Raises
        ------
        ValueError
            If *code* is not a known calendar.
        """
        ...

    def is_holiday(self, date: datetime.date) -> bool:
        """
        Check whether a date is a holiday.

        Parameters
        ----------
        date : datetime.date
            The date to check.

        Returns
        -------
        bool
            Whether holiday holds for this `HolidayCalendar`.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def is_business_day(self, date: datetime.date) -> bool:
        """
        Check whether a date is a business day.

        Parameters
        ----------
        date : datetime.date
            The date to check.

        Returns
        -------
        bool
            Whether business day holds for this `HolidayCalendar`.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    @property
    def metadata(self) -> Optional[CalendarMetadata]:
        """
        Calendar metadata (if available).

        Returns
        -------
        CalendarMetadata | None
        """
        ...

    @property
    def code(self) -> str:
        """
        Return the code for `HolidayCalendar`.
        Calendar code.

        Returns
        -------
        str
            The code exposed by this `HolidayCalendar`.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

def adjust(
    date: datetime.date,
    convention: Union[BusinessDayConvention, str],
    calendar: Union[HolidayCalendar, str],
) -> datetime.date:
    """
    Adjust a date according to a business-day convention and calendar.

    Parameters
    ----------
    date : datetime.date
        The date to adjust.
    convention : BusinessDayConvention | str
        Adjustment convention.
    calendar : HolidayCalendar | str
        Holiday calendar (object or code string).

    Returns
    -------
    datetime.date
        The adjusted date.

    Raises
    ------
    ValueError
        If the calendar or convention is invalid.

    Examples
    --------
    >>> from finstack_quant.core.dates import adjust
    >>> callable(adjust)
    True
    """
    ...

def available_calendars() -> list[str]:
    """
    Return the list of available calendar codes in the global registry.

    Returns
    -------
    list[str]
        Calendar code strings.

    Examples
    --------
    >>> from finstack_quant.core.dates import available_calendars
    >>> callable(available_calendars)
    True
    """
    ...

# ---------------------------------------------------------------------------
# Schedule
# ---------------------------------------------------------------------------

class StubKind:
    """
    Stub positioning rule for schedule generation.

    Immutable, hashable enum-style type.

    Examples
    --------
    >>> from finstack_quant.core.dates import StubKind
    >>> StubKind.__name__
    'StubKind'
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
        Parse from a string.

        Parameters
        ----------
        name : str
            Stub kind identifier (e.g. ``"short_front"``, ``"long_back"``).

        Returns
        -------
        StubKind

            Result of from name for this `StubKind` in the annotated representation.
        Raises
        ------
        ValueError
            If *name* is not recognised.

        Examples
        --------
        >>> from finstack_quant.core.dates import StubKind
        >>> callable(StubKind.from_name)
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
    >>> ScheduleErrorPolicy.__name__
    'ScheduleErrorPolicy'
    """

    STRICT: ScheduleErrorPolicy
    """Strict -- errors are immediately propagated."""
    MISSING_CALENDAR_WARNING: ScheduleErrorPolicy
    """Emit a warning for missing calendars, but continue."""
    GRACEFUL_EMPTY: ScheduleErrorPolicy
    """Gracefully return an empty schedule on error."""

    def __repr__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class Schedule:
    """
    A generated date schedule.

    Immutable value type produced by :class:`ScheduleBuilder`.

    Examples
    --------
    >>> from finstack_quant.core.dates import Schedule
    >>> Schedule.__name__
    'Schedule'
    """

    @property
    def dates(self) -> list[datetime.date]:
        """
        Schedule dates as a list of ``datetime.date``.

        Returns
        -------
        list[datetime.date]
        """
        ...

    def has_warnings(self) -> bool:
        """
        Whether any warnings were generated during schedule building.

        Returns
        -------
        bool
            Whether this `Schedule` has warnings.
        """
        ...

    def used_graceful_fallback(self) -> bool:
        """
        Whether a graceful fallback was used during schedule building.

        Returns
        -------
        bool
            Result of used graceful fallback for this `Schedule` in the annotated representation.
        """
        ...

    @property
    def warnings(self) -> list[str]:
        """
        Warning messages (if any).

        Returns
        -------
        list[str]
            The warnings exposed by this `Schedule`.
        """
        ...

    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class ScheduleBuilder:
    """
    Builder for constructing date schedules.

    Setters mutate the builder **in place** and return that same instance,
    matching Rust's fluent builder semantics.

    Parameters
    ----------
    start : datetime.date
        Schedule start date.
    end : datetime.date
        Schedule end date (must be after *start*).

    Raises
    ------
    ValueError
        If *start* >= *end*.

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
    ...     ScheduleBuilder(date(2025, 1, 15), date(2030, 1, 15))
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

    def __init__(self, start: datetime.date, end: datetime.date) -> None:
        """
        Start a new schedule builder with start and end dates.

        Parameters
        ----------
        start : datetime.date
            Schedule start date.
        end : datetime.date
            Schedule end date.

        Raises
        ------
        ValueError
            If *start* >= *end*.
        """
        ...

    def frequency(self, freq: Union[Tenor, str]) -> ScheduleBuilder:
        """
        Set the coupon/roll frequency.

        Parameters
        ----------
        freq : Tenor | str
            Tenor object or string like ``"3M"``.

        Returns
        -------
        ScheduleBuilder
            Result of frequency for this `ScheduleBuilder` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def stub_rule(self, stub: StubKind) -> ScheduleBuilder:
        """
        Set the stub rule.

        Parameters
        ----------
        stub : StubKind
            Stub positioning rule.

        Returns
        -------
        ScheduleBuilder
            Result of stub rule for this `ScheduleBuilder` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def adjust_with(self, convention: BusinessDayConvention, calendar_id: str) -> ScheduleBuilder:
        """
        Set the business-day convention and calendar for adjustment.

        Parameters
        ----------
        convention : BusinessDayConvention
            Business-day convention.
        calendar_id : str
            Calendar identifier (e.g. ``"target2"``).

        Returns
        -------
        ScheduleBuilder
            Result of adjust with for this `ScheduleBuilder` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
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
            Result of end of month for this `ScheduleBuilder` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def cds_imm(self) -> ScheduleBuilder:
        """
        Enable CDS IMM date mode and disable standard IMM mode.
        Returns
        -------
        ScheduleBuilder
            This builder.
        """
        ...

    def imm(self) -> ScheduleBuilder:
        """
        Enable standard IMM date mode and disable CDS IMM mode.
        Returns
        -------
        ScheduleBuilder
            This builder.
        """
        ...

    def error_policy(self, policy: ScheduleErrorPolicy) -> ScheduleBuilder:
        """
        Set the error policy.

        Setting a policy fully replaces any previous policy; calls are
        order-independent and idempotent.

        Parameters
        ----------
        policy : ScheduleErrorPolicy
            Error handling policy.

        Returns
        -------
        ScheduleBuilder
            Result of error policy for this `ScheduleBuilder` in the annotated representation.

        Raises
        ------
        ValueError
            If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
        """
        ...

    def build(self) -> Schedule:
        """
        Build the schedule.

        Under the default ``STRICT`` policy any build warnings raise
        ``ValueError``. Under ``MISSING_CALENDAR_WARNING`` or
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

# ---------------------------------------------------------------------------
# Free functions
# ---------------------------------------------------------------------------

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

        Result of create date for the binding in the annotated representation.
    Raises
    ------
    ValueError
        If the date components are invalid.

    Examples
    --------
    >>> from finstack_quant.core.dates import create_date
    >>> callable(create_date)
    True
    """
    ...

def days_since_epoch(date: datetime.date) -> int:
    """
    Return the number of days since the Unix epoch (1970-01-01).

    Parameters
    ----------
    date : datetime.date
        Input date.

    Returns
    -------
    int
        Signed number of days since 1970-01-01.

    Raises
    ------
    ValueError
        If supplied inputs violate the documented type, shape, finite-value, or domain constraints.

    Examples
    --------
    >>> from finstack_quant.core.dates import days_since_epoch
    >>> callable(days_since_epoch)
    True
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

        Result of date from epoch days for the binding in the annotated representation.
    Raises
    ------
    ValueError
        If *days* is out of the valid date range.

    Examples
    --------
    >>> from finstack_quant.core.dates import date_from_epoch_days
    >>> callable(date_from_epoch_days)
    True
    """
    ...
