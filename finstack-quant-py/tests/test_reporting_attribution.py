"""Tests for the attribution tear sheet and its waterfall chart."""

from __future__ import annotations

import datetime as dt
from pathlib import Path

DATA = Path(__file__).parent / "data"


def test_waterfall_chart_totals_signs_and_colors() -> None:
    from finstack_quant.reporting import charts
    from finstack_quant.reporting.theme import INSTITUTIONAL

    svg = charts.waterfall_chart(["Carry", "Rates"], [25000.0, -2400.0], theme=INSTITUTIONAL, total_label="Total P&L")
    assert svg.startswith("<svg")
    # all three bar labels present
    for lab in ("Carry", "Rates", "Total P&L"):
        assert lab in svg
    # positive step uses pos color, negative uses neg, total uses ink
    assert INSTITUTIONAL.pos in svg
    assert INSTITUTIONAL.neg in svg
    assert INSTITUTIONAL.ink in svg
    # signed value labels (minus is U+2212)
    assert "+25,000" in svg
    assert "−2,400" in svg
    # total = 25000 - 2400 = 22,600
    assert "+22,600" in svg
    # hover bands present
    assert 'class="fq-hb"' in svg


def _load_attr() -> object:
    from finstack_quant.attribution import PnlAttribution

    return PnlAttribution.from_json((DATA / "attribution_bond.json").read_text())


def test_waterfall_field_order_matches_engine() -> None:
    # Drift guard: the static factor order must match the engine's canonical order.
    from finstack_quant.attribution import default_waterfall_order
    from finstack_quant.reporting.attribution import _WF_FIELD

    assert list(_WF_FIELD.keys()) == list(default_waterfall_order())


def test_attribution_tearsheet_from_object_has_sections_and_kpis() -> None:
    from finstack_quant.reporting import attribution_tearsheet

    ts = attribution_tearsheet(_load_attr(), generated=dt.date(2026, 6, 21))
    titles = [s.title for s in ts.sections]
    assert "P&L Attribution" in titles  # waterfall
    assert "Factor Contributions" in titles  # factor table
    assert "Carry & Roll-Down" in titles  # carry detail (bond has carry)
    assert "Credit Factor Detail" not in titles  # no credit P&L -> omitted (adaptive)
    kpi_labels = [k.label for k in ts.kpis]
    assert kpi_labels == ["Total P&L", "Mark-to-Market", "Carry", "Residual", "Repricings"]
    html = ts.to_html()
    assert "ATTR-BOND-001" in html
    assert "Total P&L" in html
    assert "Carry" in html


def test_attribution_tearsheet_section_selection() -> None:
    from finstack_quant.reporting import attribution_tearsheet

    ts = attribution_tearsheet(_load_attr(), sections=["waterfall"], generated=dt.date(2026, 6, 21))
    assert [s.title for s in ts.sections] == ["P&L Attribution"]


def test_attribution_tearsheet_json_and_object_match() -> None:
    from finstack_quant.reporting import attribution_tearsheet

    raw = (DATA / "attribution_bond.json").read_text()
    from_obj = attribution_tearsheet(_load_attr(), generated=dt.date(2026, 6, 21)).to_html()
    from_json = attribution_tearsheet(raw, generated=dt.date(2026, 6, 21)).to_html()
    assert from_obj == from_json


def test_attribution_tearsheet_requires_inputs() -> None:
    import pytest

    from finstack_quant.reporting import attribution_tearsheet

    with pytest.raises(ValueError, match=r"requires a PnlAttribution|instrument"):
        attribution_tearsheet()


def test_reporting_import_is_engine_light() -> None:
    # Importing reporting and rendering from a PnlAttribution OBJECT must not import
    # the attribution engine module.
    import subprocess
    import sys

    code = (
        "import sys, finstack_quant.reporting as r;"
        "assert 'finstack_quant.attribution' not in sys.modules, "
        "'reporting import pulled the attribution engine'"
    )
    out = subprocess.run([sys.executable, "-c", code], capture_output=True, text=True, check=False)  # noqa: S603
    assert out.returncode == 0, out.stderr


def test_attribution_tearsheet_rejects_unknown_section() -> None:
    import pytest

    from finstack_quant.reporting import attribution_tearsheet

    with pytest.raises(ValueError, match=r"section"):
        attribution_tearsheet(_load_attr(), sections=["nope"])
