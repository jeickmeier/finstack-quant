"""Binding-level tests for statements / statements_analytics review fixes.

Covers: goal_seek no-update return shape, run_checks formula-check
resolution, ScorecardConfig.period, roll-forward opening balance, and the
StatementResult typed accessors (get_money / get_scalar).
"""

from __future__ import annotations

import json

from finstack.core.currency import Currency
from finstack.core.money import Money
from finstack.statements_analytics import (
    ScorecardConfig,
    add_roll_forward_with_opening,
    goal_seek,
    run_checks,
)
import pytest

from finstack import statements


def _model_json() -> str:
    b = statements.ModelBuilder("test_model")
    b.periods("2024Q1..Q2", None)
    b.value("revenue", [("2024Q1", 100_000.0), ("2024Q2", 110_000.0)])
    b.value("cogs", [("2024Q1", 40_000.0), ("2024Q2", 44_000.0)])
    b.compute("gross_profit", "revenue - cogs")
    return b.build().to_json()


class TestGoalSeek:
    def test_update_model_false_returns_none(self) -> None:
        solved, updated = goal_seek(
            _model_json(),
            "gross_profit",
            "2024Q1",
            70_000.0,
            "revenue",
            "2024Q1",
            update_model=False,
        )
        assert solved > 100_000.0
        assert updated is None

    def test_update_model_true_returns_json(self) -> None:
        solved, updated = goal_seek(
            _model_json(),
            "gross_profit",
            "2024Q1",
            70_000.0,
            "revenue",
            "2024Q1",
            update_model=True,
        )
        assert solved > 100_000.0
        assert updated is not None
        json.loads(updated)


class TestRunChecks:
    def test_formula_checks_are_resolved(self) -> None:
        spec = {
            "name": "formula suite",
            "builtin_checks": [],
            "formula_checks": [
                {
                    "id": "revenue_positive",
                    "name": "Revenue must be positive",
                    "category": "internal_consistency",
                    "severity": "error",
                    "formula": "revenue > 0",
                    "message_template": "Revenue not positive in {period}",
                }
            ],
        }
        report_json = run_checks(_model_json(), json.dumps(spec))
        assert "revenue_positive" in report_json


class TestScorecardConfig:
    def test_period_round_trip(self) -> None:
        cfg = ScorecardConfig(rating_scale="S&P", metrics=[], period="2024Q1")
        assert cfg.period == "2024Q1"
        assert json.loads(cfg.to_json())["period"] == "2024Q1"

    def test_period_defaults_to_none(self) -> None:
        cfg = ScorecardConfig()
        assert cfg.period is None


class TestRollForward:
    def test_opening_balance_seeds_first_period(self) -> None:
        b = statements.ModelBuilder("rf")
        b.periods("2024Q1..Q2", None)
        b.value("adds", [("2024Q1", 10.0), ("2024Q2", 10.0)])
        model_json = b.build().to_json()
        updated = add_roll_forward_with_opening(model_json, "bal", ["adds"], [], 100.0)
        model = statements.FinancialModelSpec.from_json(updated)
        result = statements.Evaluator().evaluate(model)
        assert result.get("bal_end", "2024Q1") == pytest.approx(110.0)
        assert result.get("bal_end", "2024Q2") == pytest.approx(120.0)


class TestStatementResultTypedAccessors:
    def test_get_money_and_get_scalar(self) -> None:
        usd = Currency("USD")
        b = statements.ModelBuilder("money_model")
        b.periods("2025Q1..Q1", None)
        b.value_money("revenue", [("2025Q1", Money(100.0, usd))])
        b.value("margin_pct", [("2025Q1", 0.4)])
        result = statements.Evaluator().evaluate(b.build())

        money = result.get_money("revenue", "2025Q1")
        assert money is not None
        assert money.currency.code == "USD"
        assert float(money.amount) == pytest.approx(100.0)
        # Scalar node is not monetary.
        assert result.get_money("margin_pct", "2025Q1") is None

        assert result.get_scalar("margin_pct", "2025Q1") == pytest.approx(0.4)
        # Monetary node is not scalar.
        assert result.get_scalar("revenue", "2025Q1") is None
