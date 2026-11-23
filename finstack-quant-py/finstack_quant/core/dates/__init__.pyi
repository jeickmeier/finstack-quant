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
    next_sifma_settlement as next_sifma_settlement,
    sifma_settlement_date as sifma_settlement_date,
)
from finstack_quant.core.dates._daycount import (
    DayCount as DayCount,
    DayCountContext as DayCountContext,
    VolatilityClock as VolatilityClock,
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
    GridMode as GridMode,
    Schedule as Schedule,
    ScheduleBuilder as ScheduleBuilder,
    StubKind as StubKind,
)
from finstack_quant.core.dates._imm import (
    is_cds_date as is_cds_date,
    is_imm_date as is_imm_date,
    next_cds_date as next_cds_date,
    next_imm as next_imm,
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

from finstack_quant.core.dates._epoch import (
    create_date as create_date,
    days_since_epoch as days_since_epoch,
    date_from_epoch_days as date_from_epoch_days,
)

__all__ = [
    # FX settlement
    # day-count
    "DayCount",
    "DayCountContext",
    "VolatilityClock",
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
    "GridMode",
    "Schedule",
    "ScheduleBuilder",
    # SIFMA settlements
    "SifmaSettlementClass",
    "sifma_settlement_date",
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
