"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/dates/calendar.rs``."""

from __future__ import annotations

import datetime
from typing import Optional, Union

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
    >>> BusinessDayConvention.from_name(" MODIFIED_FOLLOWING ") == BusinessDayConvention.MODIFIED_FOLLOWING
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
            Canonical snake_case name (``"following"``, ``"modified_following"``),
            ignoring surrounding whitespace and ASCII case.

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
        (for example ``"modified_following"``), ignoring outer whitespace and ASCII case.
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
