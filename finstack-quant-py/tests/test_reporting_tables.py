# finstack-quant-py/tests/test_reporting_tables.py
from __future__ import annotations

from xml.dom import minidom

from finstack_quant.reporting import tables
from finstack_quant.reporting.theme import INSTITUTIONAL


def test_kv_table_renders_rows() -> None:
    html = tables.kv_table([("Sharpe", "1.42", ""), ("Max DD", "-14.6%", "neg")], theme=INSTITUTIONAL)
    minidom.parseString(html)  # noqa: S318
    assert "Sharpe" in html and "1.42" in html  # noqa: PT018
    assert 'class="v neg"' in html


def test_heatmap_grid_shape() -> None:
    rows = [
        (2021, [1.0] * 12, 12.7),
        (2022, [None, -2.0] + [1.0] * 10, -8.0),
    ]
    html = tables.heatmap(rows, theme=INSTITUTIONAL)
    minidom.parseString(html)  # noqa: S318
    assert "2021" in html and "2022" in html  # noqa: PT018
    assert "Jan" in html and "Dec" in html and "Year" in html  # noqa: PT018


def test_data_table_applies_formats() -> None:
    rows = [{"Peak → Trough": "Mar 22 → Sep 22", "Depth": -14.6}]
    html = tables.data_table(
        rows,
        columns=["Peak → Trough", "Depth"],
        formats={"Depth": lambda v: f"{v:.1f}%"},
        neg_columns={"Depth"},
        theme=INSTITUTIONAL,
    )
    minidom.parseString(html)  # noqa: S318
    assert "-14.6%" in html


def test_scroll_wraps_inner_html() -> None:
    inner = '<table class="dd"><tbody><tr><td>x</td></tr></tbody></table>'
    out = tables.scroll(inner)
    assert out.startswith('<div class="fq-scroll">')
    assert inner in out
    assert out.endswith("</div>")
