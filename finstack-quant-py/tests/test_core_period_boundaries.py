"""Public period builders reject unsafe ranges before enumeration."""

from datetime import date

import pytest

from finstack_quant.core.dates import FiscalConfig, build_fiscal_periods, build_periods


@pytest.mark.parametrize(
    "spec",
    [
        "1..2147483647",
        "2147483647..2147483647",
        "9999D365..D365",
        "9999W52..W52",
        "2025D1..FY2025D2",
        "2025Q1..FY2025Q2",
        "1D1..9998D365",
    ],
)
def test_calendar_period_boundaries_raise_value_error(spec: str) -> None:
    with pytest.raises(ValueError, match=r"(?i)period|date|year|range"):
        build_periods(spec)


@pytest.mark.parametrize("spec", ["FY2147483647..FY2147483647", "FY1D1..FY9999D365"])
def test_fiscal_period_boundaries_raise_value_error(spec: str) -> None:
    with pytest.raises(ValueError, match=r"(?i)period|date|year|range"):
        build_fiscal_periods(spec, FiscalConfig.us_federal())


def test_terminal_representable_daily_period_succeeds() -> None:
    periods = build_periods("9999D364..D364").periods
    assert len(periods) == 1
    assert periods[0].end == date.max
