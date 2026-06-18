"""Date, calendar, and schedule utilities from ``finstack-quant-core``.

Provides day-count conventions, tenor types, period generation, schedule
building, holiday calendars, and business-day adjustment functions.

Example::

    >>> import datetime
    >>> from finstack_quant.core.dates import DayCount, Tenor, ScheduleBuilder
    >>> dc = DayCount.ACT_365F
    >>> dc.year_fraction(datetime.date(2024, 1, 1), datetime.date(2025, 1, 1))
    1.0
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
    # free functions
    "create_date",
    "days_since_epoch",
    "date_from_epoch_days",
]

# ---------------------------------------------------------------------------
# Day-count conventions
# ---------------------------------------------------------------------------

class DayCount:
    """Day-count convention for year-fraction calculations.

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
    """Actual/365 Leap (AFB)."""
    THIRTY_360: DayCount
    """30/360 US (Bond Basis)."""
    THIRTY_E_360: DayCount
    """30E/360 (Eurobond Basis)."""
    ACT_ACT: DayCount
    """Actual/Actual (ISDA)."""
    ACT_ACT_ISMA: DayCount
    """Actual/Actual (ICMA/ISMA)."""
    BUS_252: DayCount
    """Business/252 (Brazilian market convention)."""

    @classmethod
    def from_name(cls, name: str) -> DayCount:
        """Parse a day-count convention from its string name.

        Parameters
        ----------
        name : str
            Convention identifier (e.g. ``"act_360"``, ``"act_365f"``,
            ``"thirty_360"``, ``"bus_252"``).

        Returns
        -------
        DayCount

        Raises
        ------
        ValueError
            If *name* is not recognised.
        """
        ...

    def year_fraction(
        self,
        start: datetime.date,
        end: datetime.date,
        ctx: Optional[DayCountContext] = None,
    ) -> float:
        """Compute the year fraction between two dates.

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
        """Compute the signed year fraction (negative when start > end).

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
        """Count the calendar days between two dates.

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
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class DayCountContext:
    """Optional context for day-count calculations.

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
    """

    def __init__(
        self,
        calendar_id: Optional[str] = None,
        frequency: Optional[Tenor] = None,
        bus_basis: Optional[int] = None,
        coupon_period: Optional[tuple[datetime.date, datetime.date]] = None,
    ) -> None:
        """Create a day-count context.

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
        """
        ...

    @property
    def calendar_id(self) -> Optional[str]:
        """Optional calendar identifier.

        Returns
        -------
        str | None
        """
        ...

    @property
    def frequency(self) -> Optional[Tenor]:
        """Optional coupon frequency.

        Returns
        -------
        Tenor | None
        """
        ...

    @property
    def bus_basis(self) -> Optional[int]:
        """Optional custom business-day divisor.

        Returns
        -------
        int | None
        """
        ...

    @property
    def coupon_period(self) -> Optional[tuple[datetime.date, datetime.date]]:
        """Optional reference coupon period as ``(start, end)`` dates.

        Returns
        -------
        tuple[datetime.date, datetime.date] | None
        """
        ...

    def to_state(self) -> DayCountContextState:
        """Convert to a serializable state snapshot.

        Returns
        -------
        DayCountContextState
        """
        ...

    def __repr__(self) -> str: ...

class DayCountContextState:
    """Serializable snapshot of :class:`DayCountContext` for persistence.

    Parameters
    ----------
    calendar_id : str | None
        Calendar identifier.
    frequency : Tenor | None
        Coupon frequency.
    bus_basis : int | None
        Custom business-day divisor.
    """

    def __init__(
        self,
        calendar_id: Optional[str] = None,
        frequency: Optional[Tenor] = None,
        bus_basis: Optional[int] = None,
        coupon_period: Optional[tuple[datetime.date, datetime.date]] = None,
    ) -> None:
        """Create a context state.

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
        """
        ...

    def to_context(self) -> DayCountContext:
        """Reconstruct a live :class:`DayCountContext` from this state.

        Returns
        -------
        DayCountContext
        """
        ...

    @property
    def calendar_id(self) -> Optional[str]:
        """Optional calendar identifier.

        Returns
        -------
        str | None
        """
        ...

    @property
    def frequency(self) -> Optional[Tenor]:
        """Optional coupon frequency.

        Returns
        -------
        Tenor | None
        """
        ...

    @property
    def bus_basis(self) -> Optional[int]:
        """Optional custom business-day divisor.

        Returns
        -------
        int | None
        """
        ...

    @property
    def coupon_period(self) -> Optional[tuple[datetime.date, datetime.date]]:
        """Optional reference coupon period as ``(start, end)`` dates.

        Returns
        -------
        tuple[datetime.date, datetime.date] | None
        """
        ...

    def __repr__(self) -> str: ...

class Thirty360Convention:
    """30/360 sub-convention (US SIA / Bond Basis, ISDA, or European).

    Immutable, hashable enum-style type.
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
    """Frequency/tenor unit enumeration.

    Immutable, hashable enum-style type.
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
        """Parse a single-character tenor unit designator.

        Parameters
        ----------
        ch : str
            One of ``'D'``, ``'W'``, ``'M'``, ``'Y'`` (case-sensitive).

        Returns
        -------
        TenorUnit

        Raises
        ------
        ValueError
            If *ch* is not a valid unit designator.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class Tenor:
    """A tenor such as ``3M``, ``1Y``, or ``2W``.

    Immutable, hashable value type combining a count and unit.

    Parameters
    ----------
    count : int
        Numeric count (e.g. ``3``).
    unit : TenorUnit
        Unit (e.g. ``TenorUnit.MONTHS``).
    """

    def __init__(self, count: int, unit: TenorUnit) -> None:
        """Construct a tenor from a count and unit.

        Parameters
        ----------
        count : int
            Numeric count.
        unit : TenorUnit
            Tenor unit.
        """
        ...

    @classmethod
    def parse(cls, s: str) -> Tenor:
        """Parse a tenor string.

        Parameters
        ----------
        s : str
            Tenor string (e.g. ``"3M"``, ``"1Y"``, ``"2W"``).

        Returns
        -------
        Tenor

        Raises
        ------
        ValueError
            If *s* cannot be parsed.
        """
        ...

    @classmethod
    def daily(cls) -> Tenor:
        """1-day tenor.

        Returns
        -------
        Tenor
        """
        ...

    @classmethod
    def weekly(cls) -> Tenor:
        """1-week tenor.

        Returns
        -------
        Tenor
        """
        ...

    @classmethod
    def biweekly(cls) -> Tenor:
        """2-week tenor.

        Returns
        -------
        Tenor
        """
        ...

    @classmethod
    def monthly(cls) -> Tenor:
        """1-month tenor.

        Returns
        -------
        Tenor
        """
        ...

    @classmethod
    def bimonthly(cls) -> Tenor:
        """2-month tenor.

        Returns
        -------
        Tenor
        """
        ...

    @classmethod
    def quarterly(cls) -> Tenor:
        """3-month (quarterly) tenor.

        Returns
        -------
        Tenor
        """
        ...

    @classmethod
    def semi_annual(cls) -> Tenor:
        """6-month (semi-annual) tenor.

        Returns
        -------
        Tenor
        """
        ...

    @classmethod
    def annual(cls) -> Tenor:
        """12-month (annual) tenor.

        Returns
        -------
        Tenor
        """
        ...

    @classmethod
    def from_payments_per_year(cls, payments: int) -> Tenor:
        """Construct from the number of coupon payments per year.

        Parameters
        ----------
        payments : int
            Payments per year (e.g. ``4`` for quarterly).

        Returns
        -------
        Tenor

        Raises
        ------
        ValueError
            If *payments* does not map to a standard tenor.
        """
        ...

    @property
    def count(self) -> int:
        """Numeric count.

        Returns
        -------
        int
        """
        ...

    @property
    def unit(self) -> TenorUnit:
        """Unit of the tenor.

        Returns
        -------
        TenorUnit
        """
        ...

    @property
    def months(self) -> Optional[int]:
        """Equivalent whole months (``None`` for day/week tenors).

        Returns
        -------
        int | None
        """
        ...

    @property
    def days(self) -> Optional[int]:
        """Equivalent whole days (``None`` for month/year tenors).

        Returns
        -------
        int | None
        """
        ...

    def to_years_simple(self) -> float:
        """Approximate tenor length in years (simple estimate, no calendar).

        Returns
        -------
        float
        """
        ...

    def to_days_approx(self) -> int:
        """Approximate tenor length in calendar days.

        Returns
        -------
        int
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
    """Period frequency kind.

    Immutable, hashable enum-style type.
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
        """Parse a period kind from a string.

        Parameters
        ----------
        name : str
            Period kind identifier (e.g. ``"quarterly"``, ``"m"``, ``"annual"``).

        Returns
        -------
        PeriodKind

        Raises
        ------
        ValueError
            If *name* is not recognised.
        """
        ...

    @property
    def periods_per_year(self) -> int:
        """Number of periods per year for this frequency.

        Returns
        -------
        int
        """
        ...

    @property
    def annualization_factor(self) -> float:
        """Annualization factor for this frequency.

        Returns
        -------
        float
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class PeriodId:
    """A period identifier such as ``2025Q1`` or ``2025M03``.

    Immutable, hashable value type.
    """

    @classmethod
    def parse(cls, code: str) -> PeriodId:
        """Parse a period code string.

        Parameters
        ----------
        code : str
            Period code (e.g. ``"2025Q1"``, ``"2025M06"``).

        Returns
        -------
        PeriodId

        Raises
        ------
        ValueError
            If *code* cannot be parsed.
        """
        ...

    @classmethod
    def month(cls, year: int, month: int) -> PeriodId:
        """Build a monthly period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        month : int
            Month (1-12).

        Returns
        -------
        PeriodId
        """
        ...

    @classmethod
    def quarter(cls, year: int, quarter: int) -> PeriodId:
        """Build a quarterly period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        quarter : int
            Quarter (1-4).

        Returns
        -------
        PeriodId
        """
        ...

    @classmethod
    def annual(cls, year: int) -> PeriodId:
        """Build an annual period identifier.

        Parameters
        ----------
        year : int
            Calendar year.

        Returns
        -------
        PeriodId
        """
        ...

    @classmethod
    def half(cls, year: int, half: int) -> PeriodId:
        """Build a semi-annual period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        half : int
            Half (1 or 2).

        Returns
        -------
        PeriodId
        """
        ...

    @classmethod
    def week(cls, year: int, week: int) -> PeriodId:
        """Build a weekly period identifier.

        Parameters
        ----------
        year : int
            Calendar year.
        week : int
            ISO week number (1-53).

        Returns
        -------
        PeriodId
        """
        ...

    @classmethod
    def day(cls, year: int, ordinal: int) -> PeriodId:
        """Build a daily period identifier from an ordinal day.

        Parameters
        ----------
        year : int
            Calendar year.
        ordinal : int
            Ordinal day of the year (1-366).

        Returns
        -------
        PeriodId
        """
        ...

    @property
    def code(self) -> str:
        """Period code string (e.g. ``"2025Q1"``).

        Returns
        -------
        str
        """
        ...

    @property
    def year(self) -> int:
        """Gregorian calendar year.

        Returns
        -------
        int
        """
        ...

    @property
    def index(self) -> int:
        """Ordinal index within the year.

        Returns
        -------
        int
        """
        ...

    @property
    def kind(self) -> PeriodKind:
        """Kind (frequency) of this period.

        Returns
        -------
        PeriodKind
        """
        ...

    @property
    def periods_per_year(self) -> int:
        """Number of periods per year for this kind.

        Returns
        -------
        int
        """
        ...

    def next(self) -> PeriodId:
        """Next period in sequence.

        Returns
        -------
        PeriodId

        Raises
        ------
        ValueError
            If the next period overflows.
        """
        ...

    def prev(self) -> PeriodId:
        """Previous period in sequence.

        Returns
        -------
        PeriodId

        Raises
        ------
        ValueError
            If the previous period underflows.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class Period:
    """A concrete period with start/end dates and an actual/forecast flag.

    Immutable value type returned by period-building functions.
    """

    @property
    def id(self) -> PeriodId:
        """Period identifier.

        Returns
        -------
        PeriodId
        """
        ...

    @property
    def start(self) -> datetime.date:
        """Inclusive start date.

        Returns
        -------
        datetime.date
        """
        ...

    @property
    def end(self) -> datetime.date:
        """Exclusive end date.

        Returns
        -------
        datetime.date
        """
        ...

    @property
    def is_actual(self) -> bool:
        """Whether this period is an actual (vs forecast).

        Returns
        -------
        bool
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

class PeriodPlan:
    """A plan containing a contiguous sequence of periods.

    Returned by :func:`build_periods` and :func:`build_fiscal_periods`.
    """

    @property
    def periods(self) -> list[Period]:
        """List of periods in ascending order.

        Returns
        -------
        list[Period]
        """
        ...

    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class FiscalConfig:
    """Fiscal year configuration.

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
    """

    def __init__(self, start_month: int, start_day: int) -> None:
        """Create a fiscal configuration from a start month and day.

        Parameters
        ----------
        start_month : int
            Month (1-12).
        start_day : int
            Day (1-31).

        Raises
        ------
        ValueError
            If the combination is invalid.
        """
        ...

    @classmethod
    def calendar_year(cls) -> FiscalConfig:
        """Standard calendar year (January 1).

        Returns
        -------
        FiscalConfig
        """
        ...

    @classmethod
    def us_federal(cls) -> FiscalConfig:
        """US Federal fiscal year (October 1).

        Returns
        -------
        FiscalConfig
        """
        ...

    @classmethod
    def uk(cls) -> FiscalConfig:
        """UK fiscal year (April 6).

        Returns
        -------
        FiscalConfig
        """
        ...

    @classmethod
    def japan(cls) -> FiscalConfig:
        """Japanese fiscal year (April 1).

        Returns
        -------
        FiscalConfig
        """
        ...

    @classmethod
    def canada(cls) -> FiscalConfig:
        """Canadian fiscal year (April 1).

        Returns
        -------
        FiscalConfig
        """
        ...

    @classmethod
    def australia(cls) -> FiscalConfig:
        """Australian fiscal year (July 1).

        Returns
        -------
        FiscalConfig
        """
        ...

    @classmethod
    def germany(cls) -> FiscalConfig:
        """German fiscal year (January 1).

        Returns
        -------
        FiscalConfig
        """
        ...

    @classmethod
    def france(cls) -> FiscalConfig:
        """French fiscal year (January 1).

        Returns
        -------
        FiscalConfig
        """
        ...

    @property
    def start_month(self) -> int:
        """Month when the fiscal year starts (1-12).

        Returns
        -------
        int
        """
        ...

    @property
    def start_day(self) -> int:
        """Day when the fiscal year starts (1-31).

        Returns
        -------
        int
        """
        ...

    def __repr__(self) -> str: ...

def build_periods(
    spec: str,
    actuals_cutoff: Optional[str] = None,
) -> PeriodPlan:
    """Build periods from a range expression.

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
    """
    ...

def build_fiscal_periods(
    spec: str,
    fiscal_config: FiscalConfig,
    actuals_cutoff: Optional[str] = None,
) -> PeriodPlan:
    """Build fiscal periods with a custom fiscal year configuration.

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
    """
    ...

# ---------------------------------------------------------------------------
# Calendar & business-day adjustment
# ---------------------------------------------------------------------------

class BusinessDayConvention:
    """Business-day adjustment convention.

    Immutable, hashable enum-style type.
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
        """Parse from a string.

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
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class CalendarMetadata:
    """Metadata for a holiday calendar.

    Immutable value type.
    """

    @property
    def id(self) -> str:
        """Calendar short code.

        Returns
        -------
        str
        """
        ...

    @property
    def name(self) -> str:
        """Human-readable name.

        Returns
        -------
        str
        """
        ...

    @property
    def ignore_weekends(self) -> bool:
        """Whether weekends are ignored for this calendar.

        Returns
        -------
        bool
        """
        ...

    def __repr__(self) -> str: ...

class HolidayCalendar:
    """A holiday calendar resolved from the global registry.

    Parameters
    ----------
    code : str
        Calendar code (e.g. ``"target2"``, ``"nyse"``).

    Raises
    ------
    ValueError
        If *code* does not match any known calendar.
    """

    def __init__(self, code: str) -> None:
        """Resolve a calendar by its code.

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
        """Check whether a date is a holiday.

        Parameters
        ----------
        date : datetime.date
            The date to check.

        Returns
        -------
        bool
        """
        ...

    def is_business_day(self, date: datetime.date) -> bool:
        """Check whether a date is a business day.

        Parameters
        ----------
        date : datetime.date
            The date to check.

        Returns
        -------
        bool
        """
        ...

    @property
    def metadata(self) -> Optional[CalendarMetadata]:
        """Calendar metadata (if available).

        Returns
        -------
        CalendarMetadata | None
        """
        ...

    @property
    def code(self) -> str:
        """Calendar code.

        Returns
        -------
        str
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

def adjust(
    date: datetime.date,
    convention: Union[BusinessDayConvention, str],
    calendar: Union[HolidayCalendar, str],
) -> datetime.date:
    """Adjust a date according to a business-day convention and calendar.

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
    """
    ...

def available_calendars() -> list[str]:
    """Return the list of available calendar codes in the global registry.

    Returns
    -------
    list[str]
        Calendar code strings.
    """
    ...

# ---------------------------------------------------------------------------
# Schedule
# ---------------------------------------------------------------------------

class StubKind:
    """Stub positioning rule for schedule generation.

    Immutable, hashable enum-style type.
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
        """Parse from a string.

        Parameters
        ----------
        name : str
            Stub kind identifier (e.g. ``"short_front"``, ``"long_back"``).

        Returns
        -------
        StubKind

        Raises
        ------
        ValueError
            If *name* is not recognised.
        """
        ...

    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __ne__(self, other: object) -> bool: ...

class ScheduleErrorPolicy:
    """Error handling policy for schedule building.

    Immutable, hashable enum-style type.
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
    """A generated date schedule.

    Immutable value type produced by :class:`ScheduleBuilder`.
    """

    @property
    def dates(self) -> list[datetime.date]:
        """Schedule dates as a list of ``datetime.date``.

        Returns
        -------
        list[datetime.date]
        """
        ...

    def has_warnings(self) -> bool:
        """Whether any warnings were generated during schedule building.

        Returns
        -------
        bool
        """
        ...

    def used_graceful_fallback(self) -> bool:
        """Whether a graceful fallback was used during schedule building.

        Returns
        -------
        bool
        """
        ...

    @property
    def warnings(self) -> list[str]:
        """Warning messages (if any).

        Returns
        -------
        list[str]
        """
        ...

    def __len__(self) -> int: ...
    def __repr__(self) -> str: ...

class ScheduleBuilder:
    """Builder for constructing date schedules.

    Unlike the Rust ``ScheduleBuilder``, the Python binding mutates the
    builder **in place**: each setter returns ``None`` rather than ``self``.
    Call setters sequentially on the same instance, then call ``build()``.

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
    >>> builder = ScheduleBuilder(date(2025, 1, 15), date(2030, 1, 15))
    >>> builder.frequency("3M")
    >>> builder.stub_rule(StubKind.SHORT_FRONT)
    >>> builder.adjust_with(BusinessDayConvention.MODIFIED_FOLLOWING, "usny")
    >>> builder.end_of_month(False)
    >>> builder.error_policy(ScheduleErrorPolicy.STRICT)
    >>> schedule = builder.build()
    >>> len(schedule) >= 20
    True
    """

    def __init__(self, start: datetime.date, end: datetime.date) -> None:
        """Start a new schedule builder with start and end dates.

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

    def frequency(self, freq: Union[Tenor, str]) -> None:
        """Set the coupon/roll frequency.

        Parameters
        ----------
        freq : Tenor | str
            Tenor object or string like ``"3M"``.
        """
        ...

    def stub_rule(self, stub: StubKind) -> None:
        """Set the stub rule.

        Parameters
        ----------
        stub : StubKind
            Stub positioning rule.
        """
        ...

    def adjust_with(self, convention: BusinessDayConvention, calendar_id: str) -> None:
        """Set the business-day convention and calendar for adjustment.

        Parameters
        ----------
        convention : BusinessDayConvention
            Business-day convention.
        calendar_id : str
            Calendar identifier (e.g. ``"target2"``).
        """
        ...

    def end_of_month(self, eom: bool) -> None:
        """Enable or disable end-of-month roll logic.

        Parameters
        ----------
        eom : bool
            Whether to enable end-of-month rolling.
        """
        ...

    def cds_imm(self) -> None:
        """Enable CDS IMM date mode.
        Returns
        -------
        None
        """
        ...

    def imm(self) -> None:
        """Enable IMM date mode.
        Returns
        -------
        None
        """
        ...

    def error_policy(self, policy: ScheduleErrorPolicy) -> None:
        """Set the error policy.

        Setting a policy fully replaces any previous policy; calls are
        order-independent and idempotent.

        Parameters
        ----------
        policy : ScheduleErrorPolicy
            Error handling policy.
        """
        ...

    def build(self) -> Schedule:
        """Build the schedule.

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
    """Create a ``datetime.date`` from year, month (1-12), and day.

    Parameters
    ----------
    year : int
        Calendar year.
    month : int
        Month (1-12).
    day : int
        Day of the month.

    Returns
    -------
    datetime.date

    Raises
    ------
    ValueError
        If the date components are invalid.
    """
    ...

def days_since_epoch(date: datetime.date) -> int:
    """Return the number of days since the Unix epoch (1970-01-01).

    Parameters
    ----------
    date : datetime.date
        Input date.

    Returns
    -------
    int
        Signed number of days since 1970-01-01.
    """
    ...

def date_from_epoch_days(days: int) -> datetime.date:
    """Reconstruct a ``datetime.date`` from epoch days (days since 1970-01-01).

    Parameters
    ----------
    days : int
        Number of days since epoch.

    Returns
    -------
    datetime.date

    Raises
    ------
    ValueError
        If *days* is out of the valid date range.
    """
    ...
