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


def test_scenario_tearsheet_tolerates_bad_inputs() -> None:
    html = scenario_tearsheet(
        scenarios={"base": "N/A", "x": None},
        monte_carlo={"periods": ["Q1", "Q2"], "p_low": [1.0], "p_mid": [2.0], "p_high": [3.0]},
        variance={"rows": [None, "bad"]},
        generated=dt.date(2026, 6, 22),
    ).to_html()
    assert "Driver Sensitivity" not in html
    assert "Scenario Comparison" not in html
    assert "Monte Carlo Distribution" not in html
    assert "Variance vs Baseline" not in html


def test_scenario_tearsheet_excluding_montecarlo_drops_kpis() -> None:
    html = scenario_tearsheet(
        tornado=_TORNADO, sections=["tornado"], monte_carlo=_MC, generated=dt.date(2026, 6, 22)
    ).to_html()
    assert "Driver Sensitivity" in html
    assert "Median" not in html  # MC KPIs gated out


def test_scenario_tornado_keeps_rust_swing_order() -> None:
    """PYPY-005: rows keep the |upside - downside| order Rust returns."""
    entries = [
        {"parameter_id": "margin", "downside": -5.0, "upside": 5.0},  # swing 10
        {"parameter_id": "capex", "downside": -10.0, "upside": -8.0},  # swing 2
    ]
    body = scenario_tearsheet(tornado=entries, sections=["tornado"]).sections[0].body
    assert body.index("margin") < body.index("capex")


def _typed_tornado() -> list[Any]:
    from finstack_quant.statements import ModelBuilder
    from finstack_quant.statements_analytics import (
        ParameterSpec,
        SensitivityConfig,
        generate_tornado_entries,
        run_sensitivity,
    )

    b = ModelBuilder("m")
    b.periods("2025Q1..Q2", None)
    b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    b.compute("profit", "revenue * 0.5")
    spec = ParameterSpec.with_percentages("revenue", "2025Q2", 110.0, [-10.0, 10.0])
    cfg = SensitivityConfig("diagonal", [spec], ["profit"])
    return generate_tornado_entries(run_sensitivity(b.build(), cfg), "profit", "2025Q2")


def _typed_variance() -> Any:
    from finstack_quant.statements import Evaluator, ModelBuilder
    from finstack_quant.statements_analytics import VarianceConfig, run_variance

    def model(revenue: float) -> Any:
        b = ModelBuilder("m")
        b.periods("2025Q1..Q1", None)
        b.value("revenue", [("2025Q1", revenue)])
        b.compute("profit", "revenue * 0.5")
        return Evaluator().evaluate(b.build())

    return run_variance(model(100.0), model(120.0), VarianceConfig("base", "actual", ["profit"], ["2025Q1"]))


def test_scenario_tearsheet_renders_typed_rust_results() -> None:
    """PYPY-006: typed TornadoEntry / VarianceReport render instead of being dropped."""
    sheet = scenario_tearsheet(tornado=_typed_tornado(), variance=_typed_variance(), sections=["tornado", "variance"])
    assert [s.title for s in sheet.sections] == ["Driver Sensitivity", "Variance vs Baseline"]
    assert "revenue" in sheet.sections[0].body


@pytest.mark.parametrize("tornado", [["bad"], [None], {"parameter_id": "x"}])
def test_scenario_tearsheet_rejects_unsupported_tornado_input(tornado: Any) -> None:
    with pytest.raises((TypeError, ValueError)):
        scenario_tearsheet(tornado=tornado, sections=["tornado"])
