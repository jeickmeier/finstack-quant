"""Cashflow schedules: typed builder, primitives, accrual, aggregation, JSON bridge.

Examples:
>>> import datetime
>>> from finstack_quant.cashflows.primitives import CFKind, CashFlow
>>> from finstack_quant.core.money import Money
>>> CashFlow(datetime.date(2025, 6, 15), Money(100.0, "USD"), CFKind.FIXED).amount.amount
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
accrued_interest = _cashflows.accrued_interest

cpr_to_smm = _cashflows.cpr_to_smm
smm_to_cpr = _cashflows.smm_to_cpr
cdr_to_mdr = _cashflows.cdr_to_mdr
mdr_to_cdr = _cashflows.mdr_to_cdr

__all__: list[str] = [
    "ScheduleBuildOpts",
    "accrual",
    "accrued_interest",
    "aggregation",
    "build_cashflow_schedule",
    "build_cashflow_schedule_json",
    "builder",
    "cdr_to_mdr",
    "cpr_to_smm",
    "dated_flows",
    "dated_flows_json",
    "fixings",
    "mdr_to_cdr",
    "primitives",
    "schedule_from_classified_flows",
    "schedule_from_dated_flows",
    "schema",
    "smm_to_cpr",
    "validate_cashflow_schedule_json",
]
