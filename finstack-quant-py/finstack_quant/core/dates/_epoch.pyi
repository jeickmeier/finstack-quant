"""Stub declarations mirroring ``finstack-quant-py/src/bindings/core/dates/epoch.rs``."""

from __future__ import annotations

import datetime
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates._sifma import SifmaSettlementClass, next_sifma_settlement, sifma_settlement_date
from finstack_quant.core.dates._daycount import DayCount, DayCountContext, VolatilityClock
from finstack_quant.core.dates._tenor import Tenor, TenorUnit
from finstack_quant.core.dates._periods import (
    FiscalConfig,
    Period,
    PeriodId,
    PeriodKind,
    PeriodPlan,
    build_fiscal_periods,
    build_periods,
)
from finstack_quant.core.dates._calendar import (
    BusinessDayConvention,
    CalendarMetadata,
    HolidayCalendar,
    adjust,
    available_calendars,
)
from finstack_quant.core.dates._schedule import GridMode, Schedule, ScheduleBuilder, StubKind
from finstack_quant.core.dates._imm import (
    is_cds_date,
    is_imm_date,
    next_cds_date,
    next_imm,
    next_semiannual_cds_maturity,
    next_third_friday,
    prev_cds_date,
    prev_cds_semiannual_roll,
    third_friday,
    third_wednesday,
)
from finstack_quant.core.dates._date_ext import (
    add_business_days,
    add_months,
    add_weekdays,
    end_of_month,
    fiscal_year,
    is_weekend,
    months_until,
    quarter,
)

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
