"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/dates/date_ext.rs``."""

from __future__ import annotations

import datetime
from typing import Union
from finstack_quant.core.dates._calendar import HolidayCalendar
from finstack_quant.core.dates._periods import FiscalConfig

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
        Signed 32-bit number of business days to move.
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
        If *date* is invalid, the shift exceeds the supported date range,
        or no business day is found within the bounded (100-day) search window.

    OverflowError
        If the offset is outside the signed 32-bit integer range.

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
        Signed 32-bit number of weekdays to move; ``0`` returns *date* unchanged.

    Returns
    -------
    datetime.date
        The shifted weekday.

    Raises
    ------
    ValueError
        If *date* is invalid or the shifted date exceeds the supported calendar range.

    OverflowError
        If the offset is outside the signed 32-bit integer range.

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
        Signed 32-bit number of calendar months (Jan 31 + 1 gives Feb 28/29).

    Returns
    -------
    datetime.date
        The shifted date.

    Raises
    ------
    ValueError
        If *date* is invalid or the shifted date exceeds the supported calendar range.

    OverflowError
        If the offset is outside the signed 32-bit integer range.

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
