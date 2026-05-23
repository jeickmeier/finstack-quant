"""Covenant package JSON validation, templates, and map-backed evaluation."""

from __future__ import annotations

__all__ = [
    "cov_lite",
    "evaluate_engine",
    "lbo_standard",
    "project_finance",
    "real_estate",
    "validate_covenant_engine",
    "validate_covenant_report",
    "validate_covenant_spec",
]

def validate_covenant_spec(spec_json: str) -> str: ...
def validate_covenant_report(report_json: str) -> str: ...
def validate_covenant_engine(engine_json: str) -> str: ...
def evaluate_engine(engine_json: str, metrics_json: str, as_of: str) -> str: ...
def lbo_standard(
    initial_leverage: float,
    interest_coverage: float,
    fixed_charge_coverage: float,
    max_capex: float,
) -> str: ...
def cov_lite(max_leverage: float, max_senior_leverage: float) -> str: ...
def real_estate(min_dscr: float, min_debt_yield: float, max_ltv: float) -> str: ...
def project_finance(
    min_dscr: float,
    distribution_lockup_dscr: float,
    min_liquidity: float,
    max_net_leverage: float,
) -> str: ...
