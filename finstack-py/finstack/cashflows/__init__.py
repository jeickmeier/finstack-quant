"""Cashflow schedule JSON construction and validation."""

from __future__ import annotations

from finstack.finstack import cashflows as _cashflows

build_cashflow_schedule_json = _cashflows.build_cashflow_schedule_json
build_cashflow_schedule_envelope_json = _cashflows.build_cashflow_schedule_envelope_json
validate_cashflow_schedule_json = _cashflows.validate_cashflow_schedule_json
validate_cashflow_schedule_envelope_json = _cashflows.validate_cashflow_schedule_envelope_json
dated_flows_json = _cashflows.dated_flows_json
accrued_interest_json = _cashflows.accrued_interest_json
bond_from_cashflows_json = _cashflows.bond_from_cashflows_json

__all__: list[str] = [
    "accrued_interest_json",
    "bond_from_cashflows_json",
    "build_cashflow_schedule_envelope_json",
    "build_cashflow_schedule_json",
    "dated_flows_json",
    "validate_cashflow_schedule_envelope_json",
    "validate_cashflow_schedule_json",
]
