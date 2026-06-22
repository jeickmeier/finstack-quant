"""Behavioral tests for the structured credit_assessment binding."""

from __future__ import annotations

from finstack_quant import statements
from finstack_quant.statements import StatementResult
from finstack_quant.statements_analytics import credit_assessment


def _results() -> StatementResult:
    """A StatementResult with a clean 4-quarter TTM window at 2025Q4.

    Built by evaluating a real model (the canonical way to obtain a
    StatementResult; hand-written JSON would omit the required ``meta`` field).
    """
    b = statements.ModelBuilder("credit_demo")
    b.periods("2025Q1..Q4", None)
    b.value("ebitda", [("2025Q1", 10.0), ("2025Q2", 20.0), ("2025Q3", 30.0), ("2025Q4", 40.0)])
    b.value(
        "interest_expense",
        [("2025Q1", 1.0), ("2025Q2", 2.0), ("2025Q3", 3.0), ("2025Q4", 4.0)],
    )
    b.value(
        "total_debt",
        [("2025Q1", 300.0), ("2025Q2", 300.0), ("2025Q3", 300.0), ("2025Q4", 300.0)],
    )
    return statements.Evaluator().evaluate(b.build())


def test_credit_assessment_returns_structured_scalars() -> None:
    # Pass the StatementResult object (exercises the object-input path).
    out = credit_assessment(_results(), "2025Q4")
    assert out["as_of"] == "2025Q4"
    assert out["leverage_ratio"] == 300.0 / 100.0  # debt / TTM EBITDA(=100)
    assert out["interest_coverage"] == 100.0 / 10.0  # TTM EBITDA / TTM interest(=10)
    assert out["free_cash_flow"] is None  # node absent


def test_credit_assessment_accepts_json_and_series_ascending() -> None:
    # Pass JSON (exercises the JSON-input path).
    out = credit_assessment(_results().to_json(), "2025Q4")
    periods = [pt["period"] for pt in out["series"]]
    assert periods == sorted(periods)
    q4 = next(pt for pt in out["series"] if pt["period"] == "2025Q4")
    assert q4["leverage_ratio"] == 3.0
    q1 = next(pt for pt in out["series"] if pt["period"] == "2025Q1")
    assert q1["leverage_ratio"] is None  # incomplete TTM window
