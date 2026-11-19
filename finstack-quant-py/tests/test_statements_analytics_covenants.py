"""Covenant evaluation and breach forecasting over statement results."""

from __future__ import annotations

import json

import pytest

from finstack_quant import statements
from finstack_quant.covenants import CovenantEngine, CovenantForecastConfig
from finstack_quant.statements import StatementResult
from finstack_quant.statements_analytics import evaluate_covenants, forecast_breaches


def _spec(covenant_type: dict[str, object], label: str, metric_id: str) -> dict[str, object]:
    return {
        "covenant": {
            "covenant_type": covenant_type,
            "test_frequency": {"count": 3, "unit": "months"},
            "consequences": [],
            "is_active": True,
            "scope": "maintenance",
            "label": label,
        },
        "metric_id": metric_id,
    }


COVERAGE = _spec({"min_interest_coverage": {"threshold": 3.0}}, "min_interest_coverage", "interest_coverage")
LEVERAGE = _spec({"max_debt_to_ebitda": {"threshold": 3.5}}, "max_debt_to_ebitda", "leverage")


def _engine(window_start: str) -> CovenantEngine:
    """Coverage tested throughout; leverage added by a window from ``window_start``."""
    return CovenantEngine.from_json(
        json.dumps({
            "specs": [COVERAGE],
            "windows": [{"start": window_start, "end": "2024-12-31", "covenants": [LEVERAGE, COVERAGE]}],
        })
    )


def _results() -> StatementResult:
    """Quarterly model whose LTM leverage is NaN until four quarters exist (2024Q4)."""
    b = statements.ModelBuilder("borrower")
    b.periods("2024Q1..Q4", None)
    quarters = ["2024Q1", "2024Q2", "2024Q3", "2024Q4"]
    b.value("ebitda", list(zip(quarters, [25.0, 25.0, 25.0, 25.0], strict=True)))
    b.value("interest_expense", list(zip(quarters, [5.0, 5.0, 5.0, 10.0], strict=True)))
    b.value("total_debt", [(q, 300.0) for q in quarters])
    b.compute("ltm_ebitda", "ttm(ebitda)")
    b.compute("leverage", "total_debt / ltm_ebitda")
    b.compute("interest_coverage", "ebitda / interest_expense")
    return statements.Evaluator().evaluate(b.build())


def test_window_keeps_early_non_finite_leverage_out_of_the_evaluation() -> None:
    results = _results()
    assert results.get("leverage", "2024Q3") is None
    assert results.get_cell("leverage", "2024Q3")["status"] == "unavailable"

    frame = evaluate_covenants(_engine("2024-10-01"), results)

    assert frame["as_of"].unique().tolist() == ["2024-03-31", "2024-06-30", "2024-09-30", "2024-12-31"]
    leverage = frame[frame["covenant"] == "max_debt_to_ebitda"]
    assert leverage[["as_of", "passed", "actual_value"]].values.tolist() == [["2024-12-31", True, 3.0]]
    coverage = frame[frame["covenant"] == "min_interest_coverage"]
    assert coverage["passed"].tolist() == [True, True, True, False]


def test_json_inputs_and_engine_series_agree() -> None:
    results = _results()
    engine = _engine("2024-10-01")
    from_json = evaluate_covenants(engine, results.to_json())
    assert from_json.equals(evaluate_covenants(engine, results))


def test_required_non_finite_metric_raises_value_error() -> None:
    results = _results()
    with pytest.raises(ValueError, match="leverage"):
        evaluate_covenants(_engine("2024-07-01"), results)
    with pytest.raises(ValueError, match="leverage"):
        forecast_breaches(_engine("2024-07-01"), results)


def test_missing_required_metric_raises_key_error() -> None:
    results = _results()
    engine = CovenantEngine.from_json(
        json.dumps({"specs": [_spec({"min_dscr": {"threshold": 1.2}}, "min_dscr", "dscr")]})
    )
    with pytest.raises(KeyError, match="dscr"):
        evaluate_covenants(engine, results)
    with pytest.raises(KeyError, match="dscr"):
        forecast_breaches(engine, results)


def test_forecast_breaches_reads_the_same_periods() -> None:
    results = _results()
    breaches = forecast_breaches(_engine("2024-10-01"), results)
    assert [(b.covenant_id, b.breach_date.isoformat(), b.breach_probability) for b in breaches] == [
        ("min_interest_coverage", "2024-12-31", 1.0)
    ]
    assert breaches == forecast_breaches(_engine("2024-10-01"), results, CovenantForecastConfig())


def test_aliases_apply_to_assessment_and_forecast() -> None:
    results = _results()
    engine = _engine("2024-07-01")
    aliases = {"leverage": "interest_coverage"}
    frame = evaluate_covenants(engine, results, aliases)
    leverage = frame[frame["covenant"] == "max_debt_to_ebitda"]
    assert leverage[["actual_value", "headroom_raw"]].values.tolist() == [[5.0, -1.5], [2.5, 1.0]]

    breaches = forecast_breaches(engine, results, node_mapping=aliases)
    assert [(b.covenant_id, b.headroom_raw) for b in breaches] == [
        ("max_debt_to_ebitda", -1.5),
        ("min_interest_coverage", -0.5),
    ]
