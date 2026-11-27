"""
Cashflow schedule construction (typed and JSON), validation, and dated-flow extraction.

Root bindings for ``finstack-quant-cashflows``. Build schedules from a
``CashflowScheduleBuildSpec``, validate canonical payloads, extract dated flows,
and compute accrued interest.

Examples:
>>> import datetime
>>> from finstack_quant.core.cashflow import CFKind, CashFlow
>>> from finstack_quant.core.money import Money
>>> CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.FIXED, 0.0).amount.amount
100.0

"""

from __future__ import annotations

from typing import Any

import datetime

from finstack_quant.cashflows import accrual as accrual
from finstack_quant.cashflows.builder import CashFlowMeta, CashFlowSchedule
from finstack_quant.core.cashflow import CashFlow, CFKind
from finstack_quant.core.dates import DayCount
from finstack_quant.core.market_data import MarketContext
from finstack_quant.core.money import Money
from finstack_quant.cashflows import aggregation as aggregation
from finstack_quant.cashflows import builder as builder
from finstack_quant.cashflows import fixings as fixings
from finstack_quant.cashflows import schema as schema

from finstack_quant.cashflows._schedule_json import (
    ScheduleBuildOpts as ScheduleBuildOpts,
    build_cashflow_schedule as build_cashflow_schedule,
    dated_flows as dated_flows,
    schedule_from_dated_flows as schedule_from_dated_flows,
    schedule_from_classified_flows as schedule_from_classified_flows,
    validate_cashflow_schedule_json as validate_cashflow_schedule_json,
)

__all__ = [
    "ScheduleBuildOpts",
    "accrual",
    "aggregation",
    "build_cashflow_schedule",
    "builder",
    "fixings",
    "dated_flows",
    "schedule_from_classified_flows",
    "schedule_from_dated_flows",
    "schema",
    "validate_cashflow_schedule_json",
]
