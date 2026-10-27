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
from finstack_quant.core.currency import Currency
from finstack_quant.core.dates._sifma import (
    SifmaSettlementClass as SifmaSettlementClass,
    estimated_sifma_settlement_date as estimated_sifma_settlement_date,
    next_sifma_settlement as next_sifma_settlement,
    sifma_settlement_date as sifma_settlement_date,
)
from finstack_quant.core.dates._daycount import (
    DayCount as DayCount,
    DayCountContext as DayCountContext,
    Thirty360Convention as Thirty360Convention,
    days_30_360 as days_30_360,
    days_30e_360_isda as days_30e_360_isda,
)
from finstack_quant.core.dates._tenor import Tenor as Tenor, TenorUnit as TenorUnit
from finstack_quant.core.dates._periods import (
    FiscalConfig as FiscalConfig,
    Period as Period,
    PeriodId as PeriodId,
    PeriodKind as PeriodKind,
    PeriodPlan as PeriodPlan,
    build_fiscal_periods as build_fiscal_periods,
    build_periods as build_periods,
)
from finstack_quant.core.dates._calendar import (
    BusinessDayConvention as BusinessDayConvention,
    CalendarMetadata as CalendarMetadata,
    HolidayCalendar as HolidayCalendar,
    adjust as adjust,
    available_calendars as available_calendars,
)
from finstack_quant.core.dates._schedule import (
    Schedule as Schedule,
    ScheduleBuilder as ScheduleBuilder,
    ScheduleErrorPolicy as ScheduleErrorPolicy,
    StubKind as StubKind,
)
from finstack_quant.core.dates._imm import (
    imm_option_expiry as imm_option_expiry,
    is_cds_date as is_cds_date,
    is_imm_date as is_imm_date,
    next_cds_date as next_cds_date,
    next_imm as next_imm,
    next_imm_option_expiry as next_imm_option_expiry,
    next_semiannual_cds_maturity as next_semiannual_cds_maturity,
    next_third_friday as next_third_friday,
    prev_cds_date as prev_cds_date,
    prev_cds_semiannual_roll as prev_cds_semiannual_roll,
    third_friday as third_friday,
    third_wednesday as third_wednesday,
)
from finstack_quant.core.dates._date_ext import (
    add_business_days as add_business_days,
    add_months as add_months,
    add_weekdays as add_weekdays,
    end_of_month as end_of_month,
    fiscal_year as fiscal_year,
    is_weekend as is_weekend,
    months_until as months_until,
    quarter as quarter,
)

__all__ = [
    # FX settlement
    "fx_standard_settlement_days",
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
    "estimated_sifma_settlement_date",
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
    "next_third_friday",
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

# Free functions

def fx_standard_settlement_days(base: Currency | str, quote: Currency | str) -> int:
    """Return the market-standard FX spot lag in business days for a currency pair.

    Parameters
    ----------
    base : Currency | str
        Base (foreign) currency as a Currency or ISO-4217 alphabetic code.
        Codes are case-insensitive and must not contain surrounding whitespace.
    quote : Currency | str
        Quote (domestic) currency, accepted in the same forms as base.
        Reversing the pair preserves its settlement lag.

    Returns
    -------
    int
        One business day for USD/CAD, USD/TRY, USD/PHP and USD/RUB in either
        order; two business days otherwise, including USD/MXN and EUR/CAD.
        This reports the standard lag without applying holiday calendars or
        an instrument's explicit settlement-lag override.

    Raises
    ------
    TypeError
        If either currency is neither a Currency nor a string.
    ValueError
        If a string is not a recognized ISO-4217 alphabetic currency code.

    Examples
    --------
    >>> from finstack_quant.core.currency import Currency
    >>> from finstack_quant.core.dates import fx_standard_settlement_days
    >>> fx_standard_settlement_days("USD", "CAD")
    1
    >>> fx_standard_settlement_days(Currency("EUR"), "USD")
    2
    """
    ...

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
