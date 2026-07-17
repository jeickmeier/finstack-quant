"""Cashflow schedule JSON construction and validation.

Examples:
--------
>>> import finstack_quant.cashflows as cashflows
>>> cashflows.__name__
'finstack_quant.cashflows'
"""

from __future__ import annotations

from finstack_quant.finstack_quant import cashflows as _cashflows

build_cashflow_schedule_json = _cashflows.build_cashflow_schedule_json
validate_cashflow_schedule_json = _cashflows.validate_cashflow_schedule_json
dated_flows_json = _cashflows.dated_flows_json
accrued_interest_json = _cashflows.accrued_interest_json

for _name in (
    "accrued_interest_json",
    "build_cashflow_schedule_json",
    "dated_flows_json",
    "validate_cashflow_schedule_json",
):
    globals()[_name].__module__ = __name__

__all__: list[str] = [
    "accrued_interest_json",
    "build_cashflow_schedule_json",
    "dated_flows_json",
    "validate_cashflow_schedule_json",
]
