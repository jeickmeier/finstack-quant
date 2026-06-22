# finstack-quant-py/tests/test_reporting_statements_common.py
from __future__ import annotations

import json
from xml.dom import minidom

import pytest

from finstack_quant import statements
from finstack_quant.reporting import format as fmt, statements_common as sc
from finstack_quant.reporting.theme import INSTITUTIONAL


def _results() -> object:
    """A small evaluated StatementResult with two nodes over two quarters."""
    b = statements.ModelBuilder("demo")
    b.periods("2025Q1..Q2", None)
    b.value("revenue", [("2025Q1", 100.0), ("2025Q2", 110.0)])
    b.value("ebitda", [("2025Q1", 27.0), ("2025Q2", 31.0)])
    return statements.Evaluator().evaluate(b.build())


def test_parse_statement_object_json_and_dict_agree() -> None:
    res = _results()
    views = [
        sc.parse_statement(res),
        sc.parse_statement(res.to_json()),
        sc.parse_statement(json.loads(res.to_json())),
    ]
    for v in views:
        assert v.get("revenue", "2025Q1") == 100.0
        assert v.get("ebitda", "2025Q2") == 31.0
        assert v.get("revenue", "2099Q9") is None
        assert v.get("nope", "2025Q1") is None
        assert "revenue" in v.node_ids()
        assert v.periods() == ["2025Q1", "2025Q2"]


def test_parse_statement_passthrough_view() -> None:
    v = sc.parse_statement(_results())
    assert sc.parse_statement(v) is v


def test_parse_statement_rejects_bad_type() -> None:
    with pytest.raises(TypeError):
        sc.parse_statement(123)


def test_pl_matrix_table_lays_out_items_and_periods() -> None:
    v = sc.parse_statement(_results())
    out = sc.pl_matrix_table(
        v,
        [("Revenue", "revenue", fmt.money), ("EBITDA", "ebitda", fmt.money)],
        ["2025Q1", "2025Q2"],
        theme=INSTITUTIONAL,
    )
    assert 'class="dd"' in out
    assert "<th>2025Q1</th>" in out
    assert "<th>2025Q2</th>" in out
    assert "Revenue" in out
    assert "EBITDA" in out
    assert "100.00" in out  # fmt.money(100.0)
    minidom.parseString(f"<root>{out}</root>")  # well-formed fragment  # noqa: S318


def test_pl_matrix_table_missing_value_is_placeholder() -> None:
    v = sc.parse_statement(_results())
    out = sc.pl_matrix_table(v, [("Missing", "nope", fmt.money)], ["2025Q1"], theme=INSTITUTIONAL)
    assert "·" in out  # fmt.money(None) -> placeholder
