"""Cashflow schedules: typed builder, primitives, accrual, aggregation, JSON bridge.

Examples:
>>> import datetime
>>> from finstack_quant.cashflows.primitives import CFKind, CashFlow
>>> from finstack_quant.core.money import Money
>>> CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.FIXED, 0.0).amount.amount
100.0

"""

from finstack_quant.finstack_quant import cashflows as _cashflows

accrual = _cashflows.accrual
aggregation = _cashflows.aggregation
builder = _cashflows.builder
fixings = _cashflows.fixings
primitives = _cashflows.primitives
schema = _cashflows.schema

ScheduleBuildOpts = _cashflows.ScheduleBuildOpts
build_cashflow_schedule = _cashflows.build_cashflow_schedule
build_cashflow_schedule_json = _cashflows.build_cashflow_schedule_json
dated_flows = _cashflows.dated_flows
schedule_from_classified_flows = _cashflows.schedule_from_classified_flows
schedule_from_dated_flows = _cashflows.schedule_from_dated_flows
validate_cashflow_schedule_json = _cashflows.validate_cashflow_schedule_json
dated_flows_json = _cashflows.dated_flows_json

__all__: list[str] = [
    "ScheduleBuildOpts",
    "accrual",
    "aggregation",
    "build_cashflow_schedule",
    "build_cashflow_schedule_json",
    "builder",
    "dated_flows",
    "dated_flows_json",
    "fixings",
    "primitives",
    "schedule_from_classified_flows",
    "schedule_from_dated_flows",
    "schema",
    "validate_cashflow_schedule_json",
]
