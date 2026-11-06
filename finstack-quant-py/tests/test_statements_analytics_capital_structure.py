"""Debt stack by seniority over an evaluated capital structure."""

from __future__ import annotations

from datetime import date
import json
import pickle

import pytest

from finstack_quant.core.currency import Currency
from finstack_quant.core.money import Money
from finstack_quant.statements import Evaluator, FinancialModelSpec, ModelBuilder, StatementResult
from finstack_quant.statements_analytics import (
    CapitalStructureSummary,
    CapitalStructureTranche,
    MaturityWallBucket,
    capital_structure_summary,
)

QUARTERS = ["2025Q1", "2025Q2", "2025Q3", "2025Q4"]
FULL_YEAR = {"period": "2025Q4", "ebitda_node": "ebitda", "cash_node": "cash"}


def _model() -> tuple[FinancialModelSpec, StatementResult]:
    """Two classes: a 500,000 pari passu 1L pair at 5% and a 420,000 2L bond at 8%."""
    builder = ModelBuilder("borrower")
    builder.periods("2025Q1..Q4", None)
    builder.value("ebitda", list(zip(QUARTERS, [100_000.0, 110_000.0, 120_000.0, 130_000.0], strict=True)))
    builder.value("cash", [(quarter, 150_000.0) for quarter in QUARTERS])
    usd = Currency("USD")
    builder.add_bond("SUB", Money(420_000.0, usd), 0.08, date(2024, 1, 1), date(2031, 3, 1), "USD-OIS")
    builder.add_bond("SR-A", Money(400_000.0, usd), 0.05, date(2024, 1, 1), date(2029, 6, 1), "USD-OIS")
    builder.add_bond("SR-B", Money(100_000.0, usd), 0.05, date(2024, 1, 1), date(2029, 1, 1), "USD-OIS")
    document = json.loads(builder.build().to_json())
    document["capital_structure"]["waterfall"] = {
        "available_cash_node": "cash",
        "priority_of_payments": ["fees", "interest", "amortization"],
        "payment_classes": [
            {"id": "2L", "rank": 1, "instrument_ids": ["SUB"]},
            {"id": "1L", "rank": 0, "instrument_ids": ["SR-A", "SR-B"]},
        ],
    }
    model = FinancialModelSpec.from_json(json.dumps(document))
    return model, Evaluator().evaluate(model)


def test_summary_figures_through_each_class() -> None:
    model, results = _model()
    summary = capital_structure_summary(results, model, FULL_YEAR)

    assert summary.as_of == "2025Q4"
    assert summary.currency == "USD"
    assert summary.total_debt.amount == 920_000.0
    assert summary.ltm_ebitda is not None
    assert summary.ltm_ebitda.amount == 460_000.0
    assert summary.total_leverage == pytest.approx(2.0)
    assert summary.net_debt is not None
    assert summary.net_debt.amount == 770_000.0
    assert summary.net_leverage == pytest.approx(770_000.0 / 460_000.0)

    assert [(t.id, t.kind, t.rank, t.payment_class) for t in summary.tranches] == [
        ("SR-A", "bond", 0, "1L"),
        ("SR-B", "bond", 0, "1L"),
        ("SUB", "bond", 1, "2L"),
    ]
    senior_a, senior_b, sub = summary.tranches
    # A full year of coupons: 400,000 at 5%, 100,000 at 5%, 420,000 at 8%.
    assert senior_a.ltm_interest is not None
    assert senior_a.ltm_interest.amount == pytest.approx(20_000.0)
    assert senior_b.ltm_interest is not None
    assert senior_b.ltm_interest.amount == pytest.approx(5_000.0)
    assert sub.ltm_interest is not None
    assert sub.ltm_interest.amount == pytest.approx(33_600.0)
    for senior in (senior_a, senior_b):
        assert senior.cumulative_debt.amount == 500_000.0
        assert senior.leverage == pytest.approx(500_000.0 / 460_000.0)
        assert senior.interest_coverage == pytest.approx(460_000.0 / 25_000.0)
    assert sub.interest_coverage == pytest.approx(460_000.0 / 58_600.0)
    assert summary.total_coverage == sub.interest_coverage
    assert senior_a.maturity == date(2029, 6, 1)

    assert [(b.year, b.amount.amount) for b in summary.maturity_wall] == [(2029, 500_000.0), (2031, 420_000.0)]
    assert summary.maturity_wall[0].share_of_debt == pytest.approx(500_000.0 / 920_000.0)


def test_a_short_window_gives_no_ratios() -> None:
    model, results = _model()
    summary = capital_structure_summary(results, model, {"period": "2025Q3", "ebitda_node": "ebitda"})
    assert (summary.ltm_periods_required, summary.ltm_periods_found) == (4, 3)
    assert summary.ltm_ebitda is None
    assert summary.total_leverage is None
    assert summary.total_coverage is None
    assert summary.cash is None
    assert summary.net_leverage is None
    assert all(t.leverage is None and t.interest_coverage is None for t in summary.tranches)


def test_wire_forms_round_trip() -> None:
    model, results = _model()
    summary = capital_structure_summary(results.to_json(), model.to_json(), json.dumps(FULL_YEAR))
    assert CapitalStructureSummary.from_json(summary.to_json()).to_dict() == summary.to_dict()
    assert pickle.loads(pickle.dumps(summary)).to_json() == summary.to_json()  # noqa: S301
    tranche = CapitalStructureTranche.from_json(summary.tranches[0].to_json())
    assert tranche.to_dict() == summary.to_dict()["tranches"][0]
    bucket = MaturityWallBucket.from_json(summary.maturity_wall[0].to_json())
    assert bucket.to_dict() == summary.to_dict()["maturity_wall"][0]


def test_dataframe_has_a_row_per_tranche() -> None:
    model, results = _model()
    frame = capital_structure_summary(results, model, FULL_YEAR).to_dataframe()
    assert list(frame["id"]) == ["SR-A", "SR-B", "SUB"]
    assert list(frame["kind"]) == ["bond", "bond", "bond"]
    assert list(frame["balance"]) == [400_000.0, 100_000.0, 420_000.0]
    assert list(frame["maturity"]) == ["2029-06-01", "2029-01-01", "2031-03-01"]
    assert frame["leverage"].iloc[2] == pytest.approx(2.0)


@pytest.mark.parametrize(
    ("config", "message"),
    [
        ({"period": "2030Q1", "ebitda_node": "ebitda"}, "not in the model"),
        ({"period": "2025Q4", "ebitda_node": "nope"}, "'nope'"),
        ({"period": "2025Q4", "ebitda_node": "ebitda", "cash_node": "nope"}, "'nope'"),
        ({"period": "2025Q4", "ebitda_node": "ebitda", "guess": True}, "guess"),
        ({"period": "2025Q4"}, "ebitda_node"),
    ],
)
def test_bad_requests_are_value_errors(config: dict[str, object], message: str) -> None:
    model, results = _model()
    with pytest.raises(ValueError, match=message):
        capital_structure_summary(results, model, config)


def test_a_model_without_a_capital_structure_is_a_value_error() -> None:
    builder = ModelBuilder("no-debt")
    builder.periods("2025Q1..Q4", None)
    builder.value("ebitda", [(quarter, 1.0) for quarter in QUARTERS])
    model = builder.build()
    with pytest.raises(ValueError, match="no capital structure"):
        capital_structure_summary(Evaluator().evaluate(model), model, {"period": "2025Q4", "ebitda_node": "ebitda"})
