"""Cashflow schedule JSON construction and validation."""

from __future__ import annotations

__all__ = [
    "accrued_interest_json",
    "bond_from_cashflows_json",
    "build_cashflow_schedule_envelope_json",
    "build_cashflow_schedule_json",
    "dated_flows_json",
    "validate_cashflow_schedule_envelope_json",
    "validate_cashflow_schedule_json",
]

def build_cashflow_schedule_envelope_json(spec_json: str, market_json: str | None = None) -> str: ...
def build_cashflow_schedule_json(spec_json: str, market_json: str | None = None) -> str: ...
def validate_cashflow_schedule_envelope_json(envelope_json: str) -> str: ...
def validate_cashflow_schedule_json(schedule_json: str) -> str: ...
def dated_flows_json(schedule_json: str) -> str: ...
def accrued_interest_json(schedule_json: str, as_of: str, config_json: str | None = None) -> float:
    """Return accrued interest as a host-language ``float``.

    The Rust engine computes from the canonical schedule and crosses the
    binding boundary as ``f64``. For large notionals, compare with an absolute
    tolerance scaled to the schedule notional rather than expecting decimal
    string equality.
    """

def bond_from_cashflows_json(
    instrument_id: str,
    schedule_json: str,
    discount_curve_id: str,
    quoted_clean: float | None = None,
) -> str: ...
