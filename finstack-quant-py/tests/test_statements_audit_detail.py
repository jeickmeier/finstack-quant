"""Audit detail on statement results: cell sources, check comparisons, waterfall sources and uses.

Each test reconstructs a reported number from data exported through the public
API: which precedence layer produced a statement cell, the two values and the
tolerance behind a check verdict, and the split of a period's principal
payment by source.
"""

from __future__ import annotations

import json

import pandas as pd
import pytest

from finstack_quant.statements import (
    CapitalStructureCashflows,
    CheckReport,
    CheckSuiteSpec,
    Evaluator,
    FinancialModelSpec,
    ForecastSpec,
    ModelBuilder,
    StatementResult,
)

Q1, Q2, Q3 = "2025Q1", "2025Q2", "2025Q3"


def _mixed_result() -> StatementResult:
    """One node resolved by a different layer in each period, plus a masked node.

    Q1 and Q2 are actuals: Q1 has an explicit value, Q2 falls through to the
    formula (forecasts never run in actual periods), and Q3 is forecast.
    """
    builder = ModelBuilder("cell-sources")
    builder.periods("2025Q1..Q3", Q2)
    mixed = builder.mixed("revenue")
    mixed.values([(Q1, 100.0)])
    mixed.forecast(ForecastSpec.growth(0.10))
    mixed.formula("123")
    builder = mixed.build()
    builder.compute("masked", "42")
    builder.where_clause("masked", "1 == 0")
    return Evaluator().evaluate(builder.build())


def test_cells_report_status_source_and_value() -> None:
    result = _mixed_result()
    cells = result.cells

    assert {period: cell["source"] for period, cell in cells["revenue"].items()} == {
        Q1: "value",
        Q2: "formula",
        Q3: "forecast",
    }
    assert cells["masked"] == {Q1: {"status": "masked"}, Q2: {"status": "masked"}, Q3: {"status": "masked"}}
    assert result.get("revenue", Q1) == 100.0
    assert result.get("revenue", Q2) == 123.0
    assert result.get("masked", Q1) is None, "a masked cell has no value"
    assert result.get_cell("masked", Q1) == {"status": "masked"}


def test_cells_cover_every_period_and_round_trip() -> None:
    result = _mixed_result()
    document = json.loads(result.to_json())

    for cells in document["cells"].values():
        assert list(cells) == [Q1, Q2, Q3]
    assert StatementResult.from_json(result.to_json()).cells == result.cells
    assert result.provenance["model_id"] == document["provenance"]["model_id"]
    assert result.provenance["observation_cutoff"] == "full_snapshot"


def test_long_dataframe_carries_status_reason_and_source() -> None:
    frame = _mixed_result().to_pandas("long")

    assert list(frame.columns) == [
        "node_id",
        "period_id",
        "status",
        "value",
        "amount",
        "currency",
        "value_type",
        "reason",
        "source",
    ]
    revenue = frame[frame["node_id"] == "revenue"].set_index("period_id")["source"]
    assert revenue.to_dict() == {Q1: "value", Q2: "formula", Q3: "forecast"}
    masked = frame[frame["node_id"] == "masked"]
    assert masked["status"].tolist() == ["masked"] * 3
    assert masked["value"].isna().all()


def test_hand_built_result_exports_null_source() -> None:
    document = json.loads(_mixed_result().to_json())
    for cells in document["cells"].values():
        for cell in cells.values():
            cell.pop("source", None)
    hand_built = StatementResult.from_json(json.dumps(document))

    assert hand_built.to_pandas("long")["source"].isna().all()


def _balance_sheet_report() -> CheckReport:
    """Q1 is out of balance by 100; Q2 balances."""
    builder = ModelBuilder("bs")
    builder.periods("2025Q1..Q2", None)
    builder.value("total_assets", [(Q1, 1000.0), (Q2, 1100.0)])
    builder.value("total_liabilities", [(Q1, 600.0), (Q2, 700.0)])
    builder.value("total_equity", [(Q1, 300.0), (Q2, 400.0)])
    suite = CheckSuiteSpec.from_json(
        json.dumps({
            "name": "bs",
            "builtin_checks": [
                {
                    "type": "balance_sheet_articulation",
                    "assets_nodes": ["total_assets"],
                    "liabilities_nodes": ["total_liabilities"],
                    "equity_nodes": ["total_equity"],
                    "tolerance": None,
                }
            ],
            "formula_checks": [],
        })
    )
    evaluator = Evaluator()
    evaluator.with_checks(suite)
    report = evaluator.evaluate(builder.build()).check_report
    assert report is not None
    return report


def test_failed_finding_reports_actual_expected_and_tolerance() -> None:
    report = _balance_sheet_report()
    (finding,) = report.findings

    comparison = finding.comparison
    assert comparison == {
        "identity": "balance_sheet_articulation",
        "period": Q1,
        "actual": 1000.0,
        "expected": 900.0,
        "difference": 100.0,
        "tolerance": 0.0,
    }
    # The numbers reconcile with the reported materiality and the verdict.
    difference = abs(comparison["difference"])
    assert difference == abs(comparison["actual"] - comparison["expected"])
    assert difference == finding.materiality_absolute
    assert difference > comparison["tolerance"]

    row = report.to_findings_table().to_pandas().iloc[0]
    assert row["comparison_actual"] == 1000.0
    assert row["comparison_expected"] == 900.0
    assert row["comparison_tolerance"] == 0.0


def test_comparisons_dataframe_lists_passing_periods_too() -> None:
    frame = _balance_sheet_report().to_comparisons_table().to_pandas()

    assert list(frame.columns) == [
        "check_id",
        "check_name",
        "category",
        "identity",
        "period",
        "actual",
        "expected",
        "difference",
        "tolerance",
        "currency",
        "within_tolerance",
    ]
    by_period = frame.set_index("period")
    assert by_period.loc[Q1, "difference"] == 100.0
    assert not bool(by_period.loc[Q1, "within_tolerance"])
    assert by_period.loc[Q2, "actual"] == 1100.0
    assert by_period.loc[Q2, "expected"] == 1100.0
    assert bool(by_period.loc[Q2, "within_tolerance"])
    assert (frame["actual"] - frame["expected"]).tolist() == frame["difference"].tolist()


def test_report_without_comparisons_yields_typed_empty_frame() -> None:
    report = CheckReport.from_json(
        json.dumps({
            "outcome": "unevaluated",
            "results": [],
            "summary": {
                "total_checks": 0,
                "passed": 0,
                "failed": 0,
                "incomplete": 0,
                "unevaluated": 0,
                "errors": 0,
                "warnings": 0,
                "infos": 0,
            },
        })
    )
    frame = report.to_comparisons_table().to_pandas()

    assert len(frame) == 0
    assert str(frame.dtypes["actual"]) == "float64"
    assert str(frame.dtypes["within_tolerance"]) == "bool"


def _usd(amount: str) -> dict[str, str]:
    return {"amount": amount, "currency": "USD"}


def _cashflows(*, with_detail: bool) -> CapitalStructureCashflows:
    breakdown = {
        "interest_expense_cash": _usd("80"),
        "interest_expense_pik": _usd("0"),
        "principal_payment": _usd("465"),
        "fees": _usd("5"),
        "debt_balance": _usd("9535"),
        "accrued_interest": _usd("0"),
    }
    payload: dict[str, object] = {
        "by_instrument": {"TL-1": {Q1: breakdown}},
        "reporting_currency": "USD",
    }
    if with_detail:
        breakdown.update({
            "opening_balance": _usd("10000"),
            "scheduled_principal": _usd("100"),
            "mandatory_prepayment": _usd("200"),
            "sweep_prepayment": _usd("115"),
            "voluntary_prepayment": _usd("50"),
        })
        payload["equity_distribution"] = {Q1: _usd("450")}
        payload["available_cash"] = {Q1: _usd("1000")}
    return CapitalStructureCashflows.from_json(json.dumps(payload))


def test_principal_sources_sum_to_principal_payment() -> None:
    frame = _cashflows(with_detail=True).to_pandas()
    amounts = frame.set_index("flow_type")["value"]

    sources = ["scheduled_principal", "mandatory_prepayment", "sweep_prepayment", "voluntary_prepayment"]
    assert amounts[sources].sum() == amounts["principal_payment"] == 465.0
    assert amounts["opening_balance"] - amounts["principal_payment"] == amounts["debt_balance"]


def test_available_cash_reconciles_the_period_uses() -> None:
    cashflows = _cashflows(with_detail=True)
    amounts = cashflows.to_pandas().set_index("flow_type")["value"]
    available = cashflows.available_cash[Q1]
    equity = cashflows.equity_distribution[Q1]

    assert str(available.currency) == "USD"
    uses = amounts["fees"] + amounts["interest_expense_cash"] + amounts["principal_payment"] + float(equity.amount)
    assert uses == float(available.amount) == 1000.0


def test_cashflows_written_before_the_split_still_load() -> None:
    cashflows = _cashflows(with_detail=False)
    frame = cashflows.to_pandas()

    assert cashflows.available_cash == {}
    assert isinstance(frame, pd.DataFrame)
    assert "scheduled_principal" not in set(frame["flow_type"])
    assert "opening_balance" not in set(frame["flow_type"])
    assert "principal_payment" in set(frame["flow_type"])


def test_model_restore_rejects_duplicate_daily_reporting_period() -> None:
    builder = ModelBuilder("duplicate-day")
    builder.periods("2025D001..D365")
    builder.compute("ebitda", "1")
    payload = json.loads(builder.build().to_json())
    rows = payload["periods"]["periods"]
    assert len(rows) == 365
    rows[1] = rows[0]
    with pytest.raises(ValueError, match="unique"):
        FinancialModelSpec.from_json(json.dumps(payload))
