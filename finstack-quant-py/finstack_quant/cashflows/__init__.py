"""Cashflow schedule construction (typed and JSON), validation, and dated-flow extraction.

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

from finstack_quant.finstack_quant import cashflows as _cashflows

accrual = _cashflows.accrual
aggregation = _cashflows.aggregation
builder = _cashflows.builder
fixings = _cashflows.fixings
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
    "schedule_from_classified_flows",
    "schedule_from_dated_flows",
    "schema",
    "validate_cashflow_schedule_json",
]
