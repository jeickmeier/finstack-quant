# finstack-quant-py/tests/test_reporting_scenario.py
from __future__ import annotations

import datetime as dt
from typing import Any

import pytest

from finstack_quant.reporting import scenario_tearsheet
from finstack_quant.reporting.document import TearSheet

_TORNADO = [
    {"parameter_id": "Revenue", "downside": -30.0, "upside": 40.0},
    {"parameter_id": "Margin", "downside": -15.0, "upside": 18.0},
]
_SCENARIOS = {"base": 31.5, "upside": 38.0, "downside": 24.0}
_MC: dict[str, Any] = {
    "periods": ["2025Q3", "2025Q4", "2026Q1"],
    "p_low": [18.0, 17.0, 16.0],
    "p_mid": [22.0, 23.0, 24.0],
    "p_high": [26.0, 28.0, 30.0],
}
_VARIANCE = {
    "rows": [
        {
            "period": "2025Q3",
            "metric": "ebitda",
            "baseline": 34.0,
            "comparison": 31.5,
            "abs_var": -2.5,
            "pct_var": -0.0735,
        },
    ]
}


def test_scenario_tearsheet_renders_all_sections() -> None:
    ts = scenario_tearsheet(
        tornado=_TORNADO,
        scenarios=_SCENARIOS,
        monte_carlo=_MC,
        variance=_VARIANCE,
        breach_probability=0.12,
        target_metric="ebitda",
        generated=dt.date(2026, 6, 22),
    )
    assert isinstance(ts, TearSheet)
    html = ts.to_html()
    assert "Driver Sensitivity" in html
    assert "Revenue" in html
    assert "Scenario Comparison" in html
    assert "upside" in html
    assert "Monte Carlo Distribution" in html
    assert "Breach" in html  # breach KPI label
    assert "Variance vs Baseline" in html
    assert "Median" in html  # P50 KPI label


def test_scenario_tearsheet_all_optional_absent_still_builds() -> None:
    html = scenario_tearsheet(generated=dt.date(2026, 6, 22)).to_html()
    assert "Scenario &amp; Sensitivity" in html  # default title (escaped &)
    assert "Driver Sensitivity" not in html
    assert "Scenario Comparison" not in html
    assert "Monte Carlo Distribution" not in html


def test_scenario_tearsheet_deterministic() -> None:
    kw = {"tornado": _TORNADO, "scenarios": _SCENARIOS, "monte_carlo": _MC, "generated": dt.date(2026, 6, 22)}
    assert scenario_tearsheet(**kw).to_html() == scenario_tearsheet(**kw).to_html()


def test_scenario_tearsheet_rejects_unknown_section() -> None:
    with pytest.raises(ValueError, match="unknown section"):
        scenario_tearsheet(tornado=_TORNADO, sections=["tornado", "nope"])
